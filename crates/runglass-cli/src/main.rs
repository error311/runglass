use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use runglass_core::{
    apply_revert, delete_report, fixture, latest_report, list_reports, load_report, make_run_id,
    prepare_run_paths, preview_revert, prune_reports, render_markdown_receipt,
    render_reverse_patch, report_run_dir, reports_dir, run_observed_command_in_mode,
    snapshot_directory_with_stats, snapshot_file_byte_limit, write_report_bundle, ObservationMode,
    RevertConflictPolicy, RevertOptions, RiskLevel, RunReport, RunStatus,
};
use runglass_web::{serve_report, serve_report_on_port, write_standalone_html};

const UNSUPPORTED_PLATFORM_MESSAGE: &str = "RunGlass command observation is Linux-first in this release.\nThis platform is not supported yet.";

#[derive(Debug, Parser)]
#[command(
    name = "runglass",
    version,
    about = "Run any command. Get a receipt for what changed."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(
        about = "Run one command and create a receipt",
        after_help = "RunGlass wraps one command and creates a receipt.\n\nExamples:\n  runglass run -- docker compose up -d\n  runglass run docker compose up -d\n  runglass run --deep -- ./install.sh"
    )]
    Run {
        #[arg(long)]
        open: bool,
        #[arg(long)]
        deep: bool,
        #[arg(required = true, num_args = 1.., allow_hyphen_values = true, trailing_var_arg = true)]
        command: Vec<String>,
    },
    #[command(about = "Open a receipt in the local browser UI")]
    Open {
        #[arg(default_value = "latest")]
        run_id: String,
        #[arg(
            long,
            default_value_t = 0,
            help = "Port to bind, or 0 for any free port"
        )]
        port: u16,
    },
    #[command(about = "Run one command in CI and write receipt artifacts")]
    Ci {
        #[arg(long)]
        deep: bool,
        #[arg(long, value_enum, default_value_t = CiProvider::Auto)]
        provider: CiProvider,
        #[arg(long, default_value = "runglass-receipt")]
        out: PathBuf,
        #[arg(
            long = "format",
            value_enum,
            value_delimiter = ',',
            default_value = "html,json,markdown"
        )]
        formats: Vec<CiFormat>,
        #[arg(required = true, num_args = 1.., allow_hyphen_values = true, trailing_var_arg = true)]
        command: Vec<String>,
    },
    Report {
        run_id: String,
        #[arg(long)]
        print_json: bool,
        #[arg(long, help = "Open the local receipt UI in your browser")]
        open: bool,
        #[arg(
            long = "no-open",
            help = "Serve the receipt UI without opening a browser"
        )]
        no_open: bool,
        #[arg(
            long,
            default_value_t = 0,
            help = "Port to bind, or 0 for any free port"
        )]
        port: u16,
    },
    List {
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        risk: Option<String>,
        #[arg(long)]
        mode: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Export {
        run_id: String,
        #[arg(long)]
        html: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        markdown: bool,
        #[arg(long = "reverse-patch")]
        reverse_patch: bool,
        #[arg(long)]
        bundle: bool,
    },
    Doctor,
    Snapshot {
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    Prune {
        #[arg(long, default_value_t = 50)]
        keep: usize,
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    Delete {
        run_id: String,
    },
    Revert {
        run_id: String,
        #[arg(long = "file")]
        files: Vec<String>,
        #[arg(long)]
        force: bool,
        #[arg(long = "skip-changed")]
        skip_changed: bool,
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    Demo {
        #[arg(long)]
        open: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CiProvider {
    Auto,
    Github,
    Gitlab,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CiFormat {
    Html,
    Json,
    Markdown,
}

struct CiArtifact {
    label: &'static str,
    path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            open,
            deep,
            command,
        } => run_command(command, open, deep),
        Commands::Open { run_id, port } => open_receipt(&run_id, port),
        Commands::Ci {
            deep,
            provider,
            out,
            formats,
            command,
        } => ci_command(command, deep, provider, &out, &formats),
        Commands::Report {
            run_id,
            print_json,
            open,
            no_open,
            port,
        } => {
            let report = resolve_receipt(&run_id)?;
            if print_json {
                println!("{}", serde_json::to_string_pretty(&report)?);
                Ok(())
            } else {
                serve_report_on_port(report, open || !no_open, port)
            }
        }
        Commands::List {
            query,
            risk,
            mode,
            limit,
        } => {
            for report in filter_reports(
                list_reports()?,
                query.as_deref(),
                risk.as_deref(),
                mode.as_deref(),
            )?
            .into_iter()
            .take(limit)
            {
                println!(
                    "{}\t{}\t{}\t{}",
                    report.run.id,
                    report.run.started_at,
                    report.run.exit_code.unwrap_or(-1),
                    report.run.command_display
                );
            }
            Ok(())
        }
        Commands::Export {
            run_id,
            html,
            json,
            markdown,
            reverse_patch,
            bundle,
        } => export_report(&run_id, html, json, markdown, reverse_patch, bundle),
        Commands::Doctor => doctor(),
        Commands::Snapshot { dry_run } => snapshot_dry_run(dry_run),
        Commands::Prune { keep, dry_run } => prune_receipts(keep, dry_run),
        Commands::Delete { run_id } => delete_receipt(&run_id),
        Commands::Revert {
            run_id,
            files,
            force,
            skip_changed,
            dry_run,
        } => revert_receipt(&run_id, &files, force, skip_changed, dry_run),
        Commands::Demo { open } => run_demo(open),
    }
}

fn run_command(command: Vec<String>, open: bool, deep: bool) -> Result<()> {
    ensure_observation_supported()?;
    let mode = if deep {
        ObservationMode::Deep
    } else {
        ObservationMode::Normal
    };
    let (report, paths) = run_observed_command_in_mode(command, mode)?;

    println!("Created receipt {}", report.run.id);
    println!("{}", paths.report_path.display());

    if open {
        serve_report(report, true)?;
    }

    Ok(())
}

fn open_receipt(run_id: &str, port: u16) -> Result<()> {
    let report = resolve_receipt(run_id)?;
    serve_report_on_port(report, true, port)
}

fn ci_command(
    command: Vec<String>,
    deep: bool,
    provider: CiProvider,
    out: &Path,
    formats: &[CiFormat],
) -> Result<()> {
    ensure_observation_supported()?;
    if formats.is_empty() {
        return Err(anyhow!("at least one --format is required"));
    }

    let mode = if deep {
        ObservationMode::Deep
    } else {
        ObservationMode::Normal
    };
    let provider = detect_ci_provider(provider);
    let (report, _paths) = run_observed_command_in_mode(command, mode)?;
    let artifacts = write_ci_artifacts(&report, out, formats)?;
    let summary = render_ci_summary(&report, provider, &artifacts);
    let summary_path = out.join("summary.md");
    fs::write(&summary_path, &summary)?;

    println!("Created CI receipt {}", report.run.id);
    println!("{}", out.display());
    println!();
    print!("{summary}");

    if matches!(provider, CiProvider::Github) {
        append_github_step_summary(&summary)?;
    }

    if let Some(code) = report.run.exit_code {
        if code != 0 {
            process::exit(normalize_exit_code(code));
        }
    } else if !matches!(report.run.status, RunStatus::Completed) {
        process::exit(1);
    }

    Ok(())
}

fn write_ci_artifacts(
    report: &RunReport,
    out: &Path,
    formats: &[CiFormat],
) -> Result<Vec<CiArtifact>> {
    fs::create_dir_all(out)?;
    let mut artifacts = Vec::new();

    if formats.contains(&CiFormat::Html) {
        let path = out.join("receipt.html");
        write_standalone_html(report, &path)?;
        artifacts.push(CiArtifact {
            label: "HTML receipt",
            path,
        });
    }

    if formats.contains(&CiFormat::Json) {
        let path = out.join("receipt.json");
        fs::write(&path, serde_json::to_vec_pretty(report)?)?;
        artifacts.push(CiArtifact {
            label: "JSON receipt",
            path,
        });
    }

    if formats.contains(&CiFormat::Markdown) {
        let path = out.join("receipt.md");
        fs::write(&path, render_markdown_receipt(report))?;
        artifacts.push(CiArtifact {
            label: "Markdown receipt",
            path,
        });
    }

    Ok(artifacts)
}

fn detect_ci_provider(provider: CiProvider) -> CiProvider {
    match provider {
        CiProvider::Auto if std::env::var_os("GITHUB_ACTIONS").is_some() => CiProvider::Github,
        CiProvider::Auto if std::env::var_os("GITLAB_CI").is_some() => CiProvider::Gitlab,
        CiProvider::Auto => CiProvider::Generic,
        explicit => explicit,
    }
}

fn render_ci_summary(report: &RunReport, provider: CiProvider, artifacts: &[CiArtifact]) -> String {
    let mut lines = Vec::new();
    lines.push("## RunGlass CI Receipt".to_string());
    lines.push(String::new());
    lines.push(format!(
        "> {}",
        concise_receipt_narrative(report, "This command")
    ));
    lines.push(String::new());
    lines.push("| Field | Value |".to_string());
    lines.push("| --- | --- |".to_string());
    lines.push(format!(
        "| Command | `{}` |",
        escape_markdown_table_cell(&report.run.command_display)
    ));
    lines.push(format!("| Receipt ID | `{}` |", report.run.id));
    lines.push(format!("| Status | {} |", status_label(&report.run.status)));
    lines.push(format!(
        "| Exit Code | {} |",
        report
            .run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    ));
    lines.push(format!(
        "| Risk | {} |",
        risk_level_label(&report.summary.risk_level)
    ));
    lines.push(String::new());
    lines.push("### What Changed".to_string());
    lines.push(format!(
        "- Files: {} created, {} modified, {} deleted.",
        report.summary.files_created, report.summary.files_modified, report.summary.files_deleted
    ));
    lines.push(format!(
        "- Runtime: {} child processes, {} outbound hosts, {} listening ports.",
        report.summary.processes_seen, report.summary.network_hosts, report.summary.ports_opened
    ));
    lines.push(format!(
        "- Docker: {} containers, {} images, {} volumes.",
        report.summary.docker_containers_created,
        report.summary.docker_images_pulled,
        report.summary.docker_volumes_created
    ));

    if !artifacts.is_empty() {
        lines.push(String::new());
        lines.push("### Artifacts".to_string());
        for artifact in artifacts {
            lines.push(format!(
                "- {}: `{}`",
                artifact.label,
                artifact.path.display()
            ));
        }
        lines.push("- CI summary: `summary.md`".to_string());
    }

    match provider {
        CiProvider::Github => {
            lines.push(String::new());
            lines.push(
                "GitHub Actions: upload the output directory with `actions/upload-artifact`."
                    .to_string(),
            );
        }
        CiProvider::Gitlab => {
            lines.push(String::new());
            lines.push(
                "GitLab CI: publish the output directory with `artifacts:paths`.".to_string(),
            );
        }
        CiProvider::Auto | CiProvider::Generic => {}
    }

    lines.push(String::new());
    lines.join("\n")
}

fn concise_receipt_narrative(report: &RunReport, subject: &str) -> String {
    let mut clauses = Vec::new();
    clauses.push(format!(
        "changed {} file{}",
        report.summary.files_changed,
        plural(report.summary.files_changed)
    ));
    clauses.push(format!(
        "observed {} child process{}",
        report.summary.processes_seen,
        if report.summary.processes_seen == 1 {
            ""
        } else {
            "es"
        }
    ));
    clauses.push(format!(
        "contacted {} host{}",
        report.summary.network_hosts,
        plural(report.summary.network_hosts)
    ));
    clauses.push(format!(
        "opened {} listening port{}",
        report.summary.ports_opened,
        plural(report.summary.ports_opened)
    ));
    format!("{subject} {}.", clauses.join(", "))
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

fn escape_markdown_table_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn append_github_step_summary(summary: &str) -> Result<()> {
    let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
        return Ok(());
    };
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file)?;
    file.write_all(summary.as_bytes())?;
    Ok(())
}

fn normalize_exit_code(code: i32) -> i32 {
    if (0..=255).contains(&code) {
        code
    } else {
        1
    }
}

fn run_demo(open: bool) -> Result<()> {
    let run_id = make_run_id("demo-receipt");
    let paths = prepare_run_paths(&run_id)?;
    let mut report = fixture::sample_report_at(run_id, Utc::now());
    if let Some(docker) = &report.docker {
        report.summary.docker_containers_created = docker.containers_created.len();
        report.summary.docker_images_pulled = docker.images_pulled.len();
        report.summary.docker_volumes_created = docker.volumes_created.len();
    }
    report.stdout_path = Some(paths.stdout_path.display().to_string());
    report.stderr_path = Some(paths.stderr_path.display().to_string());

    write_report_bundle(
        &paths,
        &report,
        report.stdout.as_deref().unwrap_or_default(),
        report.stderr.as_deref().unwrap_or_default(),
    )?;

    println!("Created demo receipt {}", report.run.id);
    println!("{}", paths.report_path.display());

    if open {
        serve_report(report, true)?;
    }

    Ok(())
}

fn export_report(
    run_id: &str,
    html: bool,
    json: bool,
    markdown: bool,
    reverse_patch: bool,
    bundle: bool,
) -> Result<()> {
    let report = resolve_receipt(run_id)?;
    let base = report_run_dir(&report.run.id)?;

    if html || (!json && !markdown && !reverse_patch && !bundle) {
        let html_path = base.join("receipt.html");
        write_standalone_html(&report, &html_path)?;
        println!("{}", html_path.display());
    }

    if json {
        let json_path = base.join("receipt.json");
        let data = serde_json::to_vec_pretty(&report)?;
        fs::write(&json_path, data)?;
        println!("{}", json_path.display());
    }

    if markdown {
        let markdown_path = base.join("receipt.md");
        fs::write(&markdown_path, render_markdown_receipt(&report))?;
        println!("{}", markdown_path.display());
    }

    if reverse_patch {
        let patch_path = base.join("receipt-reverse.patch");
        fs::write(&patch_path, render_reverse_patch(&report)?)?;
        println!("{}", patch_path.display());
    }

    if bundle {
        let bundle_path = write_share_bundle(&report, &base)?;
        println!("{}", bundle_path.display());
    }

    Ok(())
}

fn filter_reports(
    reports: Vec<RunReport>,
    query: Option<&str>,
    risk: Option<&str>,
    mode: Option<&str>,
) -> Result<Vec<RunReport>> {
    let query = query.map(str::to_ascii_lowercase);
    let risk = risk.map(str::to_ascii_lowercase);
    let mode = mode.map(str::to_ascii_lowercase);
    Ok(reports
        .into_iter()
        .filter(|report| {
            query.as_ref().is_none_or(|query| {
                report.run.id.to_ascii_lowercase().contains(query)
                    || report
                        .run
                        .command_display
                        .to_ascii_lowercase()
                        .contains(query)
                    || report
                        .files
                        .iter()
                        .any(|file| file.path.to_ascii_lowercase().contains(query))
                    || report.network.iter().any(|event| {
                        event.ip.to_ascii_lowercase().contains(query)
                            || event
                                .host
                                .as_deref()
                                .unwrap_or_default()
                                .to_ascii_lowercase()
                                .contains(query)
                    })
            })
        })
        .filter(|report| {
            risk.as_ref()
                .is_none_or(|risk| risk_level_label(&report.summary.risk_level) == risk.as_str())
        })
        .filter(|report| {
            mode.as_ref().is_none_or(|mode| {
                matches!(
                    (mode.as_str(), report.run.mode),
                    ("normal", ObservationMode::Normal) | ("deep", ObservationMode::Deep)
                )
            })
        })
        .collect())
}

fn risk_level_label(risk: &RiskLevel) -> &'static str {
    match risk {
        RiskLevel::None => "none",
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
    }
}

fn status_label(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "running",
        RunStatus::Completed => "completed",
        RunStatus::Interrupted => "interrupted",
        RunStatus::FailedToStart => "failed to start",
        RunStatus::TimedOut => "timed out",
    }
}

fn doctor() -> Result<()> {
    println!("RunGlass Doctor");
    let platform_supported = observation_supported();
    print_check("Platform", std::env::consts::OS, platform_supported);
    if !platform_supported {
        println!();
        println!("{}", unsupported_platform_message());
    }
    print_check(
        "Reports directory",
        &reports_dir()?.display().to_string(),
        true,
    );
    print_check(
        "Snapshot cap",
        &human_size(snapshot_file_byte_limit()),
        snapshot_file_byte_limit() > 0,
    );
    if platform_supported {
        print_check("ss", "socket sampling helper", command_on_path("ss"));
        print_check(
            "strace",
            "deep mode tracing helper",
            command_on_path("strace"),
        );
    }
    print_check("docker", "Docker diff support", command_on_path("docker"));
    println!();
    println!(
        "Tip: run `runglass snapshot --dry-run` inside a project to preview file capture scope."
    );
    Ok(())
}

fn ensure_observation_supported() -> Result<()> {
    if observation_supported() {
        Ok(())
    } else {
        Err(anyhow!(unsupported_platform_message()))
    }
}

fn observation_supported() -> bool {
    cfg!(target_os = "linux")
}

fn unsupported_platform_message() -> &'static str {
    UNSUPPORTED_PLATFORM_MESSAGE
}

