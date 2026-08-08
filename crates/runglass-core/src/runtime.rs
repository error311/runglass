use std::collections::{HashMap, HashSet};
use std::env;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};

#[cfg(target_os = "linux")]
use crate::collectors::read_proc_processes;
use crate::collectors::{
    capture_docker_snapshot, count_child_process_samples, diff_docker_snapshots, diff_snapshots,
    merge_deep_process_samples, merge_processes_with_network_samples, parse_deep_trace_capture,
    prepare_deep_trace_prefix, read_network_samples, read_network_samples_ss,
    read_process_tree_sample_with_known, snapshot_directory_with_stats, snapshot_file_byte_limit,
    summarize_network_samples, summarize_process_samples, wrap_command_for_mode,
};
use crate::reporting::{
    build_summary, derive_risk_level, derive_risks, empty_summary, network_events, unique_hosts,
};
use crate::storage::{
    make_run_id, persist_file_change_artifacts, prepare_run_paths, write_report_bundle,
};
use crate::{
    build_command_report, DockerSnapshot, FileChange, NetworkDirection, ObservationMode,
    ProcessInfo, ProcessSample, RiskNote, RunPaths, RunProgress, RunReport, RunStatus, Severity,
    SnapshotEntry, Summary, TimelineEvent,
};

pub fn run_observed_command(command: Vec<String>) -> Result<(RunReport, RunPaths)> {
    run_observed_command_in_mode(command, ObservationMode::Normal)
}

pub fn run_observed_shell_command(command_line: String) -> Result<(RunReport, RunPaths)> {
    run_observed_shell_command_in_mode(command_line, ObservationMode::Normal)
}

pub fn run_observed_command_in_mode(
    command: Vec<String>,
    mode: ObservationMode,
) -> Result<(RunReport, RunPaths)> {
    run_observed_command_with_control_in_mode(command, mode, |_| {}, || false)
}

pub fn run_observed_shell_command_in_mode(
    command_line: String,
    mode: ObservationMode,
) -> Result<(RunReport, RunPaths)> {
    run_observed_shell_command_with_control_in_mode(command_line, mode, |_| {}, || false)
}

pub fn run_observed_command_with_progress<F>(
    command: Vec<String>,
    on_progress: F,
) -> Result<(RunReport, RunPaths)>
where
    F: FnMut(RunProgress),
{
    run_observed_command_with_control_in_mode(command, ObservationMode::Normal, on_progress, || {
        false
    })
}

pub fn run_observed_shell_command_with_progress<F>(
    command_line: String,
    on_progress: F,
) -> Result<(RunReport, RunPaths)>
where
    F: FnMut(RunProgress),
{
    run_observed_shell_command_with_control_in_mode(
        command_line,
        ObservationMode::Normal,
        on_progress,
        || false,
    )
}

pub fn run_observed_command_with_control<F, C>(
    command: Vec<String>,
    on_progress: F,
    should_cancel: C,
) -> Result<(RunReport, RunPaths)>
where
    F: FnMut(RunProgress),
    C: Fn() -> bool,
{
    run_observed_command_with_control_in_mode(
        command,
        ObservationMode::Normal,
        on_progress,
        should_cancel,
    )
}

pub fn run_observed_shell_command_with_control<F, C>(
    command_line: String,
    on_progress: F,
    should_cancel: C,
) -> Result<(RunReport, RunPaths)>
where
    F: FnMut(RunProgress),
    C: Fn() -> bool,
{
    run_observed_shell_command_with_control_in_mode(
        command_line,
        ObservationMode::Normal,
        on_progress,
        should_cancel,
    )
}

pub fn run_observed_command_with_control_in_mode<F, C>(
    command: Vec<String>,
    mode: ObservationMode,
    on_progress: F,
    should_cancel: C,
) -> Result<(RunReport, RunPaths)>
where
    F: FnMut(RunProgress),
    C: Fn() -> bool,
{
    run_observed_command_internal(command, None, mode, on_progress, should_cancel)
}

pub fn run_observed_shell_command_with_control_in_mode<F, C>(
    command_line: String,
    mode: ObservationMode,
    on_progress: F,
    should_cancel: C,
) -> Result<(RunReport, RunPaths)>
where
    F: FnMut(RunProgress),
    C: Fn() -> bool,
{
    run_observed_command_internal(
        vec!["sh".to_string(), "-lc".to_string(), command_line.clone()],
        Some(command_line),
        mode,
        on_progress,
        should_cancel,
    )
}

