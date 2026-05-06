use std::env;
use std::path::Path;
use std::process::Output;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::reporting::timeline::{docker_events, file_events, network_events, process_events};
use crate::{
    DockerSummary, FileChange, NetworkEvent, ObservationMode, ProcessInfo, RunMeta, RunReport,
    RunStatus, Severity, TimelineEvent,
};

use super::risks::{build_summary, derive_risk_level, derive_risks};

#[allow(clippy::too_many_arguments)]
pub fn build_command_report(
    run_id: String,
    command: &[String],
    cwd: &Path,
    output: &Output,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    duration: Duration,
    root_pid: Option<u32>,
    stdout_path: &Path,
    stderr_path: &Path,
    mut processes: Vec<ProcessInfo>,
    network: Vec<NetworkEvent>,
    docker: Option<DockerSummary>,
    files: Vec<FileChange>,
    mode: ObservationMode,
    run_status: RunStatus,
    extra_limitations: Vec<String>,
) -> RunReport {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_code = output.status.code();

    if let Some(root_pid) = root_pid {
        if let Some(root) = processes.iter_mut().find(|process| process.pid == root_pid) {
            root.exit_code = exit_code;
            root.exited_at = Some(ended_at);
        }
    }

    let risks = derive_risks(&files, &network, docker.as_ref(), &run_status, exit_code);
    let risk_level = derive_risk_level(&risks, &files);

    let mut events = vec![
        TimelineEvent {
            at: started_at,
            kind: "command_started".to_string(),
            title: "Command started".to_string(),
            detail: Some(command_display(command)),
            severity: Severity::Info,
            related_path: None,
            related_pid: None,
        },
        TimelineEvent {
            at: ended_at,
            kind: if matches!(run_status, RunStatus::Interrupted) {
                "command_interrupted".to_string()
            } else {
                "command_exited".to_string()
            },
            title: if matches!(run_status, RunStatus::Interrupted) {
                "Command interrupted".to_string()
            } else {
                "Command exited".to_string()
            },
            detail: Some(format!("Exit code: {}", exit_code.unwrap_or(-1))),
            severity: if matches!(run_status, RunStatus::Interrupted) {
                Severity::Warning
            } else if output.status.success() {
                Severity::Success
            } else {
                Severity::Warning
            },
            related_path: None,
            related_pid: None,
        },
    ];
    events.extend(process_events(&processes, root_pid));
    events.extend(network_events(&network));
    events.extend(docker_events(docker.as_ref(), ended_at));
    events.extend(file_events(&files, ended_at));
    events.sort_by_key(|event| event.at);

    if processes.is_empty() {
        processes.push(ProcessInfo {
            pid: root_pid.unwrap_or(0),
            ppid: None,
            command: command
                .first()
                .cloned()
                .unwrap_or_else(|| "command".to_string()),
            argv: command.to_vec(),
            started_at: Some(started_at),
            exited_at: Some(ended_at),
            exit_code,
            observed_by: "command_runner".to_string(),
        });
    }

    RunReport {
        schema_version: "0.1.0".to_string(),
        run: RunMeta {
            id: run_id,
            command_display: command_display(command),
            argv: command.to_vec(),
            cwd: cwd.display().to_string(),
            shell: env::var("SHELL").ok(),
            mode,
            started_at,
            ended_at: Some(ended_at),
            duration_ms: Some(duration.as_millis() as u64),
            exit_code,
            status: run_status.clone(),
        },
        summary: build_summary(
            &files,
            processes
                .iter()
                .filter(|process| Some(process.pid) != root_pid)
                .count(),
            &network,
            docker.as_ref(),
            risk_level,
        ),
        events,
        processes,
        files,
        network,
        docker,
        risks,
        stdout_path: Some(stdout_path.display().to_string()),
        stderr_path: Some(stderr_path.display().to_string()),
        stdout: (!stdout.is_empty()).then_some(stdout),
        stderr: (!stderr.is_empty()).then_some(stderr),
        limitations: {
            let mut limitations = match mode {
                ObservationMode::Normal => vec![
                    "Process observations in normal mode are derived from adaptive /proc polling."
                        .to_string(),
                    "Very short-lived processes can still be missed between polling intervals."
                        .to_string(),
                    "Observed network activity in normal mode is derived from /proc socket polling plus ss sampling, and PID attribution can still be incomplete.".to_string(),
                ],
                ObservationMode::Deep => vec![
                    "Deep mode supplements normal observation with strace-based exec and socket tracing on Linux.".to_string(),
                    "Deep mode improves short-lived process and outbound socket fidelity, but it still focuses on the traced command tree rather than system-wide activity.".to_string(),
                ],
            };
            limitations.push(
                "File changes are currently collected with a scoped before/after snapshot of the working directory.".to_string(),
            );
            if matches!(run_status, RunStatus::Interrupted) {
                limitations.push(
                    "This run was interrupted before completion, so collected effects may be partial."
                        .to_string(),
                );
            }
            limitations.extend(extra_limitations);
            limitations
        },
    }
}

fn command_display(command: &[String]) -> String {
    command
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '='))
    {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
}