fn print_check(name: &str, detail: &str, ok: bool) {
    println!("{}\t{}\t{}", if ok { "ok" } else { "warn" }, name, detail);
}

fn snapshot_dry_run(_dry_run: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (entries, stats) = snapshot_directory_with_stats(&cwd)?;
    println!("Snapshot dry run for {}", cwd.display());
    println!("Captured files: {}", entries.len());
    println!("Per-file cap: {}", human_size(snapshot_file_byte_limit()));
    println!(
        ".runglassignore: {}",
        if cwd.join(".runglassignore").exists() {
            "active"
        } else {
            "not present"
        }
    );
    println!("Skipped large files: {}", stats.skipped_large_files.len());
    for skipped in stats.skipped_large_files.iter().take(12) {
        println!("- {} ({})", skipped.path, human_size(skipped.size));
    }
    Ok(())
}

fn prune_receipts(keep: usize, dry_run: bool) -> Result<()> {
    let deleted = prune_reports(keep, dry_run)?;
    println!(
        "{} {} receipt{}",
        if dry_run { "Would prune" } else { "Pruned" },
        deleted.len(),
        if deleted.len() == 1 { "" } else { "s" }
    );
    for id in deleted {
        println!("- {id}");
    }
    Ok(())
}

fn delete_receipt(run_id: &str) -> Result<()> {
    let report = resolve_receipt(run_id)?;
    let deleted = delete_report(&report.run.id)?;
    println!("Deleted receipt {} at {}", report.run.id, deleted.display());
    Ok(())
}