fn run_observed_command_internal<F, C>(
    command: Vec<String>,
    display_override: Option<String>,
    mode: ObservationMode,
    mut on_progress: F,
    should_cancel: C,
) -> Result<(RunReport, RunPaths)>
where
    F: FnMut(RunProgress),
    C: Fn() -> bool,
{
    let run_id = make_run_id(display_override.as_deref().unwrap_or(&command.join(" ")));
    let paths = prepare_run_paths(&run_id)?;
    let cwd = env::current_dir()?;
    let (before, before_snapshot_stats) = snapshot_directory_with_stats(&cwd)?;
    let docker_before = capture_docker_snapshot();
    let started_at = Utc::now();
    let timer = Instant::now();
    let command_display = display_override
        .clone()
        .unwrap_or_else(|| command.join(" "));
    let mut effective_mode = mode;
    let mut startup_limitations = Vec::new();
    if matches!(mode, ObservationMode::Deep) && !command_on_path("strace") {
        effective_mode = ObservationMode::Normal;
        startup_limitations.push(
            "Deep mode was requested, but strace was not found on PATH. RunGlass fell back to normal observation."
                .to_string(),
        );
    }
    let trace_prefix = match effective_mode {
        ObservationMode::Deep => Some(prepare_deep_trace_prefix(&paths)?),
        ObservationMode::Normal => None,
    };
    let spawn_command = wrap_command_for_mode(&command, effective_mode, trace_prefix.as_deref())?;

    let mut child = Command::new(&spawn_command[0])
        .args(&spawn_command[1..])
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run {}", spawn_command[0]))?;
    let root_pid = child.id();
    let stdout_live = Arc::new(Mutex::new(Vec::new()));
    let stderr_live = Arc::new(Mutex::new(Vec::new()));
    let stdout_reader = child
        .stdout
        .take()
        .map(|stdout| spawn_output_reader(stdout, Arc::clone(&stdout_live)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| spawn_output_reader(stderr, Arc::clone(&stderr_live)));

    let mut process_samples = Vec::new();
    let mut network_samples = Vec::new();
    let mut known_process_ids = HashSet::from([root_pid]);
    let mut live_files = Vec::new();
    let mut live_docker = None;
    let mut live_risks = Vec::new();
    let mut live_summary = empty_summary();
    let mut last_side_effect_refresh = Instant::now()
        .checked_sub(Duration::from_millis(500))
        .unwrap_or_else(Instant::now);
    let mut last_ss_refresh = Instant::now()
        .checked_sub(Duration::from_millis(200))
        .unwrap_or_else(Instant::now);
    let mut interrupted = false;
    let exit_status = loop {
        let observed_at = Utc::now();
        let current_processes =
            read_process_tree_sample_with_known(root_pid, &known_process_ids, observed_at);
        known_process_ids.extend(current_processes.iter().map(|process| process.pid));
        network_samples.extend(read_network_samples(&current_processes, observed_at));
        if last_ss_refresh.elapsed() >= ss_poll_interval(timer.elapsed()) {
            let ss_samples = read_network_samples_ss(root_pid, &current_processes, observed_at);
            known_process_ids.extend(ss_samples.iter().filter_map(|sample| sample.pid));
            network_samples.extend(ss_samples);
            last_ss_refresh = Instant::now();
        }
        process_samples.extend(current_processes);
        if last_side_effect_refresh.elapsed() >= Duration::from_millis(250) {
            refresh_live_side_effects(
                root_pid,
                &before,
                &docker_before,
                &cwd,
                &process_samples,
                &network_samples,
                &mut live_files,
                &mut live_docker,
                &mut live_risks,
                &mut live_summary,
            );
            last_side_effect_refresh = Instant::now();
        }
        on_progress(progress_snapshot(
            root_pid,
            &command,
            &command_display,
            effective_mode,
            started_at,
            &stdout_live,
            &stderr_live,
            &process_samples,
            &network_samples,
            &live_summary,
            &live_files,
            live_docker.as_ref(),
            &live_risks,
        ));
        if should_cancel() {
            interrupted = true;
            terminate_process_tree(root_pid);
            let status = child
                .wait()
                .with_context(|| format!("failed to stop {}", command[0]))?;
            break status;
        }
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("failed to observe {}", command[0]))?
        {
            settle_observations_after_exit(
                root_pid,
                &mut known_process_ids,
                &mut process_samples,
                &mut network_samples,
                &before,
                &docker_before,
                &cwd,
                &mut live_files,
                &mut live_docker,
                &mut live_risks,
                &mut live_summary,
            );
            break status;
        }
        thread::sleep(observation_poll_interval(
            timer.elapsed(),
            known_process_ids.len(),
        ));
    };

    let ended_at = Utc::now();
    let stdout = join_output_reader(stdout_reader)?;
    let stderr = join_output_reader(stderr_reader)?;
    let output = Output {
        status: exit_status,
        stdout,
        stderr,
    };
    let mut deep_limitations = Vec::new();
    let mut effective_root_pid = Some(root_pid);
    let (after, after_snapshot_stats) = snapshot_directory_with_stats(&cwd)?;
    let docker_after = capture_docker_snapshot();
    let files = diff_snapshots(&before, &after);
    if let Some(prefix) = trace_prefix.as_deref() {
        match parse_deep_trace_capture(prefix) {
            Ok(capture) => {
                effective_root_pid = capture.root_pid.or(effective_root_pid);
                process_samples = merge_deep_process_samples(process_samples, &capture.processes);
                network_samples.extend(capture.network_samples);
            }
            Err(error) => deep_limitations.push(format!(
                "Deep tracing was requested, but RunGlass could not fully parse the strace capture: {error}"
            )),
        }
    }
    let mut processes = merge_processes_with_network_samples(
        summarize_process_samples(root_pid, &command, started_at, ended_at, process_samples),
        &network_samples,
        root_pid,
    );
    if matches!(effective_mode, ObservationMode::Deep) {
        processes.retain(|process| process.command != "strace");
    }
    let network = summarize_network_samples(network_samples);
    let (docker, docker_limitations) = match (docker_before, docker_after) {
        (Ok(before), Ok(after)) => (Some(diff_docker_snapshots(&before, &after)), Vec::new()),
        (Err(error), _) | (_, Err(error)) => (
            None,
            vec![format!("Docker changes were not collected: {error}")],
        ),
    };
    let mut report = build_command_report(
        run_id,
        &command,
        &cwd,
        &output,
        started_at,
        ended_at,
        timer.elapsed(),
        effective_root_pid,
        &paths.stdout_path,
        &paths.stderr_path,
        processes,
        network,
        docker,
        files,
        effective_mode,
        if interrupted {
            RunStatus::Interrupted
        } else {
            RunStatus::Completed
        },
        {
            let mut limitations = startup_limitations;
            limitations.extend(docker_limitations);
            limitations.extend(deep_limitations);
            limitations.extend(build_snapshot_control_notes(
                &cwd,
                &before_snapshot_stats,
                &after_snapshot_stats,
            ));
            limitations
        },
    );

    if let Some(display) = display_override {
        report.run.command_display = display;
        report.run.shell = Some("sh".to_string());
    }

    persist_file_change_artifacts(&paths, &mut report, &before, &after)?;
    write_report_bundle(
        &paths,
        &report,
        report.stdout.as_deref().unwrap_or_default(),
        report.stderr.as_deref().unwrap_or_default(),
    )?;
    Ok((report, paths))
}

