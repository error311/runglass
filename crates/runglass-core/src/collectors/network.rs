use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::collections::HashSet;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::net::{Ipv4Addr, Ipv6Addr};
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::Command;

use chrono::{DateTime, Utc};

#[cfg(target_os = "linux")]
use crate::collectors::processes::{read_proc_processes, ProcProcess};
use crate::{NetworkDirection, NetworkEvent, NetworkSample, ProcessSample};
pub fn read_network_samples(
    processes: &[ProcessSample],
    observed_at: DateTime<Utc>,
) -> Vec<NetworkSample> {
    #[cfg(target_os = "linux")]
    {
        let pid_names: HashMap<u32, String> = processes
            .iter()
            .map(|process| (process.pid, process.command.clone()))
            .collect();
        if pid_names.is_empty() {
            return Vec::new();
        }

        let inode_map = socket_inode_map(&pid_names);
        let mut samples = Vec::new();
        samples.extend(read_socket_table(
            "/proc/net/tcp",
            "tcp",
            false,
            observed_at,
            &inode_map,
        ));
        samples.extend(read_socket_table(
            "/proc/net/tcp6",
            "tcp",
            true,
            observed_at,
            &inode_map,
        ));
        samples.extend(read_socket_table(
            "/proc/net/udp",
            "udp",
            false,
            observed_at,
            &inode_map,
        ));
        samples.extend(read_socket_table(
            "/proc/net/udp6",
            "udp",
            true,
            observed_at,
            &inode_map,
        ));
        samples
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (processes, observed_at);
        Vec::new()
    }
}

pub fn read_network_samples_ss(
    root_pid: u32,
    processes: &[ProcessSample],
    observed_at: DateTime<Utc>,
) -> Vec<NetworkSample> {
    #[cfg(target_os = "linux")]
    {
        let pid_names: HashMap<u32, String> = processes
            .iter()
            .map(|process| (process.pid, process.command.clone()))
            .collect();
        let proc_map = read_proc_processes();
        let mut samples = Vec::new();
        samples.extend(read_ss_table(
            &["-H", "-tanp"],
            observed_at,
            root_pid,
            &pid_names,
            &proc_map,
            NetworkDirection::Outbound,
        ));
        samples.extend(read_ss_table(
            &["-H", "-uanp"],
            observed_at,
            root_pid,
            &pid_names,
            &proc_map,
            NetworkDirection::Unknown,
        ));
        samples
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root_pid, processes, observed_at);
        Vec::new()
    }
}

pub fn summarize_network_samples(samples: Vec<NetworkSample>) -> Vec<NetworkEvent> {
    let mut grouped = HashMap::<
        (
            String,
            u16,
            String,
            Option<u32>,
            Option<String>,
            NetworkDirection,
        ),
        NetworkEvent,
    >::new();
    for sample in samples {
        let key = (
            sample.ip.clone(),
            sample.port,
            sample.protocol.clone(),
            sample.pid,
            sample.process_name.clone(),
            sample.direction.clone(),
        );
        let entry = grouped.entry(key).or_insert_with(|| NetworkEvent {
            host: None,
            ip: sample.ip.clone(),
            port: sample.port,
            protocol: sample.protocol.clone(),
            pid: sample.pid,
            process_name: sample.process_name.clone(),
            first_seen: sample.observed_at,
            last_seen: sample.observed_at,
            count: 0,
            direction: sample.direction.clone(),
        });
        entry.first_seen = entry.first_seen.min(sample.observed_at);
        entry.last_seen = entry.last_seen.max(sample.observed_at);
        entry.count += 1;
    }

    let mut events: Vec<NetworkEvent> = grouped.into_values().collect();
    events.sort_by(|left, right| {
        left.first_seen
            .cmp(&right.first_seen)
            .then_with(|| left.ip.cmp(&right.ip))
            .then_with(|| left.port.cmp(&right.port))
    });
    events
}

#[cfg(target_os = "linux")]
fn socket_inode_map(pid_names: &HashMap<u32, String>) -> HashMap<u64, (u32, String)> {
    let mut map = HashMap::new();
    for (pid, name) in pid_names {
        let fd_dir = PathBuf::from("/proc").join(pid.to_string()).join("fd");
        let Ok(entries) = fs::read_dir(fd_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            let Some(target) = target.to_str() else {
                continue;
            };
            if let Some(inode) = parse_socket_inode(target) {
                map.entry(inode).or_insert((*pid, name.clone()));
            }
        }
    }
    map
}

