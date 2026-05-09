use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::collectors::{hash_bytes, simple_unified_diff};
use crate::storage::report_run_dir;
use crate::{
    FileChange, FileChangeType, RevertConflictPolicy, RevertFileState, RevertFileStatus,
    RevertOptions, RevertPreview, RunReport,
};

pub fn render_reverse_patch(report: &RunReport) -> Result<String> {
    let mut lines = vec![
        "# RunGlass Reverse Patch".to_string(),
        format!("# Receipt: {}", report.run.id),
        format!("# Command: {}", report.run.command_display),
        String::new(),
    ];

    let mut emitted = false;
    for file in &report.files {
        if !file.is_text {
            lines.push(format!("# Skipped non-text file {}", file.path));
            continue;
        }

        match file.change_type {
            FileChangeType::Modified => {
                let before = load_artifact_text(report, file.before_artifact_path.as_deref())?;
                let after = load_artifact_text(report, file.after_artifact_path.as_deref())?;
                let (Some(before), Some(after)) = (before, after) else {
                    lines.push(format!("# Missing stored text snapshot for {}", file.path));
                    continue;
                };
                lines.extend(git_style_patch(&file.path, Some(&after), Some(&before)));
                emitted = true;
            }
            FileChangeType::Created => {
                let after = load_artifact_text(report, file.after_artifact_path.as_deref())?;
                let Some(after) = after else {
                    lines.push(format!("# Missing stored text snapshot for {}", file.path));
                    continue;
                };
                lines.extend(git_style_patch(&file.path, Some(&after), None));
                emitted = true;
            }
            FileChangeType::Deleted => {
                let before = load_artifact_text(report, file.before_artifact_path.as_deref())?;
                let Some(before) = before else {
                    lines.push(format!("# Missing stored text snapshot for {}", file.path));
                    continue;
                };
                lines.extend(git_style_patch(&file.path, None, Some(&before)));
                emitted = true;
            }
        }
    }

    if !emitted {
        lines.push("# No reversible text patch content was available.".to_string());
    }

    Ok(lines.join("\n"))
}

pub fn preview_revert(
    report: &RunReport,
    selected_paths: Option<&[String]>,
) -> Result<RevertPreview> {
    let targets = select_revert_targets(report, selected_paths)?;
    let cwd = PathBuf::from(&report.run.cwd);
    let mut preview = RevertPreview {
        receipt_id: report.run.id.clone(),
        target_count: targets.len(),
        restore_modified: 0,
        delete_created: 0,
        restore_deleted: 0,
        safe: Vec::new(),
        conflicts: Vec::new(),
        already_reverted: Vec::new(),
        missing_artifacts: Vec::new(),
    };

    for file in targets {
        match file.change_type {
            FileChangeType::Modified => preview.restore_modified += 1,
            FileChangeType::Created => preview.delete_created += 1,
            FileChangeType::Deleted => preview.restore_deleted += 1,
        }

        let status = evaluate_revert_status(file, &cwd)?;
        match status.status {
            RevertFileState::Safe => preview.safe.push(status),
            RevertFileState::ChangedSinceReceipt => preview.conflicts.push(status),
            RevertFileState::AlreadyReverted => preview.already_reverted.push(status),
            RevertFileState::MissingArtifacts => preview.missing_artifacts.push(status),
        }
    }

    Ok(preview)
}

