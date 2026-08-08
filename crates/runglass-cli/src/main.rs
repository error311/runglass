use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use runglass_core::render_ai_receipt_summary;
use runglass_core::{
    apply_revert, delete_report, fixture, latest_report, list_reports, load_report, make_run_id,
    prepare_run_paths, preview_revert, prune_reports, render_markdown_receipt,
    render_reverse_patch, render_summary_markdown_receipt, report_run_dir, reports_dir,
    run_observed_command_in_mode, snapshot_directory_with_stats, snapshot_file_byte_limit,
    write_report_bundle, CiMetadata, FileChangeType, ObservationMode, RevertConflictPolicy,
    RevertOptions, RiskLevel, RunReport, RunStatus,
};
use runglass_web::{serve_report, serve_report_on_port, write_standalone_html};

mod github;

const UNSUPPORTED_PLATFORM_MESSAGE: &str = "RunGlass live command observation is Linux-first in this release.\nThis platform can inspect, export, and validate existing receipts, but `runglass run` and `runglass ci` are not supported yet.";

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
        after_help = "RunGlass wraps one command and creates a receipt.\n\nExamples:\n  runglass run -- docker compose up -d\n  runglass run docker compose up -d\n  runglass run --deep -- ./install.sh\n  runglass run --review -- ./install.sh"
    )]
    Run {
        #[arg(long)]
        open: bool,
        #[arg(long)]
        deep: bool,
        #[arg(
            long,
            help = "After the command exits, guide keep/revert/export/open actions"
        )]
        review: bool,
        #[arg(
            long = "non-interactive",
            value_enum,
            default_value_t = ReviewNonInteractive::Fail,
            help = "Review-mode behavior when stdin/stdout is not interactive"
        )]
        non_interactive: ReviewNonInteractive,
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
    #[command(
        about = "Review a saved receipt and choose keep/revert/export/open actions",
        after_help = "Examples:\n  runglass review latest\n  runglass review latest --non-interactive summary\n  runglass review <receipt-id>"
    )]
    Review {
        #[arg(default_value = "latest")]
        receipt: String,
        #[arg(
            long = "non-interactive",
            value_enum,
            default_value_t = ReviewNonInteractive::Fail,
            help = "Review behavior when stdin/stdout is not interactive"
        )]
        non_interactive: ReviewNonInteractive,
    },
    #[command(about = "Run one command in CI and write receipt artifacts")]
    Ci {
        #[arg(long)]
        deep: bool,
        #[arg(long, value_enum, default_value_t = CiProvider::Auto)]
        provider: CiProvider,
        #[arg(long, visible_alias = "output", default_value = "runglass-receipt")]
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
        #[arg(long, help = "Print a compact AI-friendly receipt summary")]
        ai: bool,
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
        #[arg(long = "format", value_enum, value_delimiter = ',')]
        formats: Vec<ExportFormat>,
    },
    #[command(about = "Check local collector readiness and receipt storage")]
    Doctor,
    #[command(about = "Validate a saved receipt or CI receipt directory")]
    Validate {
        #[arg(default_value = "latest")]
        receipt: String,
    },
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
    #[command(
        subcommand,
        about = "Detect GitHub context and post PR receipt comments"
    )]
    Github(GithubCommands),
    #[command(about = "Post or preview a compact RunGlass receipt comment on a pull request")]
    PrComment(GithubCommentArgs),
    Revert {
        run_id: String,
        #[arg(long = "file")]
        files: Vec<String>,
        #[arg(long, help = "Apply the supported file revert after previewing it")]
        apply: bool,
        #[arg(
            long,
            help = "Preview the supported file revert without changing files"
        )]
        preview: bool,
        #[arg(long)]
        force: bool,
        #[arg(long = "skip-changed")]
        skip_changed: bool,
        #[arg(long = "dry-run", help = "Alias for --preview")]
        dry_run: bool,
    },
    Demo {
        #[arg(long)]
        open: bool,
    },
}

#[derive(Debug, Subcommand)]
enum GithubCommands {
    Detect {
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        pr: Option<u64>,
    },
    Comment(GithubCommentArgs),
}

#[derive(Debug, Clone, clap::Args)]
struct GithubCommentArgs {
    #[arg(long, default_value = "latest")]
    receipt: String,
    #[arg(long)]
    repo: Option<String>,
    #[arg(long)]
    pr: Option<u64>,
    #[arg(long)]
    auto: bool,
    #[arg(long = "dry-run")]
    dry_run: bool,
    #[arg(long)]
    api_url: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ExportFormat {
    Html,
    Json,
    Markdown,
    #[value(name = "reverse-patch")]
    ReversePatch,
    Bundle,
    #[value(name = "summary-md")]
    SummaryMd,
    Ai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ReviewNonInteractive {
    Fail,
    Summary,
}

enum ReviewAction {
    Keep,
    PreviewRevert,
    RevertSupported,
    Export,
    Open,
}

struct CiArtifact {
    label: &'static str,
    path: PathBuf,
}

struct ExportSelection {
    html: bool,
    json: bool,
    markdown: bool,
    reverse_patch: bool,
    bundle: bool,
    formats: Vec<ExportFormat>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            open,
            deep,
            review,
            non_interactive,
            command,
        } => run_command(command, open, deep, review, non_interactive),
        Commands::Open { run_id, port } => open_receipt(&run_id, port),
        Commands::Review {
            receipt,
            non_interactive,
        } => review_receipt_command(&receipt, non_interactive),
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
            ai,
            open,
            no_open,
            port,
        } => {
            let report = resolve_receipt(&run_id)?;
            if print_json && ai {
                Err(anyhow!("use either --print-json or --ai, not both"))
            } else if print_json {
                println!("{}", serde_json::to_string_pretty(&report)?);
                Ok(())
            } else if ai {
                print!("{}", render_ai_receipt_summary(&report));
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
            formats,
        } => export_report(
            &run_id,
            ExportSelection {
                html,
                json,
                markdown,
                reverse_patch,
                bundle,
                formats,
            },
        ),
        Commands::Doctor => doctor(),
        Commands::Validate { receipt } => validate_receipt_command(&receipt),
        Commands::Snapshot { dry_run } => snapshot_dry_run(dry_run),
        Commands::Prune { keep, dry_run } => prune_receipts(keep, dry_run),
        Commands::Delete { run_id } => delete_receipt(&run_id),
        Commands::Github(command) => match command {
            GithubCommands::Detect { repo, pr } => github::detect_command(repo, pr),
            GithubCommands::Comment(args) => github_comment_command(args),
        },
        Commands::PrComment(args) => github_comment_command(args),
        Commands::Revert {
            run_id,
            files,
            apply,
            preview,
            force,
            skip_changed,
            dry_run,
        } => revert_receipt(
            &run_id,
            &files,
            apply,
            preview || dry_run,
            force,
            skip_changed,
        ),
        Commands::Demo { open } => run_demo(open),
    }
}