fn command_on_path(command: &str) -> bool {
    let command_path = Path::new(command);
    if command_path.components().count() > 1 {
        return command_path.is_file();
    }
    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|path| path.join(command).is_file()))
        .unwrap_or(false)
}

fn observation_poll_interval(elapsed: Duration, known_processes: usize) -> Duration {
    if elapsed < Duration::from_millis(250) {
        Duration::from_millis(3)
    } else if elapsed < Duration::from_secs(1) {
        Duration::from_millis(6)
    } else if elapsed < Duration::from_secs(3) || known_processes > 1 {
        Duration::from_millis(15)
    } else {
        Duration::from_millis(35)
    }
}

fn ss_poll_interval(elapsed: Duration) -> Duration {
    if elapsed < Duration::from_millis(300) {
        Duration::from_millis(5)
    } else if elapsed < Duration::from_secs(1) {
        Duration::from_millis(10)
    } else if elapsed < Duration::from_secs(3) {
        Duration::from_millis(15)
    } else {
        Duration::from_millis(50)
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh_live_side_effects(
    root_pid: u32,
    before: &HashMap<String, SnapshotEntry>,
    docker_before: &Result<DockerSnapshot>,
    cwd: &std::path::Path,
    process_samples: &[ProcessSample],
    network_samples: &[crate::NetworkSample],
    live_files: &mut Vec<FileChange>,
    live_docker: &mut Option<crate::DockerSummary>,
    live_risks: &mut Vec<RiskNote>,
    live_summary: &mut Summary,
) {
    if let Ok((current_snapshot, _)) = snapshot_directory_with_stats(cwd) {
        *live_files = diff_snapshots(before, &current_snapshot);
    }
    if let Ok(current_docker) = capture_docker_snapshot() {
        if let Ok(before_docker) = docker_before {
            *live_docker = Some(diff_docker_snapshots(before_docker, &current_docker));
        }
    }
    let network = summarize_network_samples(network_samples.to_vec());
    *live_risks = derive_risks(
        live_files,
        &network,
        live_docker.as_ref(),
        &RunStatus::Running,
        None,
    );
    *live_summary = build_summary(
        live_files,
        count_child_process_samples(process_samples, root_pid),
        &network,
        live_docker.as_ref(),
        derive_risk_level(live_risks, live_files),
    );
}

fn build_snapshot_control_notes(
    cwd: &std::path::Path,
    before: &crate::SnapshotDirectoryStats,
    after: &crate::SnapshotDirectoryStats,
) -> Vec<String> {
    let mut notes = vec![format!(
        "Snapshot controls: RunGlass captured working-directory files up to {} per file unless RUNGLASS_MAX_SNAPSHOT_BYTES configured a larger or smaller limit.",
        human_size(snapshot_file_byte_limit())
    )];
    if cwd.join(".runglassignore").exists() {
        notes.push(
            "Snapshot controls: .runglassignore was active for this run, so matching paths were skipped before diffing."
                .to_string(),
        );
    }

    let mut skipped = before.skipped_large_files.clone();
    skipped.extend(after.skipped_large_files.clone());
    if skipped.is_empty() {
        return notes;
    }

    let mut unique_paths = Vec::new();
    let mut seen = HashSet::new();
    for item in skipped {
        if seen.insert(item.path.clone()) {
            unique_paths.push(item.path);
        }
    }

    let preview = unique_paths
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = unique_paths.len().saturating_sub(3);
    let suffix = if remaining > 0 {
        format!(", and {remaining} more")
    } else {
        String::new()
    };

    notes.push(format!(
        "RunGlass skipped {} file{} larger than {} while snapshotting the working directory: {}{}.",
        unique_paths.len(),
        if unique_paths.len() == 1 { "" } else { "s" },
        human_size(snapshot_file_byte_limit()),
        preview,
        suffix
    ));
    notes
}

fn human_size(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;
    if bytes >= MIB {
        if bytes % MIB == 0 {
            format!("{} MiB", bytes / MIB)
        } else {
            format!("{:.1} MiB", bytes as f64 / MIB as f64)
        }
    } else if bytes >= KIB {
        if bytes % KIB == 0 {
            format!("{} KiB", bytes / KIB)
        } else {
            format!("{:.1} KiB", bytes as f64 / KIB as f64)
        }
    } else {
        format!("{bytes} bytes")
    }
}

#[allow(clippy::too_many_arguments)]
fn settle_observations_after_exit(
    root_pid: u32,
    known_process_ids: &mut HashSet<u32>,
    process_samples: &mut Vec<ProcessSample>,
    network_samples: &mut Vec<crate::NetworkSample>,
    before: &HashMap<String, SnapshotEntry>,
    docker_before: &Result<DockerSnapshot>,
    cwd: &std::path::Path,
    live_files: &mut Vec<FileChange>,
    live_docker: &mut Option<crate::DockerSummary>,
    live_risks: &mut Vec<RiskNote>,
    live_summary: &mut Summary,
) {
    for _ in 0..24 {
        thread::sleep(Duration::from_millis(12));
        let observed_at = Utc::now();
        let current_processes =
            read_process_tree_sample_with_known(root_pid, known_process_ids, observed_at);
        known_process_ids.extend(current_processes.iter().map(|process| process.pid));
        network_samples.extend(read_network_samples(&current_processes, observed_at));
        let ss_samples = read_network_samples_ss(root_pid, &current_processes, observed_at);
        known_process_ids.extend(ss_samples.iter().filter_map(|sample| sample.pid));
        network_samples.extend(ss_samples);
        process_samples.extend(current_processes);
    }
    refresh_live_side_effects(
        root_pid,
        before,
        docker_before,
        cwd,
        process_samples,
        network_samples,
        live_files,
        live_docker,
        live_risks,
        live_summary,
    );
}

fn spawn_output_reader<R>(
    mut reader: R,
    live: Arc<Mutex<Vec<u8>>>,
) -> thread::JoinHandle<Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut all = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            let read = reader.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            all.extend_from_slice(&chunk[..read]);
            if let Ok(mut bytes) = live.lock() {
                bytes.extend_from_slice(&chunk[..read]);
                if bytes.len() > 64 * 1024 {
                    let drop_len = bytes.len() - 64 * 1024;
                    bytes.drain(..drop_len);
                }
            }
        }
        Ok(all)
    })
}