fn write_share_bundle(report: &RunReport, base: &Path) -> Result<PathBuf> {
    let html_path = base.join("receipt.html");
    write_standalone_html(report, &html_path)?;
    let markdown_path = base.join("receipt.md");
    fs::write(&markdown_path, render_markdown_receipt(report))?;
    let json_path = base.join("receipt.json");
    fs::write(&json_path, serde_json::to_vec_pretty(report)?)?;
    let patch_path = base.join("receipt-reverse.patch");
    fs::write(&patch_path, render_reverse_patch(report)?)?;

    let bundle_path = base.join(format!("runglass-share-{}.tar", report.run.id));
    let mut bundle = fs::File::create(&bundle_path)?;
    append_tar_file(&mut bundle, "receipt.html", &html_path)?;
    append_tar_file(&mut bundle, "receipt.md", &markdown_path)?;
    append_tar_file(&mut bundle, "receipt.json", &json_path)?;
    append_tar_file(&mut bundle, "receipt-reverse.patch", &patch_path)?;
    append_tar_file(&mut bundle, "stdout.log", &base.join("stdout.log"))?;
    append_tar_file(&mut bundle, "stderr.log", &base.join("stderr.log"))?;
    append_tar_tree(&mut bundle, "file-artifacts", &base.join("file-artifacts"))?;
    bundle.write_all(&[0_u8; 1024])?;
    Ok(bundle_path)
}