fn github_comment_command(args: GithubCommentArgs) -> Result<()> {
    let report = resolve_receipt_input(&args.receipt)?;
    let options = github::GithubCommentOptions {
        repo: args.repo,
        pr: args.pr,
        auto: args.auto,
        dry_run: args.dry_run,
        api_url: args.api_url,
    };
    github::comment_command(&report, options)
}

fn run_command(
    command: Vec<String>,
    open: bool,
    deep: bool,
    review: bool,
    non_interactive: ReviewNonInteractive,
) -> Result<()> {
    ensure_observation_supported()?;
    if !review {
        if matches!(non_interactive, ReviewNonInteractive::Summary) {
            return Err(anyhow!("--non-interactive is only valid with --review"));
        }
        return run_plain_command(command, open, deep);
    }

    run_review_command(command, open, deep, non_interactive)
}

fn run_plain_command(command: Vec<String>, open: bool, deep: bool) -> Result<()> {
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

fn run_review_command(
    command: Vec<String>,
    open: bool,
    deep: bool,
    non_interactive: ReviewNonInteractive,
) -> Result<()> {
    let interactive = review_stdio_is_interactive();
    if !interactive && matches!(non_interactive, ReviewNonInteractive::Fail) {
        return Err(anyhow!(
            "review mode requires an interactive stdin/stdout; use `--non-interactive summary` to run and print a review summary without prompts"
        ));
    }

    let mode = if deep {
        ObservationMode::Deep
    } else {
        ObservationMode::Normal
    };
    let (report, paths) = run_observed_command_in_mode(command, mode)?;

    println!("Created receipt {}", report.run.id);
    println!("{}", paths.report_path.display());
    println!();
    print_review_summary(&report)?;

    if open {
        serve_report(report.clone(), true)?;
    }

    if !interactive {
        return finish_review(&report, true);
    }

    review_loop(report, true)
}

fn review_stdio_is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn review_receipt_command(selector: &str, non_interactive: ReviewNonInteractive) -> Result<()> {
    let interactive = review_stdio_is_interactive();
    if !interactive && matches!(non_interactive, ReviewNonInteractive::Fail) {
        return Err(anyhow!(
            "review requires an interactive stdin/stdout; use `--non-interactive summary` to print a saved receipt review summary without prompts"
        ));
    }

    let report = resolve_receipt(selector)?;
    print_review_summary(&report)?;

    if !interactive {
        return Ok(());
    }

    review_loop(report, false)
}

fn review_loop(report: RunReport, preserve_command_status: bool) -> Result<()> {
    loop {
        match prompt_review_action()? {
            ReviewAction::Keep => return finish_review(&report, preserve_command_status),
            ReviewAction::PreviewRevert => {
                let preview = preview_revert(&report, None)?;
                print_revert_preview(&preview);
            }
            ReviewAction::RevertSupported => {
                let preview = preview_revert(&report, None)?;
                print_revert_preview(&preview);
                if !confirm_review_revert()? {
                    println!("Revert cancelled.");
                    continue;
                }
                let result = apply_revert(
                    &report,
                    None,
                    RevertOptions {
                        policy: RevertConflictPolicy::Abort,
                    },
                )?;
                println!("Applied supported file revert.");
                print_revert_preview(&result);
                return finish_review(&report, preserve_command_status);
            }
            ReviewAction::Export => {
                println!("Exporting receipt artifacts:");
                export_report(
                    &report.run.id,
                    ExportSelection {
                        html: true,
                        json: true,
                        markdown: true,
                        reverse_patch: true,
                        bundle: true,
                        formats: vec![ExportFormat::SummaryMd, ExportFormat::Ai],
                    },
                )?;
            }
            ReviewAction::Open => {
                serve_report(report.clone(), true)?;
                return finish_review(&report, preserve_command_status);
            }
        }
        println!();
    }
}

fn prompt_review_action() -> Result<ReviewAction> {
    loop {
        println!();
        println!("Review actions:");
        println!("  k  keep changes and finish");
        println!("  p  preview supported file revert");
        println!("  r  revert supported file changes");
        println!("  e  export receipt artifacts");
        println!("  o  open receipt UI");
        print!("Choose action [k/p/r/e/o]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim().to_ascii_lowercase().as_str() {
            "" | "k" | "keep" => return Ok(ReviewAction::Keep),
            "p" | "preview" => return Ok(ReviewAction::PreviewRevert),
            "r" | "revert" => return Ok(ReviewAction::RevertSupported),
            "e" | "export" => return Ok(ReviewAction::Export),
            "o" | "open" => return Ok(ReviewAction::Open),
            _ => println!("Unknown review action."),
        }
    }
}

fn confirm_review_revert() -> Result<bool> {
    print!("Type `revert` to apply supported file changes: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim() == "revert")
}

fn finish_review(report: &RunReport, preserve_command_status: bool) -> Result<()> {
    if preserve_command_status {
        if let Some(code) = report.run.exit_code {
            if code != 0 {
                process::exit(normalize_exit_code(code));
            }
        } else if !matches!(report.run.status, RunStatus::Completed) {
            process::exit(1);
        }
    }
    Ok(())
}

fn print_review_summary(report: &RunReport) -> Result<()> {
    println!("RunGlass Review");
    println!("Receipt: {}", report.run.id);
    println!("Command: {}", report.run.command_display);
    println!(
        "Status: {}, exit {}",
        status_label(&report.run.status),
        report
            .run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!("Risk: {}", risk_level_label(&report.summary.risk_level));
    println!();
    println!("Impact");
    println!(
        "- Files: {} created, {} modified, {} deleted",
        report.summary.files_created, report.summary.files_modified, report.summary.files_deleted
    );
    println!(
        "- Runtime: {} processes, {} outbound hosts, {} listening ports",
        report.summary.processes_seen, report.summary.network_hosts, report.summary.ports_opened
    );
    println!(
        "- Docker: {} containers, {} images, {} volumes",
        report.summary.docker_containers_created,
        report.summary.docker_images_pulled,
        report.summary.docker_volumes_created
    );

    if !report.files.is_empty() {
        println!();
        println!("Changed files");
        for file in report.files.iter().take(12) {
            println!(
                "- {} {}",
                file_change_type_label(&file.change_type),
                file.path
            );
        }
        if report.files.len() > 12 {
            println!("- ... {} more", report.files.len() - 12);
        }
    }

    if !report.risks.is_empty() {
        println!();
        println!("Risk notes");
        for risk in report.risks.iter().take(5) {
            println!("- {}: {}", risk.title, risk.detail);
        }
        if report.risks.len() > 5 {
            println!("- ... {} more", report.risks.len() - 5);
        }
    }

    let preview = preview_revert(report, None)?;
    println!();
    println!("Supported file revert");
    println!(
        "- Targets: {} safe, {} changed since receipt, {} missing snapshots, {} already reverted",
        preview.safe.len(),
        preview.conflicts.len(),
        preview.missing_artifacts.len(),
        preview.already_reverted.len()
    );
    println!(
        "- Plan: restore {} modified, delete {} created, restore {} deleted",
        preview.restore_modified, preview.delete_created, preview.restore_deleted
    );
    println!(
        "- Non-file side effects such as Docker changes, network calls, databases, package-manager globals, and external services are not undone."
    );

    Ok(())
}

fn file_change_type_label(change_type: &FileChangeType) -> &'static str {
    match change_type {
        FileChangeType::Created => "created",
        FileChangeType::Modified => "modified",
        FileChangeType::Deleted => "deleted",
    }
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
    let (mut report, _paths) = run_observed_command_in_mode(command, mode)?;
    report.ci = Some(build_ci_metadata(provider, out));
    let artifacts = write_ci_artifacts(&report, provider, out, formats)?;
    let summary = render_ci_summary(&report, provider, &artifacts);

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
    provider: CiProvider,
    out: &Path,
    formats: &[CiFormat],
) -> Result<Vec<CiArtifact>> {
    fs::create_dir_all(out)?;
    let artifacts_dir = out.join("artifacts");
    fs::create_dir_all(&artifacts_dir)?;
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

    let patch_path = out.join("reverse.patch");
    fs::write(&patch_path, render_reverse_patch(report)?)?;
    artifacts.push(CiArtifact {
        label: "Reverse patch",
        path: patch_path,
    });

    let stdout_path = artifacts_dir.join("stdout.txt");
    fs::write(
        &stdout_path,
        report_output_text(report.stdout.as_deref(), &report.stdout_path)?,
    )?;
    artifacts.push(CiArtifact {
        label: "stdout",
        path: stdout_path,
    });

    let stderr_path = artifacts_dir.join("stderr.txt");
    fs::write(
        &stderr_path,
        report_output_text(report.stderr.as_deref(), &report.stderr_path)?,
    )?;
    artifacts.push(CiArtifact {
        label: "stderr",
        path: stderr_path,
    });

    let diffs_dir = artifacts_dir.join("diffs");
    if diffs_dir.exists() {
        fs::remove_dir_all(&diffs_dir)?;
    }
    fs::create_dir_all(&diffs_dir)?;
    let diff_count = write_ci_diff_artifacts(report, &diffs_dir)?;
    artifacts.push(CiArtifact {
        label: if diff_count == 0 {
            "Diff directory (empty)"
        } else {
            "Diff directory"
        },
        path: diffs_dir,
    });

    let snapshots_dir = artifacts_dir.join("file-snapshots");
    if snapshots_dir.exists() {
        fs::remove_dir_all(&snapshots_dir)?;
    }
    copy_ci_file_snapshots(report, &snapshots_dir)?;
    artifacts.push(CiArtifact {
        label: "File snapshots",
        path: snapshots_dir,
    });

    let metadata_path = artifacts_dir.join("metadata.json");
    fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "runglass-ci-artifacts-v1",
            "receipt_id": report.run.id,
            "provider": ci_provider_label(provider),
            "command": report.run.command_display,
            "status": report.run.status,
            "exit_code": report.run.exit_code,
            "generated_at": Utc::now(),
            "ci": report.ci,
            "contents": [
                "receipt.html",
                "receipt.md",
                "receipt.json",
                "summary.md",
                "ai-summary.txt",
                "reverse.patch",
                "bundle.tar",
                "artifacts/stdout.txt",
                "artifacts/stderr.txt",
                "artifacts/diffs/",
                "artifacts/file-snapshots/"
            ]
        }))?,
    )?;
    artifacts.push(CiArtifact {
        label: "CI metadata",
        path: metadata_path,
    });

    let summary_path = out.join("summary.md");
    artifacts.push(CiArtifact {
        label: "CI summary",
        path: summary_path.clone(),
    });
    let ai_summary_path = out.join("ai-summary.txt");
    artifacts.push(CiArtifact {
        label: "AI summary",
        path: ai_summary_path.clone(),
    });
    let bundle_path = out.join("bundle.tar");
    artifacts.push(CiArtifact {
        label: "Receipt bundle",
        path: bundle_path.clone(),
    });

    let summary = render_ci_summary(report, provider, &artifacts);
    fs::write(&summary_path, &summary)?;
    fs::write(&ai_summary_path, render_ai_receipt_summary(report))?;
    write_ci_bundle(report, out, &bundle_path)?;

    Ok(artifacts)
}

