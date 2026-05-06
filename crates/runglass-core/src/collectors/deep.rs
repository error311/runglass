use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};

use crate::{
    NetworkDirection, NetworkSample, ObservationMode, ProcessInfo, ProcessSample, RunPaths,
};

#[derive(Debug, Default)]
pub(crate) struct DeepTraceCapture {
    pub(crate) root_pid: Option<u32>,
    pub(crate) processes: Vec<ProcessInfo>,
    pub(crate) network_samples: Vec<NetworkSample>,
}
pub(crate) fn prepare_deep_trace_prefix(paths: &RunPaths) -> Result<PathBuf> {
    let trace_dir = paths.run_dir.join("deep-trace");
    fs::create_dir_all(&trace_dir)?;
    Ok(trace_dir.join("trace"))
}

pub(crate) fn wrap_command_for_mode(
    command: &[String],
    mode: ObservationMode,
    trace_prefix: Option<&Path>,
) -> Result<Vec<String>> {
    match mode {
        ObservationMode::Normal => Ok(command.to_vec()),
        ObservationMode::Deep => {
            let prefix =
                trace_prefix.ok_or_else(|| anyhow!("deep mode requires a trace output path"))?;
            Ok(vec![
                "strace".to_string(),
                "-ff".to_string(),
                "-qq".to_string(),
                "-ttt".to_string(),
                "-s".to_string(),
                "256".to_string(),
                "-e".to_string(),
                "trace=execve,clone,fork,vfork,connect,bind,listen".to_string(),
                "-o".to_string(),
                prefix.display().to_string(),
                "--".to_string(),
            ]
            .into_iter()
            .chain(command.iter().cloned())
            .collect())
        }
    }
}

pub(crate) fn merge_deep_process_samples(
    mut samples: Vec<ProcessSample>,
    processes: &[ProcessInfo],
) -> Vec<ProcessSample> {
    let mut seen = HashSet::new();
    for sample in &samples {
        seen.insert((sample.pid, sample.observed_at));
    }
    for process in processes {
        if let Some(started_at) = process.started_at {
            let sample = ProcessSample {
                pid: process.pid,
                ppid: process.ppid,
                command: process.command.clone(),
                argv: process.argv.clone(),
                observed_at: started_at,
            };
            if seen.insert((sample.pid, sample.observed_at)) {
                samples.push(sample);
            }
        }
        if let Some(exited_at) = process.exited_at {
            let sample = ProcessSample {
                pid: process.pid,
                ppid: process.ppid,
                command: process.command.clone(),
                argv: process.argv.clone(),
                observed_at: exited_at,
            };
            if seen.insert((sample.pid, sample.observed_at)) {
                samples.push(sample);
            }
        }
    }
    samples
}

pub(crate) fn parse_deep_trace_capture(prefix: &Path) -> Result<DeepTraceCapture> {
    let parent = prefix
        .parent()
        .ok_or_else(|| anyhow!("deep trace prefix has no parent directory"))?;
    let base = prefix
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("deep trace prefix has no file name"))?;

    let mut pid_state = HashMap::<u32, ProcessInfo>::new();
    let mut network_samples = Vec::new();
    let mut trace_files = fs::read_dir(parent)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == base || name.starts_with(&format!("{base}.")))
        })
        .collect::<Vec<_>>();
    trace_files.sort();

    for path in trace_files {
        let Some(pid) = trace_pid_from_path(&path, base) else {
            continue;
        };
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        for line in content.lines() {
            let Some((at, body)) = split_trace_timestamp(line) else {
                continue;
            };
            if body.starts_with("execve(") {
                let (command, argv) = parse_execve_line(body);
                let entry = pid_state.entry(pid).or_insert_with(|| ProcessInfo {
                    pid,
                    ppid: None,
                    command: command.clone(),
                    argv: argv.clone(),
                    started_at: Some(at),
                    exited_at: None,
                    exit_code: None,
                    observed_by: "strace".to_string(),
                });
                entry.started_at = Some(
                    entry
                        .started_at
                        .map(|current| current.min(at))
                        .unwrap_or(at),
                );
                if !command.is_empty() {
                    entry.command = command;
                }
                if !argv.is_empty() {
                    entry.argv = argv;
                }
                continue;
            }

            if let Some(child_pid) = parse_child_pid(body) {
                let entry = pid_state.entry(child_pid).or_insert_with(|| ProcessInfo {
                    pid: child_pid,
                    ppid: Some(pid),
                    command: String::new(),
                    argv: Vec::new(),
                    started_at: Some(at),
                    exited_at: None,
                    exit_code: None,
                    observed_by: "strace".to_string(),
                });
                entry.ppid = Some(pid);
                entry.started_at = Some(
                    entry
                        .started_at
                        .map(|current| current.min(at))
                        .unwrap_or(at),
                );
                continue;
            }

            if let Some((direction, ip, port, protocol)) = parse_socket_trace(body) {
                let process_name = pid_state
                    .get(&pid)
                    .map(|process| process.command.clone())
                    .filter(|command| !command.is_empty());
                network_samples.push(NetworkSample {
                    ip,
                    port,
                    protocol,
                    pid: Some(pid),
                    process_name,
                    observed_at: at,
                    direction,
                });
                continue;
            }

            if let Some(exit_code) = parse_trace_exit(body) {
                let entry = pid_state.entry(pid).or_insert_with(|| ProcessInfo {
                    pid,
                    ppid: None,
                    command: String::new(),
                    argv: Vec::new(),
                    started_at: Some(at),
                    exited_at: Some(at),
                    exit_code: Some(exit_code),
                    observed_by: "strace".to_string(),
                });
                entry.exited_at = Some(at);
                entry.exit_code = Some(exit_code);
            }
        }
    }

    let mut processes = pid_state
        .into_values()
        .filter(|process| !process.command.eq("strace"))
        .collect::<Vec<_>>();
    processes.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then_with(|| left.pid.cmp(&right.pid))
    });

    Ok(DeepTraceCapture {
        root_pid: processes.first().map(|process| process.pid),
        processes,
        network_samples,
    })
}