pub fn apply_revert(
    report: &RunReport,
    selected_paths: Option<&[String]>,
    options: RevertOptions,
) -> Result<RevertPreview> {
    let preview = preview_revert(report, selected_paths)?;
    if !preview.missing_artifacts.is_empty() {
        return Err(anyhow!(
            "receipt is missing stored file snapshots for: {}",
            preview
                .missing_artifacts
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !preview.conflicts.is_empty() && matches!(options.policy, RevertConflictPolicy::Abort) {
        return Err(anyhow!(
            "some files changed after the receipt finished: {}",
            preview
                .conflicts
                .iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let targets = select_revert_targets(report, selected_paths)?;
    let cwd = PathBuf::from(&report.run.cwd);
    for file in targets {
        let status = evaluate_revert_status(file, &cwd)?;
        if matches!(status.status, RevertFileState::AlreadyReverted) {
            continue;
        }
        if matches!(status.status, RevertFileState::ChangedSinceReceipt)
            && matches!(options.policy, RevertConflictPolicy::SkipChanged)
        {
            continue;
        }
        apply_revert_file(report, file, &cwd)?;
    }

    preview_revert(report, selected_paths)
}

fn select_revert_targets<'a>(
    report: &'a RunReport,
    selected_paths: Option<&[String]>,
) -> Result<Vec<&'a FileChange>> {
    if let Some(paths) = selected_paths {
        if paths.is_empty() {
            return Ok(report.files.iter().collect());
        }
        let mut selected = Vec::new();
        for path in paths {
            let file = report
                .files
                .iter()
                .find(|file| file.path == *path)
                .ok_or_else(|| anyhow!("receipt does not include file change {}", path))?;
            selected.push(file);
        }
        return Ok(selected);
    }
    Ok(report.files.iter().collect())
}

fn artifact_path(report: &RunReport, relative: &str) -> Result<PathBuf> {
    let cwd_local = PathBuf::from(&report.run.cwd)
        .join(".runglass")
        .join("reports")
        .join(&report.run.id)
        .join(relative);
    if cwd_local.exists() {
        return Ok(cwd_local);
    }
    Ok(report_run_dir(&report.run.id)?.join(relative))
}

fn load_artifact_bytes(report: &RunReport, relative: Option<&str>) -> Result<Option<Vec<u8>>> {
    let Some(relative) = relative else {
        return Ok(None);
    };
    let path = artifact_path(report, relative)?;
    Ok(Some(fs::read(&path).with_context(|| {
        format!("failed to read artifact {}", path.display())
    })?))
}

fn load_artifact_text(report: &RunReport, relative: Option<&str>) -> Result<Option<String>> {
    let Some(bytes) = load_artifact_bytes(report, relative)? else {
        return Ok(None);
    };
    Ok(String::from_utf8(bytes).ok())
}

fn current_file_hash(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(Some(hash_bytes(&bytes)))
}

fn evaluate_revert_status(file: &FileChange, cwd: &Path) -> Result<RevertFileStatus> {
    let current_path = cwd.join(&file.path);
    let current_hash = current_file_hash(&current_path)?;

    let status = match file.change_type {
        FileChangeType::Modified => {
            if file.before_artifact_path.is_none() {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::MissingArtifacts,
                    detail: "Missing stored before snapshot.".to_string(),
                }
            } else if current_hash.as_deref() == file.after_hash.as_deref() {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::Safe,
                    detail: "Current file still matches the receipt's after-run version."
                        .to_string(),
                }
            } else if current_hash.as_deref() == file.before_hash.as_deref() {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::AlreadyReverted,
                    detail: "Current file already matches the stored before-run version."
                        .to_string(),
                }
            } else {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::ChangedSinceReceipt,
                    detail: "File contents changed after the receipt finished.".to_string(),
                }
            }
        }
        FileChangeType::Created => {
            if current_hash.is_none() {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::AlreadyReverted,
                    detail: "Created file is already gone.".to_string(),
                }
            } else if current_hash.as_deref() == file.after_hash.as_deref() {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::Safe,
                    detail:
                        "Created file still matches the receipt version and can be deleted safely."
                            .to_string(),
                }
            } else {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::ChangedSinceReceipt,
                    detail: "Created file changed after the receipt finished.".to_string(),
                }
            }
        }
        FileChangeType::Deleted => {
            if file.before_artifact_path.is_none() {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::MissingArtifacts,
                    detail: "Missing stored before snapshot.".to_string(),
                }
            } else if current_hash.is_none() {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::Safe,
                    detail: "Deleted file is still absent and can be restored safely.".to_string(),
                }
            } else if current_hash.as_deref() == file.before_hash.as_deref() {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::AlreadyReverted,
                    detail: "Deleted file already matches the stored before-run version."
                        .to_string(),
                }
            } else {
                RevertFileStatus {
                    path: file.path.clone(),
                    change_type: file.change_type.clone(),
                    status: RevertFileState::ChangedSinceReceipt,
                    detail: "A newer file now exists at this path.".to_string(),
                }
            }
        }
    };

    Ok(status)
}

