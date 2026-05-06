use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;

use crate::{RunPaths, RunReport, SnapshotEntry};

pub fn reports_dir() -> Result<PathBuf> {
    let candidates = report_dir_candidates();

    let mut last_error = None;
    for candidate in candidates {
        match ensure_writable_reports_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) => last_error = Some((candidate, error)),
        }
    }

    if let Some((path, error)) = last_error {
        Err(anyhow!(
            "failed to prepare reports directory {}: {}",
            path.display(),
            error
        ))
    } else {
        Err(anyhow!("no writable reports directory is available"))
    }
}

pub(crate) fn report_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(value) = env::var("RUNGLASS_DATA_HOME") {
        candidates.push(PathBuf::from(value).join("runglass").join("reports"));
    }
    if let Ok(value) = env::var("XDG_DATA_HOME") {
        candidates.push(PathBuf::from(value).join("runglass").join("reports"));
    }
    if let Ok(home) = home_dir() {
        candidates.push(
            home.join(".local")
                .join("share")
                .join("runglass")
                .join("reports"),
        );
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join(".runglass").join("reports"));
    }
    dedupe_paths(candidates)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            deduped.push(path);
        }
    }
    deduped
}

fn ensure_writable_reports_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    let probe = path.join(format!(
        ".write-probe-{}",
        Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_else(|| Utc::now().timestamp_micros() * 1000)
    ));
    fs::create_dir(&probe)?;
    fs::remove_dir(&probe)?;
    Ok(())
}

pub fn home_dir() -> Result<PathBuf> {
    env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| anyhow!("HOME is not set"))
}

pub fn make_run_id(command: &str) -> String {
    let ts = Utc::now().format("%Y-%m-%dT%H-%M-%SZ");
    let slug = slugify(command);
    format!("{ts}_{slug}")
}

pub fn prepare_run_paths(run_id: &str) -> Result<RunPaths> {
    let run_dir = reports_dir()?.join(run_id);
    fs::create_dir_all(&run_dir)
        .with_context(|| format!("failed to create run directory {}", run_dir.display()))?;
    Ok(RunPaths {
        report_path: run_dir.join("report.json"),
        stdout_path: run_dir.join("stdout.log"),
        stderr_path: run_dir.join("stderr.log"),
        run_dir,
    })
}

pub fn write_report_bundle(
    paths: &RunPaths,
    report: &RunReport,
    stdout: &str,
    stderr: &str,
) -> Result<()> {
    fs::create_dir_all(&paths.run_dir)?;
    fs::write(&paths.stdout_path, stdout)?;
    fs::write(&paths.stderr_path, stderr)?;
    let json = serde_json::to_vec_pretty(report)?;
    fs::write(&paths.report_path, json)?;
    Ok(())
}

pub fn persist_file_change_artifacts(
    paths: &RunPaths,
    report: &mut RunReport,
    before: &HashMap<String, SnapshotEntry>,
    after: &HashMap<String, SnapshotEntry>,
) -> Result<()> {
    let artifacts_dir = paths.run_dir.join("file-artifacts");
    fs::create_dir_all(&artifacts_dir)?;

    for (index, file) in report.files.iter_mut().enumerate() {
        let base = format!("{:03}_{}", index + 1, slugify(&file.path));

        if let Some(before_entry) = before.get(&file.path) {
            let relative = format!("file-artifacts/{base}.before");
            fs::write(paths.run_dir.join(&relative), &before_entry.bytes)?;
            file.before_artifact_path = Some(relative);
            file.before_executable = Some(before_entry.executable);
        }

        if let Some(after_entry) = after.get(&file.path) {
            let relative = format!("file-artifacts/{base}.after");
            fs::write(paths.run_dir.join(&relative), &after_entry.bytes)?;
            file.after_artifact_path = Some(relative);
            file.after_executable = Some(after_entry.executable);
        }
    }

    Ok(())
}

pub fn load_report(run_id: &str) -> Result<RunReport> {
    let path = locate_report_json(run_id)?;
    let data =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(serde_json::from_str(&data)?)
}

pub fn list_reports() -> Result<Vec<RunReport>> {
    let mut reports = Vec::new();
    let mut seen = HashSet::new();

    for dir in report_dir_candidates() {
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let report_path = entry.path().join("report.json");
            if !report_path.exists() {
                continue;
            }
            let data = fs::read_to_string(&report_path)?;
            let report: RunReport = serde_json::from_str(&data)?;
            if seen.insert(report.run.id.clone()) {
                reports.push(report);
            }
        }
    }
    reports.sort_by(|a, b| {
        b.run
            .started_at
            .cmp(&a.run.started_at)
            .then_with(|| b.run.id.cmp(&a.run.id))
    });
    Ok(reports)
}

pub fn latest_report() -> Result<RunReport> {
    list_reports()?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no saved receipts are available"))
}

pub fn report_run_dir(run_id: &str) -> Result<PathBuf> {
    let report_json = locate_report_json(run_id)?;
    report_json
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("failed to resolve receipt directory for {}", run_id))
}

pub fn delete_report(run_id: &str) -> Result<PathBuf> {
    let run_dir = report_run_dir(run_id)?;
    fs::remove_dir_all(&run_dir)
        .with_context(|| format!("failed to delete receipt directory {}", run_dir.display()))?;
    Ok(run_dir)
}

pub fn prune_reports(keep: usize, dry_run: bool) -> Result<Vec<String>> {
    let reports = list_reports()?;
    let mut deleted = Vec::new();
    for report in reports.into_iter().skip(keep) {
        if !dry_run {
            let _ = delete_report(&report.run.id)?;
        }
        deleted.push(report.run.id);
    }
    Ok(deleted)
}

fn locate_report_json(run_id: &str) -> Result<PathBuf> {
    let mut attempted = Vec::new();
    for dir in report_dir_candidates() {
        let path = dir.join(run_id).join("report.json");
        attempted.push(path.display().to_string());
        if path.exists() {
            return Ok(path);
        }
    }

    Err(anyhow!(
        "failed to locate receipt {}. looked in: {}",
        run_id,
        attempted.join(", ")
    ))
}

pub(crate) fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "run".to_string()
    } else {
        slug.to_string()
    }
}