fn trace_pid_from_path(path: &Path, base: &str) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    if name == base {
        return None;
    }
    let pid = name.strip_prefix(&format!("{base}."))?;
    pid.parse().ok()
}

fn split_trace_timestamp(line: &str) -> Option<(DateTime<Utc>, &str)> {
    let (stamp, body) = line.split_once(' ')?;
    let at = parse_trace_timestamp(stamp)?;
    Some((at, body.trim_start()))
}

fn parse_trace_timestamp(stamp: &str) -> Option<DateTime<Utc>> {
    let value: f64 = stamp.parse().ok()?;
    let seconds = value.trunc() as i64;
    let nanos = ((value.fract() * 1_000_000_000.0).round() as u32).min(999_999_999);
    Utc.timestamp_opt(seconds, nanos).single()
}

fn parse_execve_line(body: &str) -> (String, Vec<String>) {
    let path = extract_quoted_after(body, "execve(").unwrap_or_default();
    let argv_block = body
        .split_once(", [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(argv, _)| argv)
        .unwrap_or_default();
    let argv = parse_quoted_list(argv_block);
    let command = argv
        .first()
        .and_then(|value| {
            Path::new(value)
                .file_name()
                .and_then(|segment| segment.to_str())
                .map(str::to_owned)
        })
        .or_else(|| {
            Path::new(&path)
                .file_name()
                .and_then(|segment| segment.to_str())
                .map(str::to_owned)
        })
        .unwrap_or(path);
    (command, argv)
}

fn parse_quoted_list(input: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape = false;

    for ch in input.chars() {
        if !in_string {
            if ch == '"' {
                in_string = true;
                current.clear();
            }
            continue;
        }

        if escape {
            current.push(ch);
            escape = false;
            continue;
        }

        match ch {
            '\\' => escape = true,
            '"' => {
                values.push(current.clone());
                current.clear();
                in_string = false;
            }
            _ => current.push(ch),
        }
    }

    values
}

fn extract_quoted_after(input: &str, prefix: &str) -> Option<String> {
    let start = input.strip_prefix(prefix)?;
    let start = start.strip_prefix('"')?;
    let mut value = String::new();
    let mut escape = false;
    for ch in start.chars() {
        if escape {
            value.push(ch);
            escape = false;
            continue;
        }
        match ch {
            '\\' => escape = true,
            '"' => return Some(value),
            _ => value.push(ch),
        }
    }
    None
}

fn parse_child_pid(body: &str) -> Option<u32> {
    if !(body.starts_with("clone(") || body.starts_with("fork(") || body.starts_with("vfork(")) {
        return None;
    }
    let result = body.rsplit_once("= ")?.1.trim();
    result.parse().ok()
}

fn parse_socket_trace(body: &str) -> Option<(NetworkDirection, String, u16, String)> {
    if body.starts_with("connect(") {
        let (ip, port) = parse_sockaddr(body)?;
        if port == 0 || is_wildcard_host(&ip) {
            return None;
        }
        return Some((NetworkDirection::Outbound, ip, port, "tcp".to_string()));
    }
    if body.starts_with("bind(") {
        let (ip, port) = parse_sockaddr(body)?;
        if port == 0 {
            return None;
        }
        return Some((NetworkDirection::Listening, ip, port, "tcp".to_string()));
    }
    None
}

fn is_wildcard_host(host: &str) -> bool {
    matches!(host, "0.0.0.0" | "::")
}

fn parse_sockaddr(body: &str) -> Option<(String, u16)> {
    if body.contains("AF_INET6") {
        let port = extract_between(body, "sin6_port=htons(", ")")?
            .parse()
            .ok()?;
        let ip = extract_quoted_after_token(body, "inet_pton(AF_INET6, ")?;
        return Some((ip, port));
    }
    if body.contains("AF_INET") {
        let port = extract_between(body, "sin_port=htons(", ")")?
            .parse()
            .ok()?;
        let ip = extract_quoted_after_token(body, "inet_addr(")
            .or_else(|| extract_between(body, "sin_addr=inet_addr(\"", "\")").map(str::to_owned))
            .or_else(|| extract_between(body, "sin_addr=inet_addr(", ")").map(str::to_owned))?;
        return Some((ip.trim_matches('"').to_string(), port));
    }
    None
}

fn extract_between<'a>(input: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let tail = input.split_once(start)?.1;
    tail.split_once(end).map(|(value, _)| value)
}

fn extract_quoted_after_token(input: &str, token: &str) -> Option<String> {
    let tail = input.split_once(token)?.1;
    let start = tail.find('"')?;
    let quoted = &tail[start + 1..];
    let mut value = String::new();
    let mut escape = false;
    for ch in quoted.chars() {
        if escape {
            value.push(ch);
            escape = false;
            continue;
        }
        match ch {
            '\\' => escape = true,
            '"' => return Some(value),
            _ => value.push(ch),
        }
    }
    None
}

fn parse_trace_exit(body: &str) -> Option<i32> {
    let exit = body.strip_prefix("+++ exited with ")?;
    let code = exit.split_whitespace().next()?;
    code.parse().ok()
}