fn build_ci_metadata(provider: CiProvider, out: &Path) -> CiMetadata {
    let provider_label = ci_provider_label(provider).to_string();
    let artifact_name = out
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("runglass-receipt")
        .to_string();
    let (repository, pull_request, commit_sha, run_url) = match provider {
        CiProvider::Github => (
            std::env::var("GITHUB_REPOSITORY").ok(),
            github_pull_request_number(),
            std::env::var("GITHUB_SHA").ok(),
            github_run_url(),
        ),
        CiProvider::Gitlab => (
            std::env::var("CI_PROJECT_PATH").ok(),
            std::env::var("CI_MERGE_REQUEST_IID")
                .ok()
                .and_then(|value| value.parse().ok()),
            std::env::var("CI_COMMIT_SHA").ok(),
            std::env::var("CI_PIPELINE_URL").ok(),
        ),
        CiProvider::Auto | CiProvider::Generic => (None, None, None, None),
    };

    CiMetadata {
        provider: provider_label,
        repository,
        pull_request,
        commit_sha,
        run_url,
        artifact_name: Some(artifact_name),
        artifact_path: Some(out.display().to_string()),
    }
}

fn ci_provider_label(provider: CiProvider) -> &'static str {
    match provider {
        CiProvider::Auto => "auto",
        CiProvider::Github => "github",
        CiProvider::Gitlab => "gitlab",
        CiProvider::Generic => "generic",
    }
}