fn join_output_reader(handle: Option<thread::JoinHandle<Result<Vec<u8>>>>) -> Result<Vec<u8>> {
    match handle {
        Some(handle) => handle
            .join()
            .map_err(|_| anyhow!("output reader thread panicked"))?,
        None => Ok(Vec::new()),
    }
}

#[allow(clippy::too_many_arguments)]
fn progress_snapshot(
    root_pid: u32,
    command: &[String],
    command_display: &str,
    mode: ObservationMode,
    started_at: DateTime<Utc>,
    stdout_live: &Arc<Mutex<Vec<u8>>>,
    stderr_live: &Arc<Mutex<Vec<u8>>>,
    process_samples: &[ProcessSample],
    network_samples: &[crate::NetworkSample],
    summary: &Summary,
    files: &[FileChange],
    docker: Option<&crate::DockerSummary>,
    risks: &[RiskNote],
) -> RunProgress {
    let observed_at = Utc::now();
    let processes = merge_processes_with_network_samples(
        summarize_process_samples(
            root_pid,
            command,
            started_at,
            observed_at,
            process_samples.to_vec(),
        ),
        network_samples,
        root_pid,
    );
    let processes_seen = processes
        .iter()
        .filter(|process| process.pid != root_pid)
        .count();
    let network = summarize_network_samples(network_samples.to_vec());
    let mut events = vec![TimelineEvent {
        at: started_at,
        kind: "command_started".to_string(),
        title: "Process started".to_string(),
        detail: Some(command_display.to_string()),
        severity: Severity::Info,
        related_path: None,
        related_pid: Some(root_pid),
    }];
    events.extend(progress_process_events(&processes, root_pid));
    events.extend(network_events(&network));
    events.sort_by_key(|event| event.at);
    RunProgress {
        command_display: command_display.to_string(),
        mode,
        started_at,
        elapsed_ms: (observed_at - started_at).num_milliseconds().max(0) as u64,
        stdout_preview: preview_bytes(stdout_live),
        stderr_preview: preview_bytes(stderr_live),
        summary: summary.clone(),
        processes_seen,
        files: files.to_vec(),
        network_hosts: unique_hosts(&network),
        ports_opened: network
            .iter()
            .filter(|event| matches!(event.direction, NetworkDirection::Listening))
            .count(),
        processes,
        network,
        docker: docker.cloned(),
        risks: risks.to_vec(),
        events,
    }
}