#[cfg(target_os = "linux")]
fn parse_socket_inode(target: &str) -> Option<u64> {
    let inode = target.strip_prefix("socket:[")?.strip_suffix(']')?;
    inode.parse().ok()
}

#[cfg(target_os = "linux")]
fn read_ss_table(
    args: &[&str],
    observed_at: DateTime<Utc>,
    root_pid: u32,
    pid_names: &HashMap<u32, String>,
    proc_map: &HashMap<u32, ProcProcess>,
    default_direction: NetworkDirection,
) -> Vec<NetworkSample> {
    let output = Command::new("ss").args(args).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let content = String::from_utf8_lossy(&output.stdout);
    content
        .lines()
        .filter_map(|line| {
            parse_ss_line(
                line,
                observed_at,
                root_pid,
                pid_names,
                proc_map,
                &default_direction,
            )
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn parse_ss_line(
    line: &str,
    observed_at: DateTime<Utc>,
    root_pid: u32,
    pid_names: &HashMap<u32, String>,
    proc_map: &HashMap<u32, ProcProcess>,
    default_direction: &NetworkDirection,
) -> Option<NetworkSample> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 6 {
        return None;
    }

    let protocol = fields[0];
    let state = fields[1];
    let local = fields[4];
    let peer = fields[5];
    let users = fields.iter().find(|field| field.starts_with("users:"))?;
    let pid = parse_ss_pid(users)?;
    let related = pid_names.contains_key(&pid) || process_descends_from(pid, root_pid, proc_map);
    if !related {
        return None;
    }
    let process_name = pid_names
        .get(&pid)
        .cloned()
        .or_else(|| proc_map.get(&pid).map(|process| process.command.clone()))
        .or_else(|| parse_ss_process_name(users));
    process_name.as_ref()?;

    let (local_ip, local_port) = parse_ss_addr(local)?;
    let (peer_ip, peer_port) = parse_ss_addr(peer).unwrap_or_else(|| ("0.0.0.0".to_string(), 0));
    let direction = classify_ss_direction(
        protocol,
        state,
        &local_ip,
        local_port,
        &peer_ip,
        peer_port,
        default_direction,
    );
    let (ip, port) = match direction {
        NetworkDirection::Listening => (local_ip, local_port),
        NetworkDirection::Outbound => (peer_ip, peer_port),
        NetworkDirection::Unknown => {
            if peer_port > 0 {
                (peer_ip, peer_port)
            } else {
                (local_ip, local_port)
            }
        }
    };

    Some(NetworkSample {
        ip,
        port,
        protocol: protocol.to_string(),
        pid: Some(pid),
        process_name,
        observed_at,
        direction,
    })
}

#[cfg(target_os = "linux")]
fn process_descends_from(pid: u32, root_pid: u32, proc_map: &HashMap<u32, ProcProcess>) -> bool {
    if pid == root_pid {
        return true;
    }

    let mut current = pid;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        let Some(process) = proc_map.get(&current) else {
            return false;
        };
        let Some(ppid) = process.ppid else {
            return false;
        };
        if ppid == root_pid {
            return true;
        }
        current = ppid;
    }

    false
}

#[cfg(target_os = "linux")]
fn parse_ss_pid(users: &str) -> Option<u32> {
    let pid_part = users.split("pid=").nth(1)?;
    let digits: String = pid_part
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

#[cfg(target_os = "linux")]
fn parse_ss_process_name(users: &str) -> Option<String> {
    let name = users.split('"').nth(1)?;
    Some(name.to_string())
}

#[cfg(target_os = "linux")]
fn parse_ss_addr(value: &str) -> Option<(String, u16)> {
    if value == "*" {
        return Some(("0.0.0.0".to_string(), 0));
    }
    let (host, port) = value.rsplit_once(':')?;
    let host = host.trim_matches(['[', ']']);
    let host = host.split('%').next().unwrap_or(host).to_string();
    Some((host, port.parse().ok()?))
}

#[cfg(target_os = "linux")]
fn classify_ss_direction(
    protocol: &str,
    state: &str,
    local_ip: &str,
    local_port: u16,
    remote_ip: &str,
    remote_port: u16,
    default_direction: &NetworkDirection,
) -> NetworkDirection {
    if state.eq_ignore_ascii_case("LISTEN") {
        return NetworkDirection::Listening;
    }
    if protocol.starts_with("udp") && state.eq_ignore_ascii_case("UNCONN") && remote_port == 0 {
        return NetworkDirection::Listening;
    }
    if remote_port > 0 && remote_ip != "0.0.0.0" && remote_ip != "::" {
        return NetworkDirection::Outbound;
    }
    if local_port > 0 && (local_ip == "0.0.0.0" || local_ip == "::" || local_ip == "127.0.0.1") {
        return NetworkDirection::Listening;
    }
    default_direction.clone()
}

#[cfg(target_os = "linux")]
fn read_socket_table(
    table_path: &str,
    protocol: &str,
    ipv6: bool,
    observed_at: DateTime<Utc>,
    inode_map: &HashMap<u64, (u32, String)>,
) -> Vec<NetworkSample> {
    let Ok(content) = fs::read_to_string(table_path) else {
        return Vec::new();
    };

    let mut samples = Vec::new();
    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 11 {
            continue;
        }

        let Some((local_ip, local_port)) = parse_socket_addr(fields[1], ipv6) else {
            continue;
        };
        let Some((remote_ip, remote_port)) = parse_socket_addr(fields[2], ipv6) else {
            continue;
        };
        let state = fields[3];
        let Some(inode) = fields
            .get(9)
            .and_then(|value| value.parse::<u64>().ok())
            .or_else(|| fields.get(10).and_then(|value| value.parse::<u64>().ok()))
        else {
            continue;
        };
        let Some((pid, process_name)) = inode_map.get(&inode) else {
            continue;
        };

        let direction = classify_direction(
            protocol,
            state,
            &local_ip,
            local_port,
            &remote_ip,
            remote_port,
        );
        let (ip, port) = match direction {
            NetworkDirection::Listening => (local_ip, local_port),
            NetworkDirection::Outbound => (remote_ip, remote_port),
            NetworkDirection::Unknown => {
                if remote_port != 0 {
                    (remote_ip, remote_port)
                } else {
                    (local_ip, local_port)
                }
            }
        };

        samples.push(NetworkSample {
            ip,
            port,
            protocol: protocol.to_string(),
            pid: Some(*pid),
            process_name: Some(process_name.clone()),
            observed_at,
            direction,
        });
    }

    samples
}

#[cfg(target_os = "linux")]
fn classify_direction(
    protocol: &str,
    state: &str,
    local_ip: &str,
    local_port: u16,
    remote_ip: &str,
    remote_port: u16,
) -> NetworkDirection {
    if protocol == "tcp" && state == "0A" {
        return NetworkDirection::Listening;
    }
    if protocol == "udp" && remote_port == 0 {
        return NetworkDirection::Listening;
    }
    if remote_port > 0 && remote_ip != "0.0.0.0" && remote_ip != "::" {
        return NetworkDirection::Outbound;
    }
    if local_port > 0 && (local_ip == "0.0.0.0" || local_ip == "::" || local_ip == "127.0.0.1") {
        return NetworkDirection::Listening;
    }
    NetworkDirection::Unknown
}

#[cfg(target_os = "linux")]
fn parse_socket_addr(value: &str, ipv6: bool) -> Option<(String, u16)> {
    let (ip_hex, port_hex) = value.split_once(':')?;
    let port = u16::from_str_radix(port_hex, 16).ok()?;
    let ip = if ipv6 {
        decode_ipv6(ip_hex)?
    } else {
        decode_ipv4(ip_hex)?
    };
    Some((ip, port))
}

#[cfg(target_os = "linux")]
fn decode_ipv4(hex: &str) -> Option<String> {
    if hex.len() != 8 {
        return None;
    }
    let raw = u32::from_str_radix(hex, 16).ok()?;
    let bytes = raw.to_le_bytes();
    Some(Ipv4Addr::from(bytes).to_string())
}

#[cfg(target_os = "linux")]
fn decode_ipv6(hex: &str) -> Option<String> {
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0_u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&hex[offset..offset + 2], 16).ok()?;
    }
    let mut normalized = [0_u8; 16];
    for chunk in 0..4 {
        let start = chunk * 4;
        normalized[start..start + 4].copy_from_slice(&[
            bytes[start + 3],
            bytes[start + 2],
            bytes[start + 1],
            bytes[start],
        ]);
    }
    Some(Ipv6Addr::from(normalized).to_string())
}