fn github_run_url() -> Option<String> {
    let server =
        std::env::var("GITHUB_SERVER_URL").unwrap_or_else(|_| "https://github.com".to_string());
    let repository = std::env::var("GITHUB_REPOSITORY").ok()?;
    let run_id = std::env::var("GITHUB_RUN_ID").ok()?;
    Some(format!("{server}/{repository}/actions/runs/{run_id}"))
}

fn github_pull_request_number() -> Option<u64> {
    std::env::var("GITHUB_REF")
        .ok()
        .and_then(|value| {
            let mut parts = value.split('/');
            match (parts.next(), parts.next(), parts.next()) {
                (Some("refs"), Some("pull"), Some(number)) => number.parse().ok(),
                _ => None,
            }
        })
        .or_else(|| {
            let path = std::env::var("GITHUB_EVENT_PATH").ok()?;
            let data = fs::read_to_string(path).ok()?;
            let value: serde_json::Value = serde_json::from_str(&data).ok()?;
            value
                .pointer("/pull_request/number")
                .and_then(|number| number.as_u64())
                .or_else(|| value.pointer("/number").and_then(|number| number.as_u64()))
        })
}

fn report_output_text(inline: Option<&str>, path: &Option<String>) -> Result<String> {
    if let Some(value) = inline {
        return Ok(value.to_string());
    }
    if let Some(path) = path {
        let path = Path::new(path);
        if path.exists() {
            return Ok(fs::read_to_string(path)?);
        }
    }
    Ok(String::new())
}

fn write_ci_diff_artifacts(report: &RunReport, diffs_dir: &Path) -> Result<usize> {
    let mut count = 0;
    for file in &report.files {
        let Some(diff) = &file.diff else {
            continue;
        };
        if diff.content.trim().is_empty() {
            continue;
        }
        count += 1;
        let name = format!("{count:03}_{}.diff", slug_for_artifact(&file.path));
        fs::write(diffs_dir.join(name), &diff.content)?;
    }
    Ok(count)
}

fn slug_for_artifact(value: &str) -> String {
    let mut slug = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
            slug.push(ch);
        } else {
            slug.push('-');
        }
        if slug.len() >= 80 {
            break;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "file".to_string()
    } else {
        slug
    }
}

fn copy_ci_file_snapshots(report: &RunReport, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    let source = report_run_dir(&report.run.id)?.join("file-artifacts");
    if source.exists() {
        copy_dir_contents(&source, target)?;
    }
    Ok(())
}