fn progress_process_events(processes: &[ProcessInfo], root_pid: u32) -> Vec<TimelineEvent> {
    processes
        .iter()
        .filter(|process| process.pid != root_pid)
        .filter_map(|process| {
            process.started_at.map(|started_at| TimelineEvent {
                at: started_at,
                kind: "process_observed".to_string(),
                title: process.command.clone(),
                detail: Some(format!("pid {}", process.pid)),
                severity: Severity::Info,
                related_path: None,
                related_pid: Some(process.pid),
            })
        })
        .collect()
}

fn preview_bytes(bytes: &Arc<Mutex<Vec<u8>>>) -> String {
    let snapshot = bytes.lock().map(|value| value.clone()).unwrap_or_default();
    let text = String::from_utf8_lossy(&snapshot).into_owned();
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 4000 {
        text
    } else {
        chars[chars.len() - 4000..].iter().collect()
    }
}

fn terminate_process_tree(root_pid: u32) {
    #[cfg(target_os = "linux")]
    {
        let all = read_proc_processes();
        let mut related = HashSet::from([root_pid]);
        let mut changed = true;
        while changed {
            changed = false;
            for (pid, process) in &all {
                if process.ppid.is_some_and(|ppid| related.contains(&ppid)) && related.insert(*pid)
                {
                    changed = true;
                }
            }
        }

        let mut pids: Vec<u32> = related.into_iter().collect();
        pids.sort_unstable_by(|left, right| right.cmp(left));
        if !pids.is_empty() {
            let pid_args: Vec<String> = pids.iter().map(|pid| pid.to_string()).collect();
            let _ = Command::new("kill").arg("-TERM").args(&pid_args).status();
            thread::sleep(Duration::from_millis(150));
            let _ = Command::new("kill").arg("-KILL").args(&pid_args).status();
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = root_pid;
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::run_observed_shell_command_in_mode;
    use crate::ObservationMode;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn observed_shell_command_handles_working_directory_with_spaces() {
        let _guard = env_lock().lock().expect("lock runtime env");
        let fixture = RuntimeFixture::new("space dir");

        let (report, _paths) = fixture.in_workspace(|| {
            run_observed_shell_command_in_mode(
                "mkdir -p 'nested dir' && printf receipt > 'nested dir/output.txt'".to_string(),
                ObservationMode::Normal,
            )
        });

        assert_eq!(report.run.exit_code, Some(0));
        assert!(report
            .files
            .iter()
            .any(|file| file.path == "nested dir/output.txt"));
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("Snapshot controls")));
    }

    #[cfg(unix)]
    #[test]
    fn observed_shell_command_skips_unreadable_child_directories() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = env_lock().lock().expect("lock runtime env");
        let fixture = RuntimeFixture::new("unreadable runtime");
        let blocked = fixture.workspace.join("blocked");
        std::fs::create_dir_all(&blocked).expect("blocked dir");
        std::fs::write(blocked.join("hidden.txt"), "hidden").expect("hidden file");
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))
            .expect("make blocked unreadable");

        let result = fixture.in_workspace(|| {
            run_observed_shell_command_in_mode(
                "printf visible > visible.txt".to_string(),
                ObservationMode::Normal,
            )
        });

        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755))
            .expect("restore blocked permissions");

        let (report, _paths) = result;
        assert_eq!(report.run.exit_code, Some(0));
        assert!(report.files.iter().any(|file| file.path == "visible.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn deep_mode_falls_back_to_normal_when_strace_is_missing() {
        use std::os::unix::fs::symlink;

        let _guard = env_lock().lock().expect("lock runtime env");
        let fixture = RuntimeFixture::new("deep fallback");
        let bin_dir = fixture.workspace.join("test-bin");
        std::fs::create_dir_all(&bin_dir).expect("test bin");
        symlink("/bin/sh", bin_dir.join("sh")).expect("link sh");

        let (report, _paths) = fixture.with_path(bin_dir.to_string_lossy().as_ref(), || {
            fixture.in_workspace(|| {
                run_observed_shell_command_in_mode(
                    "printf fallback > fallback.txt".to_string(),
                    ObservationMode::Deep,
                )
            })
        });

        assert_eq!(report.run.exit_code, Some(0));
        assert_eq!(report.run.mode, ObservationMode::Normal);
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("fell back to normal observation")));
        assert!(report.files.iter().any(|file| file.path == "fallback.txt"));
    }

    struct RuntimeFixture {
        workspace: PathBuf,
        previous_cwd: PathBuf,
        previous_data_home: Option<std::ffi::OsString>,
    }

    impl RuntimeFixture {
        fn new(name: &str) -> Self {
            let root = env::temp_dir().join(format!(
                "runglass-runtime-{name}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time")
                    .as_nanos()
            ));
            let workspace = root.join("workspace");
            std::fs::create_dir_all(&workspace).expect("workspace");
            let previous_cwd = env::current_dir().expect("cwd");
            let previous_data_home = env::var_os("RUNGLASS_DATA_HOME");
            env::set_var("RUNGLASS_DATA_HOME", root.join("data-home"));
            Self {
                workspace,
                previous_cwd,
                previous_data_home,
            }
        }

        fn in_workspace<T>(&self, action: impl FnOnce() -> anyhow::Result<T>) -> T {
            env::set_current_dir(&self.workspace).expect("set cwd");
            let result = action();
            env::set_current_dir(&self.previous_cwd).expect("restore cwd");
            result.expect("observed command")
        }

        fn with_path<T>(&self, value: &str, action: impl FnOnce() -> T) -> T {
            let previous = env::var_os("PATH");
            env::set_var("PATH", value);
            let result = action();
            if let Some(previous) = previous {
                env::set_var("PATH", previous);
            } else {
                env::remove_var("PATH");
            }
            result
        }
    }

    impl Drop for RuntimeFixture {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.previous_cwd);
            if let Some(previous) = &self.previous_data_home {
                env::set_var("RUNGLASS_DATA_HOME", previous);
            } else {
                env::remove_var("RUNGLASS_DATA_HOME");
            }
            if let Some(root) = self.workspace.parent() {
                let _ = std::fs::remove_dir_all(root);
            }
        }
    }
}