fn apply_revert_file(report: &RunReport, file: &FileChange, cwd: &Path) -> Result<()> {
    let path = cwd.join(&file.path);
    match file.change_type {
        FileChangeType::Modified | FileChangeType::Deleted => {
            let bytes = load_artifact_bytes(report, file.before_artifact_path.as_deref())?
                .ok_or_else(|| anyhow!("missing stored before snapshot for {}", file.path))?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, bytes)?;
            set_executable_flag(&path, file.before_executable.unwrap_or(false))?;
        }
        FileChangeType::Created => {
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to delete {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn set_executable_flag(path: &Path, executable: bool) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        let mode = if executable { 0o755 } else { 0o644 };
        perms.set_mode(mode);
        fs::set_permissions(path, perms)?;
    }

    #[cfg(not(unix))]
    {
        let _ = (path, executable);
    }

    Ok(())
}

fn git_style_patch(path: &str, before: Option<&str>, after: Option<&str>) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("diff --git a/{path} b/{path}"));
    match (before, after) {
        (Some(_), None) => {
            lines.push("deleted file mode 100644".to_string());
            lines.push(format!("--- a/{path}"));
            lines.push("+++ /dev/null".to_string());
            lines.push(simple_unified_diff(before.unwrap_or_default(), ""));
        }
        (None, Some(_)) => {
            lines.push("new file mode 100644".to_string());
            lines.push("--- /dev/null".to_string());
            lines.push(format!("+++ b/{path}"));
            lines.push(simple_unified_diff("", after.unwrap_or_default()));
        }
        (Some(before), Some(after)) => {
            lines.push(format!("--- a/{path}"));
            lines.push(format!("+++ b/{path}"));
            lines.push(simple_unified_diff(before, after));
        }
        (None, None) => {}
    }
    lines.push(String::new());
    lines
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{env, fs};

    use chrono::Utc;

    use super::{apply_revert, preview_revert, render_reverse_patch};
    use crate::collectors::hash_bytes;
    use crate::{
        FileChange, FileChangeType, ObservationMode, RevertConflictPolicy, RevertOptions,
        RiskLevel, RunMeta, RunReport, RunStatus, Summary,
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn preview_and_apply_revert_cover_modified_created_and_deleted_files() {
        let _guard = env_lock().lock().expect("lock test env");
        let fixture = RevertFixture::new("revert-apply");

        let preview = preview_revert(&fixture.report, None).expect("preview");
        assert_eq!(preview.target_count, 3);
        assert_eq!(preview.safe.len(), 3);
        assert!(preview.conflicts.is_empty());

        let after_apply = apply_revert(
            &fixture.report,
            None,
            RevertOptions {
                policy: RevertConflictPolicy::Force,
            },
        )
        .expect("apply revert");

        assert_eq!(after_apply.already_reverted.len(), 3);
        assert_eq!(
            fs::read_to_string(fixture.workspace.join("modified.txt")).expect("modified content"),
            "before\n"
        );
        assert!(
            !fixture.workspace.join("created.txt").exists(),
            "created file should be removed"
        );
        assert_eq!(
            fs::read_to_string(fixture.workspace.join("deleted.txt")).expect("deleted restored"),
            "restore me\n"
        );
    }

    #[test]
    fn preview_detects_conflicts_and_skip_changed_leaves_newer_edits_in_place() {
        let _guard = env_lock().lock().expect("lock test env");
        let fixture = RevertFixture::new("revert-conflict");

        fs::write(fixture.workspace.join("modified.txt"), "changed again\n")
            .expect("write newer modified");
        fs::write(fixture.workspace.join("created.txt"), "changed created\n")
            .expect("write newer created");

        let preview = preview_revert(&fixture.report, None).expect("preview");
        assert_eq!(preview.conflicts.len(), 2);
        assert_eq!(preview.safe.len(), 1);

        let after_apply = apply_revert(
            &fixture.report,
            None,
            RevertOptions {
                policy: RevertConflictPolicy::SkipChanged,
            },
        )
        .expect("apply skip changed");

        assert_eq!(after_apply.conflicts.len(), 2);
        assert_eq!(
            fs::read_to_string(fixture.workspace.join("modified.txt")).expect("modified content"),
            "changed again\n"
        );
        assert_eq!(
            fs::read_to_string(fixture.workspace.join("created.txt")).expect("created content"),
            "changed created\n"
        );
        assert_eq!(
            fs::read_to_string(fixture.workspace.join("deleted.txt")).expect("deleted restored"),
            "restore me\n"
        );
    }

    #[test]
    fn reverse_patch_contains_git_style_operations() {
        let _guard = env_lock().lock().expect("lock test env");
        let fixture = RevertFixture::new("reverse-patch");

        let patch = render_reverse_patch(&fixture.report).expect("reverse patch");
        assert!(patch.contains("# RunGlass Reverse Patch"));
        assert!(patch.contains("diff --git a/modified.txt b/modified.txt"));
        assert!(patch.contains("deleted file mode 100644"));
        assert!(patch.contains("new file mode 100644"));
    }

    struct RevertFixture {
        workspace: std::path::PathBuf,
        report: RunReport,
    }

    impl RevertFixture {
        fn new(name: &str) -> Self {
            let root = unique_test_root(name);
            let workspace = root.join("workspace");
            let run_id = format!("{name}-{}", unique_suffix());
            let run_dir = workspace.join(".runglass").join("reports").join(&run_id);
            let artifacts_dir = run_dir.join("file-artifacts");

            fs::create_dir_all(&workspace).expect("workspace dir");
            fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

            fs::write(workspace.join("modified.txt"), "after\n").expect("modified workspace");
            fs::write(workspace.join("created.txt"), "created\n").expect("created workspace");

            fs::write(artifacts_dir.join("001_modified-txt.before"), "before\n")
                .expect("modified before");
            fs::write(artifacts_dir.join("001_modified-txt.after"), "after\n")
                .expect("modified after");
            fs::write(artifacts_dir.join("002_created-txt.after"), "created\n")
                .expect("created after");
            fs::write(artifacts_dir.join("003_deleted-txt.before"), "restore me\n")
                .expect("deleted before");

            let report = RunReport {
                schema_version: "0.1.0".to_string(),
                ci: None,
                run: RunMeta {
                    id: run_id.clone(),
                    command_display: "sh -c 'test revert'".to_string(),
                    argv: vec![
                        "sh".to_string(),
                        "-c".to_string(),
                        "test revert".to_string(),
                    ],
                    cwd: workspace.display().to_string(),
                    shell: Some("/bin/sh".to_string()),
                    mode: ObservationMode::Normal,
                    started_at: Utc::now(),
                    ended_at: Some(Utc::now()),
                    duration_ms: Some(250),
                    exit_code: Some(0),
                    status: RunStatus::Completed,
                },
                summary: Summary {
                    files_changed: 3,
                    files_created: 1,
                    files_modified: 1,
                    files_deleted: 1,
                    processes_seen: 0,
                    network_hosts: 0,
                    ports_opened: 0,
                    docker_containers_created: 0,
                    docker_images_pulled: 0,
                    docker_volumes_created: 0,
                    risk_level: RiskLevel::Low,
                },
                events: Vec::new(),
                processes: Vec::new(),
                files: vec![
                    file_change(
                        "modified.txt",
                        FileChangeType::Modified,
                        Some("before\n"),
                        Some("after\n"),
                        Some("file-artifacts/001_modified-txt.before"),
                        Some("file-artifacts/001_modified-txt.after"),
                    ),
                    file_change(
                        "created.txt",
                        FileChangeType::Created,
                        None,
                        Some("created\n"),
                        None,
                        Some("file-artifacts/002_created-txt.after"),
                    ),
                    file_change(
                        "deleted.txt",
                        FileChangeType::Deleted,
                        Some("restore me\n"),
                        None,
                        Some("file-artifacts/003_deleted-txt.before"),
                        None,
                    ),
                ],
                network: Vec::new(),
                docker: None,
                risks: Vec::new(),
                stdout_path: None,
                stderr_path: None,
                stdout: None,
                stderr: None,
                limitations: vec!["test receipt".to_string()],
            };

            fs::create_dir_all(&run_dir).expect("run dir");
            fs::write(
                run_dir.join("report.json"),
                serde_json::to_vec_pretty(&report).expect("report json"),
            )
            .expect("write report json");

            Self { workspace, report }
        }
    }

    fn file_change(
        path: &str,
        change_type: FileChangeType,
        before: Option<&str>,
        after: Option<&str>,
        before_artifact_path: Option<&str>,
        after_artifact_path: Option<&str>,
    ) -> FileChange {
        FileChange {
            path: path.to_string(),
            change_type,
            before_hash: before.map(|value| hash_bytes(value.as_bytes())),
            after_hash: after.map(|value| hash_bytes(value.as_bytes())),
            before_size: before.map(|value| value.len() as u64),
            after_size: after.map(|value| value.len() as u64),
            is_text: true,
            diff: None,
            risk_tags: Vec::new(),
            before_artifact_path: before_artifact_path.map(ToString::to_string),
            after_artifact_path: after_artifact_path.map(ToString::to_string),
            before_executable: Some(false),
            after_executable: Some(false),
        }
    }

    fn unique_test_root(name: &str) -> std::path::PathBuf {
        let root = env::temp_dir().join(format!("runglass-{name}-{}", unique_suffix()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove stale test root");
        }
        fs::create_dir_all(&root).expect("create test root");
        root
    }

    fn unique_suffix() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        format!("{}-{nanos}", std::process::id())
    }
}