fn copy_dir_contents(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_contents(&source_path, &target_path)?;
        } else if source_path.is_file() {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn write_ci_bundle(report: &RunReport, out: &Path, bundle_path: &Path) -> Result<()> {
    let bundle_root = format!("runglass-receipt-{}", report.run.id);
    let mut bundle = fs::File::create(bundle_path)?;
    for relative in [
        "receipt.html",
        "receipt.md",
        "summary.md",
        "ai-summary.txt",
        "receipt.json",
        "reverse.patch",
        "artifacts/stdout.txt",
        "artifacts/stderr.txt",
        "artifacts/metadata.json",
    ] {
        append_tar_file(
            &mut bundle,
            &format!("{bundle_root}/{relative}"),
            &out.join(relative),
        )?;
    }
    append_tar_tree(
        &mut bundle,
        &format!("{bundle_root}/artifacts/diffs"),
        &out.join("artifacts/diffs"),
    )?;
    append_tar_tree(
        &mut bundle,
        &format!("{bundle_root}/artifacts/file-snapshots"),
        &out.join("artifacts/file-snapshots"),
    )?;
    bundle.write_all(&[0_u8; 1024])?;
    Ok(())
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

fn export_report(run_id: &str, selection: ExportSelection) -> Result<()> {
    let report = resolve_receipt(run_id)?;
    let base = report_run_dir(&report.run.id)?;
    let html = selection.html || selection.formats.contains(&ExportFormat::Html);
    let json = selection.json || selection.formats.contains(&ExportFormat::Json);
    let markdown = selection.markdown || selection.formats.contains(&ExportFormat::Markdown);
    let reverse_patch =
        selection.reverse_patch || selection.formats.contains(&ExportFormat::ReversePatch);
    let bundle = selection.bundle || selection.formats.contains(&ExportFormat::Bundle);
    let summary_md = selection.formats.contains(&ExportFormat::SummaryMd);
    let ai = selection.formats.contains(&ExportFormat::Ai);

    if html || (!json && !markdown && !reverse_patch && !bundle && !summary_md && !ai) {
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
        let patch_path = base.join("reverse.patch");
        fs::write(&patch_path, render_reverse_patch(&report)?)?;
        println!("{}", patch_path.display());
    }

    if bundle {
        let bundle_path = write_share_bundle(&report, &base)?;
        println!("{}", bundle_path.display());
    }

    if summary_md {
        let summary_path = base.join("summary.md");
        fs::write(&summary_path, render_summary_markdown_receipt(&report))?;
        println!("{}", summary_path.display());
    }

    if ai {
        let ai_path = base.join("ai-summary.txt");
        fs::write(&ai_path, render_ai_receipt_summary(&report))?;
        println!("{}", ai_path.display());
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
    print_check("Architecture", std::env::consts::ARCH, true);
    print_check(
        "Live observation",
        if platform_supported {
            "available"
        } else {
            "Linux-only in this release"
        },
        platform_supported,
    );
    print_check("Inspect/export/validate", "available", true);
    if !platform_supported {
        println!();
        println!("{}", unsupported_platform_message());
    }
    let cwd = std::env::current_dir()?;
    print_check(
        "Working directory",
        &cwd.display().to_string(),
        cwd.exists(),
    );
    print_check(
        "Working directory writable",
        "file snapshot scope",
        dir_writable(&cwd),
    );
    let reports = reports_dir()?;
    print_check("Reports directory", &reports.display().to_string(), true);
    print_check(
        "Reports directory writable",
        "receipt storage",
        dir_writable(&reports),
    );
    print_check(
        "Snapshot cap",
        &human_size(snapshot_file_byte_limit()),
        snapshot_file_byte_limit() > 0,
    );
    print_check(
        "Shell",
        &std::env::var("SHELL").unwrap_or_else(|_| "not detected".to_string()),
        std::env::var_os("SHELL").is_some(),
    );
    print_check("git", "repository context helper", command_on_path("git"));
    if platform_supported {
        print_check("ss", "socket sampling helper", command_on_path("ss"));
        print_check(
            "strace",
            "deep mode tracing helper",
            command_on_path("strace"),
        );
    }
    print_check(
        "docker",
        if platform_supported {
            "Docker diff support"
        } else {
            "detected for future live observation support"
        },
        command_on_path("docker"),
    );
    println!();
    if platform_supported {
        println!(
            "Tip: run `runglass snapshot --dry-run` inside a project to preview file capture scope."
        );
    } else {
        println!(
            "Tip: this platform can inspect, export, and validate existing receipts; live `run` and `ci` remain Linux-first."
        );
    }
    Ok(())
}

fn validate_receipt_command(selector: &str) -> Result<()> {
    let input = load_receipt_for_validation(selector)?;
    let result = validate_report(&input.report, input.base_dir.as_deref());

    println!("RunGlass Receipt Validation");
    println!("Source: {}", input.source);
    println!("Receipt: {}", input.report.run.id);
    println!("Command: {}", input.report.run.command_display);
    println!(
        "Status: {}, exit {}",
        status_label(&input.report.run.status),
        input
            .report
            .run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!();

    if result.errors.is_empty() {
        println!("ok\tReceipt JSON parsed and required metadata is present");
    } else {
        println!("error\t{} validation error(s)", result.errors.len());
        for error in &result.errors {
            println!("- {error}");
        }
    }

    if result.warnings.is_empty() {
        println!("ok\tNo artifact or revert warnings");
    } else {
        println!("warn\t{} validation warning(s)", result.warnings.len());
        for warning in &result.warnings {
            println!("- {warning}");
        }
    }

    if result.errors.is_empty() {
        println!();
        println!(
            "Validation passed{}.",
            if result.warnings.is_empty() {
                "".to_string()
            } else {
                format!(" with {} warning(s)", result.warnings.len())
            }
        );
        Ok(())
    } else {
        Err(anyhow!(
            "receipt validation failed with {} error(s)",
            result.errors.len()
        ))
    }
}

struct ValidationInput {
    report: RunReport,
    base_dir: Option<PathBuf>,
    source: String,
}

#[derive(Default)]
struct ValidationResult {
    errors: Vec<String>,
    warnings: Vec<String>,
}

fn load_receipt_for_validation(selector: &str) -> Result<ValidationInput> {
    if selector.trim().is_empty() {
        return Err(anyhow!("receipt selector cannot be empty"));
    }

    let path = Path::new(selector);
    if path.is_dir() {
        let receipt_path = path.join("receipt.json");
        let report = read_receipt_json(&receipt_path)?;
        return Ok(ValidationInput {
            report,
            base_dir: Some(path.to_path_buf()),
            source: receipt_path.display().to_string(),
        });
    }
    if path.is_file() {
        let report = read_receipt_json(path)?;
        return Ok(ValidationInput {
            report,
            base_dir: path.parent().map(Path::to_path_buf),
            source: path.display().to_string(),
        });
    }

    let report = resolve_receipt(selector)?;
    let base_dir = report_run_dir(&report.run.id).ok();
    Ok(ValidationInput {
        source: selector.to_string(),
        report,
        base_dir,
    })
}

fn read_receipt_json(path: &Path) -> Result<RunReport> {
    let data = fs::read_to_string(path)
        .map_err(|error| anyhow!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&data).map_err(|error| {
        anyhow!(
            "failed to parse receipt JSON at {}: {error}",
            path.display()
        )
    })
}

fn validate_report(report: &RunReport, base_dir: Option<&Path>) -> ValidationResult {
    let mut result = ValidationResult::default();

    if report.schema_version.trim().is_empty() {
        result.errors.push("schema_version is empty".to_string());
    }
    if report.run.id.trim().is_empty() {
        result.errors.push("run.id is empty".to_string());
    }
    if report.run.command_display.trim().is_empty() {
        result
            .errors
            .push("run.command_display is empty".to_string());
    }
    if report.run.cwd.trim().is_empty() {
        result.errors.push("run.cwd is empty".to_string());
    }

    let file_total =
        report.summary.files_created + report.summary.files_modified + report.summary.files_deleted;
    if report.summary.files_changed != file_total {
        result.warnings.push(format!(
            "summary.files_changed is {}, but created+modified+deleted is {}",
            report.summary.files_changed, file_total
        ));
    }
    if report.summary.files_changed != report.files.len() {
        result.warnings.push(format!(
            "summary.files_changed is {}, but the receipt lists {} file change(s)",
            report.summary.files_changed,
            report.files.len()
        ));
    }
    if report.limitations.is_empty() {
        result
            .warnings
            .push("receipt has no fidelity or snapshot limitation notes".to_string());
    }

    validate_output_artifact(
        base_dir,
        &mut result,
        "stdout",
        report.stdout.as_deref(),
        report.stdout_path.as_deref(),
    );
    validate_output_artifact(
        base_dir,
        &mut result,
        "stderr",
        report.stderr.as_deref(),
        report.stderr_path.as_deref(),
    );
    validate_revert_artifacts(report, base_dir, &mut result);
    validate_ci_artifacts(report, base_dir, &mut result);

    result
}

fn validate_output_artifact(
    base_dir: Option<&Path>,
    result: &mut ValidationResult,
    label: &str,
    inline: Option<&str>,
    path: Option<&str>,
) {
    if inline.is_some() {
        return;
    }
    let Some(path) = path else {
        result
            .warnings
            .push(format!("{label} was not stored inline and has no path"));
        return;
    };
    if !artifact_exists(base_dir, path) {
        result
            .warnings
            .push(format!("{label} path is missing or not included: {path}"));
    }
}

fn validate_revert_artifacts(
    report: &RunReport,
    base_dir: Option<&Path>,
    result: &mut ValidationResult,
) {
    for file in &report.files {
        match file.change_type {
            FileChangeType::Modified | FileChangeType::Deleted => {
                validate_file_artifact(
                    base_dir,
                    result,
                    &file.path,
                    "before-run snapshot",
                    file.before_artifact_path.as_deref(),
                );
            }
            FileChangeType::Created => {
                validate_file_artifact(
                    base_dir,
                    result,
                    &file.path,
                    "after-run snapshot",
                    file.after_artifact_path.as_deref(),
                );
            }
        }
    }
}

fn validate_file_artifact(
    base_dir: Option<&Path>,
    result: &mut ValidationResult,
    file_path: &str,
    label: &str,
    artifact: Option<&str>,
) {
    let Some(artifact) = artifact else {
        result.warnings.push(format!(
            "{file_path} has no {label}; supported revert may be limited"
        ));
        return;
    };
    if !artifact_exists(base_dir, artifact) {
        result.warnings.push(format!(
            "{file_path} references missing {label}: {artifact}"
        ));
    }
}

fn validate_ci_artifacts(
    report: &RunReport,
    base_dir: Option<&Path>,
    result: &mut ValidationResult,
) {
    let Some(base_dir) = base_dir else {
        return;
    };
    let looks_like_ci_output = report.ci.is_some() || base_dir.join("artifacts").exists();
    if !looks_like_ci_output {
        return;
    }

    for relative in [
        "receipt.json",
        "summary.md",
        "ai-summary.txt",
        "reverse.patch",
        "bundle.tar",
        "artifacts/stdout.txt",
        "artifacts/stderr.txt",
        "artifacts/metadata.json",
    ] {
        if !base_dir.join(relative).exists() {
            result
                .warnings
                .push(format!("CI artifact is missing: {relative}"));
        }
    }
    for relative in ["artifacts/diffs", "artifacts/file-snapshots"] {
        if !base_dir.join(relative).is_dir() {
            result
                .warnings
                .push(format!("CI artifact directory is missing: {relative}"));
        }
    }
}

fn artifact_exists(base_dir: Option<&Path>, artifact: &str) -> bool {
    let path = Path::new(artifact);
    if path.exists() {
        return true;
    }
    let Some(base_dir) = base_dir else {
        return false;
    };
    if base_dir.join(path).exists() {
        return true;
    }
    if let Some(rest) = artifact.strip_prefix("file-artifacts/") {
        return base_dir
            .join("artifacts/file-snapshots")
            .join(rest)
            .exists();
    }
    false
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
    let summary_path = base.join("summary.md");
    fs::write(&summary_path, render_summary_markdown_receipt(report))?;
    let ai_summary_path = base.join("ai-summary.txt");
    fs::write(&ai_summary_path, render_ai_receipt_summary(report))?;
    let json_path = base.join("receipt.json");
    fs::write(&json_path, serde_json::to_vec_pretty(report)?)?;
    let patch_path = base.join("reverse.patch");
    fs::write(&patch_path, render_reverse_patch(report)?)?;
    let metadata_path = base.join("bundle-metadata.json");
    fs::write(
        &metadata_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "bundle_schema": "runglass-receipt-bundle-v1",
            "receipt_id": report.run.id,
            "command": report.run.command_display,
            "status": report.run.status,
            "created_at": Utc::now(),
            "contents": [
                "receipt.html",
                "receipt.md",
                "summary.md",
                "ai-summary.txt",
                "receipt.json",
                "reverse.patch",
                "artifacts/stdout.txt",
                "artifacts/stderr.txt",
                "artifacts/file-snapshots/"
            ]
        }))?,
    )?;

    let bundle_root = format!("runglass-receipt-{}", report.run.id);
    let bundle_path = base.join(format!("{bundle_root}.tar"));
    let mut bundle = fs::File::create(&bundle_path)?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/receipt.html"),
        &html_path,
    )?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/receipt.md"),
        &markdown_path,
    )?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/summary.md"),
        &summary_path,
    )?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/ai-summary.txt"),
        &ai_summary_path,
    )?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/receipt.json"),
        &json_path,
    )?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/reverse.patch"),
        &patch_path,
    )?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/artifacts/stdout.txt"),
        &base.join("stdout.log"),
    )?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/artifacts/stderr.txt"),
        &base.join("stderr.log"),
    )?;
    append_tar_file(
        &mut bundle,
        &format!("{bundle_root}/artifacts/metadata.json"),
        &metadata_path,
    )?;
    append_tar_tree(
        &mut bundle,
        &format!("{bundle_root}/artifacts/file-snapshots"),
        &base.join("file-artifacts"),
    )?;
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
    write_tar_name(&mut header, name)?;
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