fn append_tar_tree(writer: &mut fs::File, prefix: &str, dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let child_prefix = format!("{prefix}/{}", entry.file_name().to_string_lossy());
            append_tar_tree(writer, &child_prefix, &path)?;
        } else if path.is_file() {
            let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
            append_tar_file(writer, &name, &path)?;
        }
    }
    Ok(())
}

fn append_tar_file(writer: &mut fs::File, name: &str, path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let bytes = fs::read(path)?;
    let mut header = [0_u8; 512];
    write_tar_field(&mut header[0..100], name.as_bytes());
    write_octal(&mut header[100..108], 0o644);
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    write_octal(&mut header[124..136], bytes.len() as u64);
    write_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = b'0';
    write_tar_field(&mut header[257..263], b"ustar\0");
    write_tar_field(&mut header[263..265], b"00");
    let checksum: u32 = header.iter().map(|byte| *byte as u32).sum();
    write_checksum(&mut header[148..156], checksum);
    writer.write_all(&header)?;
    writer.write_all(&bytes)?;
    let padding = (512 - (bytes.len() % 512)) % 512;
    if padding > 0 {
        writer.write_all(&vec![0_u8; padding])?;
    }
    Ok(())
}

fn write_tar_field(field: &mut [u8], value: &[u8]) {
    let len = value.len().min(field.len());
    field[..len].copy_from_slice(&value[..len]);
}

