use std::collections::{HashMap, HashSet};
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::{NetworkSample, ProcessInfo, ProcessSample};

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
pub(crate) struct ProcProcess {
    pub(crate) ppid: Option<u32>,
    pub(crate) command: String,
    pub(crate) argv: Vec<String>,
}
pub fn read_process_tree_sample(root_pid: u32, observed_at: DateTime<Utc>) -> Vec<ProcessSample> {
    read_process_tree_sample_with_known(root_pid, &HashSet::new(), observed_at)
}

pub(crate) fn read_process_tree_sample_with_known(
    root_pid: u32,
    known_pids: &HashSet<u32>,
    observed_at: DateTime<Utc>,
) -> Vec<ProcessSample> {
    #[cfg(target_os = "linux")]
    {
        let all = read_proc_processes();
        if all.is_empty() {
            return Vec::new();
        }

        let mut related = HashSet::from([root_pid]);
        related.extend(known_pids.iter().copied());
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
        pids.sort_unstable();
        pids.into_iter()
            .filter_map(|pid| {
                all.get(&pid).map(|process| ProcessSample {
                    pid,
                    ppid: process.ppid,
                    command: process.command.clone(),
                    argv: process.argv.clone(),
                    observed_at,
                })
            })
            .collect()
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root_pid, known_pids, observed_at);
        Vec::new()
    }
}

pub(crate) fn summarize_process_samples(
    root_pid: u32,
    command: &[String],
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    samples: Vec<ProcessSample>,
) -> Vec<ProcessInfo> {
    #[derive(Debug)]
    struct Aggregate {
        ppid: Option<u32>,
        command: String,
        argv: Vec<String>,
        started_at: DateTime<Utc>,
        last_seen: DateTime<Utc>,
    }

    let mut aggregates: HashMap<u32, Aggregate> = HashMap::new();
    for sample in samples {
        let entry = aggregates.entry(sample.pid).or_insert_with(|| Aggregate {
            ppid: sample.ppid,
            command: sample.command.clone(),
            argv: sample.argv.clone(),
            started_at: sample.observed_at,
            last_seen: sample.observed_at,
        });
        entry.last_seen = sample.observed_at;
        if should_refresh_process_identity(
            &entry.command,
            &entry.argv,
            &sample.command,
            &sample.argv,
        ) {
            entry.argv = sample.argv.clone();
            entry.command = sample.command.clone();
        }
        if entry.ppid.is_none() {
            entry.ppid = sample.ppid;
        }
    }

    aggregates.entry(root_pid).or_insert_with(|| Aggregate {
        ppid: None,
        command: command
            .first()
            .cloned()
            .unwrap_or_else(|| "command".to_string()),
        argv: command.to_vec(),
        started_at,
        last_seen: ended_at,
    });

    let mut processes: Vec<ProcessInfo> = aggregates
        .into_iter()
        .map(|(pid, aggregate)| ProcessInfo {
            pid,
            ppid: aggregate.ppid,
            command: aggregate.command,
            argv: aggregate.argv,
            started_at: Some(aggregate.started_at),
            exited_at: Some(aggregate.last_seen),
            exit_code: None,
            observed_by: "proc_polling".to_string(),
        })
        .collect();

    processes.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then_with(|| left.pid.cmp(&right.pid))
    });
    processes
}

pub(crate) fn merge_processes_with_network_samples(
    mut processes: Vec<ProcessInfo>,
    network_samples: &[NetworkSample],
    root_pid: u32,
) -> Vec<ProcessInfo> {
    let seen: HashSet<u32> = processes.iter().map(|process| process.pid).collect();
    let mut synthetic = HashMap::<u32, ProcessInfo>::new();

    for sample in network_samples {
        let Some(pid) = sample.pid else {
            continue;
        };
        if pid == root_pid || seen.contains(&pid) {
            continue;
        }
        let entry = synthetic.entry(pid).or_insert_with(|| ProcessInfo {
            pid,
            ppid: Some(root_pid),
            command: sample
                .process_name
                .clone()
                .unwrap_or_else(|| "observed-process".to_string()),
            argv: sample
                .process_name
                .clone()
                .map(|name| vec![name])
                .unwrap_or_default(),
            started_at: Some(sample.observed_at),
            exited_at: Some(sample.observed_at),
            exit_code: None,
            observed_by: "ss_sampling".to_string(),
        });
        if entry.command.is_empty() {
            entry.command = sample
                .process_name
                .clone()
                .unwrap_or_else(|| "observed-process".to_string());
        }
        entry.started_at = Some(
            entry
                .started_at
                .map(|current| current.min(sample.observed_at))
                .unwrap_or(sample.observed_at),
        );
        entry.exited_at = Some(
            entry
                .exited_at
                .map(|current| current.max(sample.observed_at))
                .unwrap_or(sample.observed_at),
        );
    }

    processes.extend(synthetic.into_values());
    processes.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then_with(|| left.pid.cmp(&right.pid))
    });
    processes
}

fn should_refresh_process_identity(
    current_command: &str,
    current_argv: &[String],
    next_command: &str,
    next_argv: &[String],
) -> bool {
    if current_command.is_empty() && !next_command.is_empty() {
        return true;
    }
    if current_argv.is_empty() && !next_argv.is_empty() {
        return true;
    }
    if current_command == next_command && current_argv == next_argv {
        return false;
    }
    if is_generic_wrapper_command(current_command) && !is_generic_wrapper_command(next_command) {
        return true;
    }
    if current_argv.len() <= 1 && next_argv.len() > current_argv.len() {
        return true;
    }
    current_command != next_command && !next_command.is_empty()
}

fn is_generic_wrapper_command(command: &str) -> bool {
    matches!(
        command,
        "sh" | "bash" | "dash" | "env" | "sudo" | "timeout" | "nohup"
    )
}

pub(crate) fn count_child_process_samples(
    process_samples: &[ProcessSample],
    root_pid: u32,
) -> usize {
    process_samples
        .iter()
        .map(|sample| sample.pid)
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|pid| *pid != root_pid)
        .count()
}

#[cfg(target_os = "linux")]
pub(crate) fn read_proc_processes() -> HashMap<u32, ProcProcess> {
    let mut processes = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return processes;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let path = entry.path();
        let stat = match fs::read_to_string(path.join("stat")) {
            Ok(stat) => stat,
            Err(_) => continue,
        };
        let ppid = parse_ppid(&stat);
        let argv = match fs::read(path.join("cmdline")) {
            Ok(bytes) => parse_cmdline(&bytes),
            Err(_) => Vec::new(),
        };
        let command = if let Some(first) = argv.first() {
            Path::new(first)
                .file_name()
                .and_then(|segment| segment.to_str())
                .unwrap_or(first)
                .to_string()
        } else {
            fs::read_to_string(path.join("comm"))
                .map(|value| value.trim().to_string())
                .unwrap_or_else(|_| pid.to_string())
        };

        processes.insert(
            pid,
            ProcProcess {
                ppid,
                command,
                argv,
            },
        );
    }

    processes
}

#[cfg(target_os = "linux")]
fn parse_ppid(stat: &str) -> Option<u32> {
    let end = stat.rfind(')')?;
    let rest = stat.get(end + 2..)?;
    let mut fields = rest.split_whitespace();
    let _state = fields.next()?;
    fields.next()?.parse().ok()
}

#[cfg(target_os = "linux")]
fn parse_cmdline(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}