fn write_tar_name(header: &mut [u8; 512], name: &str) -> Result<()> {
    if name.len() <= 100 {
        write_tar_field(&mut header[0..100], name.as_bytes());
        return Ok(());
    }

    let split = name
        .match_indices('/')
        .filter_map(|(index, _)| {
            let prefix = &name[..index];
            let file_name = &name[index + 1..];
            (prefix.len() <= 155 && file_name.len() <= 100).then_some((prefix, file_name))
        })
        .next_back()
        .ok_or_else(|| anyhow!("tar entry path is too long for portable ustar header: {name}"))?;

    write_tar_field(&mut header[0..100], split.1.as_bytes());
    write_tar_field(&mut header[345..500], split.0.as_bytes());
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

fn dir_writable(path: &Path) -> bool {
    if fs::create_dir_all(path).is_err() {
        return false;
    }
    let probe = path.join(format!(
        ".runglass-write-probe-{}",
        Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_else(|| Utc::now().timestamp_micros() * 1000)
    ));
    match fs::File::create(&probe) {
        Ok(_) => fs::remove_file(&probe).is_ok(),
        Err(_) => false,
    }
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
    apply: bool,
    preview_only: bool,
    force: bool,
    skip_changed: bool,
) -> Result<()> {
    if apply && preview_only {
        return Err(anyhow!("use either --apply or --preview, not both"));
    }
    if force && skip_changed {
        return Err(anyhow!("use either --force or --skip-changed, not both"));
    }

    let report = resolve_receipt(run_id)?;
    let selected = (!files.is_empty()).then_some(files);
    let preview = preview_revert(&report, selected)?;
    print_revert_preview(&preview);

    if !apply || preview_only {
        println!();
        println!("Preview only. Re-run with --apply to revert supported file changes.");
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
    println!(
        "Applied supported file revert for receipt {}",
        report.run.id
    );
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

fn resolve_receipt_input(selector: &str) -> Result<RunReport> {
    if selector.trim().is_empty() {
        return Err(anyhow!("receipt selector cannot be empty"));
    }

    let path = Path::new(selector);
    if path.is_file() {
        let data = fs::read_to_string(path)?;
        return serde_json::from_str(&data).map_err(|error| {
            anyhow!("failed to read receipt JSON at {}: {error}", path.display())
        });
    }

    resolve_receipt(selector)
}

#[cfg(test)]
mod tests {
    use super::{
        unsupported_platform_message, CiFormat, CiProvider, Cli, Commands, ExportFormat,
        GithubCommands, ReviewNonInteractive,
    };
    use clap::{CommandFactory, Parser};
    use std::collections::BTreeMap;

    fn prepare_man_command(
        cmd: clap::Command,
        display_path: String,
        bin_path: String,
    ) -> clap::Command {
        cmd.display_name(display_path.clone())
            .bin_name(bin_path.clone())
            .mut_subcommands(|subcommand| {
                let name = subcommand.get_name().to_string();
                prepare_man_command(
                    subcommand,
                    format!("{display_path}-{name}"),
                    format!("{bin_path} {name}"),
                )
            })
    }

    fn render_manpage(command: clap::Command) -> String {
        let title = command
            .get_display_name()
            .unwrap_or_else(|| command.get_name())
            .to_string();
        let mut buffer = Vec::new();
        clap_mangen::Man::new(command)
            .title(title)
            .date("2026-08-08")
            .source(format!("runglass {}", env!("CARGO_PKG_VERSION")))
            .manual("RunGlass Manual")
            .render(&mut buffer)
            .expect("render man page");
        String::from_utf8(buffer).expect("man page utf8")
    }

    fn collect_manpages(command: &clap::Command, pages: &mut BTreeMap<String, String>) {
        let display_name = command
            .get_display_name()
            .unwrap_or_else(|| command.get_name())
            .to_string();
        pages.insert(format!("{display_name}.1"), render_manpage(command.clone()));

        for subcommand in command
            .get_subcommands()
            .filter(|command| !command.is_hide_set())
        {
            collect_manpages(subcommand, pages);
        }
    }

    fn generated_manpages() -> BTreeMap<String, String> {
        let mut command = prepare_man_command(
            Cli::command().disable_help_subcommand(true),
            "runglass".to_string(),
            "runglass".to_string(),
        );
        command.build();

        let mut pages = BTreeMap::new();
        collect_manpages(&command, &mut pages);
        pages
    }

    #[test]
    fn checked_in_manpages_are_current() {
        let generated = generated_manpages();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("docs/man");

        if std::env::var_os("RUNGLASS_UPDATE_MANPAGE").is_some() {
            std::fs::create_dir_all(&dir).expect("create man dir");
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                if entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "1")
                {
                    std::fs::remove_file(entry.path()).expect("remove stale man page");
                }
            }
            for (file_name, contents) in generated {
                std::fs::write(dir.join(file_name), contents).expect("write man page");
            }
            return;
        }

        let mut current = BTreeMap::new();
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|error| {
            panic!(
                "failed to read {}; run `RUNGLASS_UPDATE_MANPAGE=1 cargo test -p runglass --bin runglass checked_in_manpages_are_current` to generate man pages: {error}",
                dir.display()
            )
        }) {
            let entry = entry.expect("read man page entry");
            if entry.path().extension().is_some_and(|extension| extension == "1") {
                current.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read_to_string(entry.path()).expect("read man page"),
                );
            }
        }

        assert_eq!(
            current.keys().collect::<Vec<_>>(),
            generated.keys().collect::<Vec<_>>(),
            "docs/man file list is stale; run `RUNGLASS_UPDATE_MANPAGE=1 cargo test -p runglass --bin runglass checked_in_manpages_are_current`"
        );
        for (file_name, expected) in generated {
            assert_eq!(
                current.get(&file_name),
                Some(&expected),
                "{file_name} is stale; run `RUNGLASS_UPDATE_MANPAGE=1 cargo test -p runglass --bin runglass checked_in_manpages_are_current`"
            );
        }
    }

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
    fn report_accepts_ai_summary_flag() {
        let cli = Cli::try_parse_from(["runglass", "report", "latest", "--ai"])
            .expect("parse report --ai");

        let Commands::Report { ai, print_json, .. } = cli.command else {
            panic!("expected report command");
        };
        assert!(ai);
        assert!(!print_json);
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
    fn run_accepts_review_and_non_interactive_summary() {
        let cli = Cli::try_parse_from([
            "runglass",
            "run",
            "--review",
            "--non-interactive",
            "summary",
            "--",
            "sh",
            "-c",
            "echo review",
        ])
        .expect("parse review run command");

        let Commands::Run {
            review,
            non_interactive,
            command,
            ..
        } = cli.command
        else {
            panic!("expected run command");
        };
        assert!(review);
        assert_eq!(non_interactive, ReviewNonInteractive::Summary);
        assert_eq!(command, vec!["sh", "-c", "echo review"]);
    }

    #[test]
    fn review_accepts_latest_and_non_interactive_summary() {
        let cli = Cli::try_parse_from(["runglass", "review"]).expect("parse review default");
        let Commands::Review {
            receipt,
            non_interactive,
        } = cli.command
        else {
            panic!("expected review command");
        };
        assert_eq!(receipt, "latest");
        assert_eq!(non_interactive, ReviewNonInteractive::Fail);

        let cli = Cli::try_parse_from([
            "runglass",
            "review",
            "abc123",
            "--non-interactive",
            "summary",
        ])
        .expect("parse review receipt summary");
        let Commands::Review {
            receipt,
            non_interactive,
        } = cli.command
        else {
            panic!("expected review command");
        };
        assert_eq!(receipt, "abc123");
        assert_eq!(non_interactive, ReviewNonInteractive::Summary);
    }

    #[test]
    fn revert_accepts_preview_and_apply_flags() {
        let preview = Cli::try_parse_from(["runglass", "revert", "latest", "--preview"])
            .expect("parse revert preview");
        let Commands::Revert {
            preview,
            apply,
            dry_run,
            ..
        } = preview.command
        else {
            panic!("expected revert command");
        };
        assert!(preview);
        assert!(!apply);
        assert!(!dry_run);

        let apply =
            Cli::try_parse_from(["runglass", "revert", "latest", "--apply", "--skip-changed"])
                .expect("parse revert apply");
        let Commands::Revert {
            preview,
            apply,
            skip_changed,
            ..
        } = apply.command
        else {
            panic!("expected revert command");
        };
        assert!(!preview);
        assert!(apply);
        assert!(skip_changed);
    }

    #[test]
    fn export_accepts_format_aliases_for_summaries() {
        let cli = Cli::try_parse_from([
            "runglass",
            "export",
            "latest",
            "--format",
            "summary-md,ai,reverse-patch",
        ])
        .expect("parse export formats");

        let Commands::Export { formats, .. } = cli.command else {
            panic!("expected export command");
        };
        assert_eq!(
            formats,
            vec![
                ExportFormat::SummaryMd,
                ExportFormat::Ai,
                ExportFormat::ReversePatch
            ]
        );
    }

    #[test]
    fn validate_defaults_to_latest_and_accepts_paths() {
        let cli = Cli::try_parse_from(["runglass", "validate"]).expect("parse validate default");
        let Commands::Validate { receipt } = cli.command else {
            panic!("expected validate command");
        };
        assert_eq!(receipt, "latest");

        let cli = Cli::try_parse_from(["runglass", "validate", "runglass-receipt/receipt.json"])
            .expect("parse validate path");
        let Commands::Validate { receipt } = cli.command else {
            panic!("expected validate command");
        };
        assert_eq!(receipt, "runglass-receipt/receipt.json");
    }

    #[test]
    fn github_comment_accepts_dry_run_arguments() {
        let cli = Cli::try_parse_from([
            "runglass",
            "github",
            "comment",
            "--receipt",
            "latest",
            "--repo",
            "error311/runglass",
            "--pr",
            "123",
            "--dry-run",
        ])
        .expect("parse github comment");

        let Commands::Github(GithubCommands::Comment(args)) = cli.command else {
            panic!("expected github comment command");
        };
        assert_eq!(args.receipt, "latest");
        assert_eq!(args.repo.as_deref(), Some("error311/runglass"));
        assert_eq!(args.pr, Some(123));
        assert!(args.dry_run);
    }

    #[test]
    fn pr_comment_alias_accepts_receipt_arguments() {
        let cli = Cli::try_parse_from([
            "runglass",
            "pr-comment",
            "--receipt",
            "abc123",
            "--repo",
            "error311/runglass",
            "--pr",
            "456",
            "--dry-run",
        ])
        .expect("parse pr-comment alias");

        let Commands::PrComment(args) = cli.command else {
            panic!("expected pr-comment command");
        };
        assert_eq!(args.receipt, "abc123");
        assert_eq!(args.repo.as_deref(), Some("error311/runglass"));
        assert_eq!(args.pr, Some(456));
        assert!(args.dry_run);
    }

    #[test]
    fn unsupported_platform_message_is_clear() {
        let message = unsupported_platform_message();
        assert!(message.contains("Linux-first"));
        assert!(message.contains("inspect, export, and validate"));
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

    #[test]
    fn ci_accepts_output_alias_for_out() {
        let cli = Cli::try_parse_from([
            "runglass",
            "ci",
            "--output",
            "receipt-out",
            "--",
            "npm",
            "test",
        ])
        .expect("parse ci --output alias");

        let Commands::Ci { out, command, .. } = cli.command else {
            panic!("expected ci command");
        };
        assert_eq!(out, std::path::PathBuf::from("receipt-out"));
        assert_eq!(command, vec!["npm", "test"]);
    }
}