fn write_octal(field: &mut [u8], value: u64) {
    let text = format!("{:0width$o}\0", value, width = field.len() - 1);
    write_tar_field(field, text.as_bytes());
}

fn write_checksum(field: &mut [u8], value: u32) {
    let text = format!("{:06o}\0 ", value);
    write_tar_field(field, text.as_bytes());
}

fn command_on_path(command: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|path| path.join(command).is_file()))
        .unwrap_or(false)
}

fn human_size(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

fn revert_receipt(
    run_id: &str,
    files: &[String],
    force: bool,
    skip_changed: bool,
    dry_run: bool,
) -> Result<()> {
    let report = resolve_receipt(run_id)?;
    let selected = (!files.is_empty()).then_some(files);
    let preview = preview_revert(&report, selected)?;
    print_revert_preview(&preview);

    if dry_run {
        return Ok(());
    }

    let policy = if force {
        RevertConflictPolicy::Force
    } else if skip_changed {
        RevertConflictPolicy::SkipChanged
    } else {
        RevertConflictPolicy::Abort
    };

    let result = apply_revert(&report, selected, RevertOptions { policy })?;
    println!();
    println!("Applied revert for receipt {}", report.run.id);
    print_revert_preview(&result);
    Ok(())
}

fn print_revert_preview(preview: &runglass_core::RevertPreview) {
    println!("Receipt: {}", preview.receipt_id);
    println!("Targets: {}", preview.target_count);
    println!(
        "Will restore {} modified, delete {} created, restore {} deleted",
        preview.restore_modified, preview.delete_created, preview.restore_deleted
    );
    if !preview.safe.is_empty() {
        println!("Safe: {}", preview.safe.len());
    }
    if !preview.already_reverted.is_empty() {
        println!("Already reverted: {}", preview.already_reverted.len());
    }
    if !preview.conflicts.is_empty() {
        println!("Changed since receipt: {}", preview.conflicts.len());
        for item in &preview.conflicts {
            println!("- {}: {}", item.path, item.detail);
        }
    }
    if !preview.missing_artifacts.is_empty() {
        println!(
            "Missing stored snapshots: {}",
            preview.missing_artifacts.len()
        );
        for item in &preview.missing_artifacts {
            println!("- {}: {}", item.path, item.detail);
        }
    }
}

fn resolve_receipt(selector: &str) -> Result<RunReport> {
    if selector == "latest" {
        return latest_report();
    }
    if selector.trim().is_empty() {
        return Err(anyhow!("receipt selector cannot be empty"));
    }
    load_report(selector)
}

#[cfg(test)]
mod tests {
    use super::{unsupported_platform_message, CiFormat, CiProvider, Cli, Commands};
    use clap::Parser;

    #[test]
    fn report_accepts_documented_open_flag() {
        let cli = Cli::try_parse_from(["runglass", "report", "latest", "--open"])
            .expect("parse report --open");

        let Commands::Report {
            open,
            no_open,
            port,
            ..
        } = cli.command
        else {
            panic!("expected report command");
        };
        assert!(open);
        assert!(!no_open);
        assert_eq!(port, 0);
    }

    #[test]
    fn report_accepts_no_open_for_headless_use() {
        let cli = Cli::try_parse_from(["runglass", "report", "latest", "--no-open"])
            .expect("parse report --no-open");

        let Commands::Report { open, no_open, .. } = cli.command else {
            panic!("expected report command");
        };
        assert!(!open);
        assert!(no_open);
    }

    #[test]
    fn open_defaults_to_latest_receipt() {
        let cli = Cli::try_parse_from(["runglass", "open"]).expect("parse open");

        let Commands::Open { run_id, port } = cli.command else {
            panic!("expected open command");
        };
        assert_eq!(run_id, "latest");
        assert_eq!(port, 0);
    }

    #[test]
    fn open_accepts_receipt_id_and_port() {
        let cli = Cli::try_parse_from(["runglass", "open", "abc123", "--port", "9876"])
            .expect("parse open id --port");

        let Commands::Open { run_id, port } = cli.command else {
            panic!("expected open command");
        };
        assert_eq!(run_id, "abc123");
        assert_eq!(port, 9876);
    }

    #[test]
    fn run_accepts_command_without_double_dash() {
        let cli = Cli::try_parse_from(["runglass", "run", "docker", "compose", "up", "-d"])
            .expect("parse no-dash run command");

        let Commands::Run { command, .. } = cli.command else {
            panic!("expected run command");
        };
        assert_eq!(command, vec!["docker", "compose", "up", "-d"]);
    }

    #[test]
    fn run_keeps_double_dash_for_unambiguous_wrapping() {
        let cli = Cli::try_parse_from([
            "runglass",
            "run",
            "--deep",
            "--",
            "cargo",
            "test",
            "--",
            "--nocapture",
        ])
        .expect("parse dashed run command");

        let Commands::Run { deep, command, .. } = cli.command else {
            panic!("expected run command");
        };
        assert!(deep);
        assert_eq!(command, vec!["cargo", "test", "--", "--nocapture"]);
    }

    #[test]
    fn unsupported_platform_message_is_clear() {
        let message = unsupported_platform_message();
        assert!(message.contains("Linux-first"));
        assert!(message.contains("not supported yet"));
    }

    #[test]
    fn ci_accepts_provider_output_formats_and_command() {
        let cli = Cli::try_parse_from([
            "runglass",
            "ci",
            "--provider",
            "github",
            "--out",
            "receipt-out",
            "--format",
            "html,json",
            "--",
            "cargo",
            "test",
        ])
        .expect("parse ci command");

        let Commands::Ci {
            provider,
            out,
            formats,
            command,
            ..
        } = cli.command
        else {
            panic!("expected ci command");
        };
        assert!(matches!(provider, CiProvider::Github));
        assert_eq!(out, std::path::PathBuf::from("receipt-out"));
        assert_eq!(formats, vec![CiFormat::Html, CiFormat::Json]);
        assert_eq!(command, vec!["cargo", "test"]);
    }
}
