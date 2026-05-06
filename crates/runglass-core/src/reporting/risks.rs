use std::collections::BTreeSet;

use crate::storage::slugify;
use crate::{
    DockerSummary, FileChange, FileChangeType, NetworkDirection, NetworkEvent, RiskEvidence,
    RiskLevel, RiskNote, RunStatus, Severity, Summary,
};

pub(crate) fn empty_summary() -> Summary {
    Summary {
        files_changed: 0,
        files_created: 0,
        files_modified: 0,
        files_deleted: 0,
        processes_seen: 0,
        network_hosts: 0,
        ports_opened: 0,
        docker_containers_created: 0,
        docker_images_pulled: 0,
        docker_volumes_created: 0,
        risk_level: RiskLevel::None,
    }
}

pub(crate) fn build_summary(
    files: &[FileChange],
    processes_seen: usize,
    network: &[NetworkEvent],
    docker: Option<&DockerSummary>,
    risk_level: RiskLevel,
) -> Summary {
    Summary {
        files_changed: files.len(),
        files_created: files
            .iter()
            .filter(|file| matches!(file.change_type, FileChangeType::Created))
            .count(),
        files_modified: files
            .iter()
            .filter(|file| matches!(file.change_type, FileChangeType::Modified))
            .count(),
        files_deleted: files
            .iter()
            .filter(|file| matches!(file.change_type, FileChangeType::Deleted))
            .count(),
        processes_seen,
        network_hosts: unique_hosts(network),
        ports_opened: network
            .iter()
            .filter(|event| matches!(event.direction, NetworkDirection::Listening))
            .count(),
        docker_containers_created: docker
            .map(|docker| docker.containers_created.len())
            .unwrap_or(0),
        docker_images_pulled: docker.map(|docker| docker.images_pulled.len()).unwrap_or(0),
        docker_volumes_created: docker
            .map(|docker| docker.volumes_created.len())
            .unwrap_or(0),
        risk_level,
    }
}

pub(crate) fn derive_risk_level(risks: &[RiskNote], files: &[FileChange]) -> RiskLevel {
    if risks
        .iter()
        .any(|risk| matches!(risk.severity, Severity::Danger))
    {
        RiskLevel::High
    } else if risks
        .iter()
        .any(|risk| matches!(risk.severity, Severity::Warning))
    {
        RiskLevel::Medium
    } else if !files.is_empty() {
        RiskLevel::Low
    } else {
        RiskLevel::None
    }
}

pub(crate) fn derive_risks(
    files: &[FileChange],
    network: &[NetworkEvent],
    docker: Option<&DockerSummary>,
    run_status: &RunStatus,
    exit_code: Option<i32>,
) -> Vec<RiskNote> {
    let mut risks = risks_from_files(files);
    if matches!(run_status, RunStatus::Interrupted) {
        risks.push(RiskNote {
            id: "command-interrupted".to_string(),
            title: "Command was interrupted".to_string(),
            detail: "The command stopped before it completed, so this receipt only reflects partial side effects.".to_string(),
            severity: Severity::Warning,
            evidence: vec![RiskEvidence {
                kind: "exit_code".to_string(),
                value: exit_code.unwrap_or(-1).to_string(),
                path: None,
                event_id: None,
            }],
            recommendation: Some(
                "Review partial file, process, network, and Docker effects before retrying."
                    .to_string(),
            ),
        });
    } else if !matches!(run_status, RunStatus::Running) && exit_code.is_some_and(|code| code != 0) {
        risks.push(RiskNote {
            id: "command-failed".to_string(),
            title: "Command exited nonzero".to_string(),
            detail: "The command returned a nonzero exit code. It may have applied some side effects before failing.".to_string(),
            severity: Severity::Warning,
            evidence: vec![RiskEvidence {
                kind: "exit_code".to_string(),
                value: exit_code.unwrap_or(-1).to_string(),
                path: None,
                event_id: None,
            }],
            recommendation: Some(
                "Inspect stdout, stderr, and the receipt before rerunning the command."
                    .to_string(),
            ),
        });
    }

    if network.iter().any(|event| {
        matches!(event.direction, NetworkDirection::Listening)
            && matches!(event.ip.as_str(), "0.0.0.0" | "::")
    }) {
        risks.push(RiskNote {
            id: "public-port-observed".to_string(),
            title: "Observed listening port on a public interface".to_string(),
            detail: "A process in the observed command tree listened on 0.0.0.0 or :: during the receipt window."
                .to_string(),
            severity: Severity::Warning,
            evidence: network
                .iter()
                .filter(|event| {
                    matches!(event.direction, NetworkDirection::Listening)
                        && matches!(event.ip.as_str(), "0.0.0.0" | "::")
                })
                .map(|event| RiskEvidence {
                    kind: "network_port".to_string(),
                    value: format!("{}:{}/{}", event.ip, event.port, event.protocol),
                    path: None,
                    event_id: None,
                })
                .collect(),
            recommendation: Some(
                "Confirm whether this listening socket should be reachable outside localhost."
                    .to_string(),
            ),
        });
    }

    if let Some(docker) = docker {
        if !docker.ports_published.is_empty() {
            let evidence: Vec<RiskEvidence> = docker
                .ports_published
                .iter()
                .filter(|port| matches!(port.host_ip.as_str(), "0.0.0.0" | "::" | ""))
                .map(|port| RiskEvidence {
                    kind: "docker_port".to_string(),
                    value: format!(
                        "{}:{}->{}/{}",
                        if port.host_ip.is_empty() {
                            "0.0.0.0"
                        } else {
                            port.host_ip.as_str()
                        },
                        port.host_port,
                        port.container_port,
                        port.protocol
                    ),
                    path: None,
                    event_id: None,
                })
                .collect();
            if !evidence.is_empty() {
                risks.push(RiskNote {
                    id: "docker-public-port".to_string(),
                    title: "Docker published a public port".to_string(),
                    detail:
                        "A Docker change exposed a container port on a host interface beyond localhost."
                            .to_string(),
                    severity: Severity::Warning,
                    evidence,
                    recommendation: Some(
                        "Confirm the published port should be reachable outside localhost."
                            .to_string(),
                    ),
                });
            }
        }
    }

    risks
}

pub fn unique_hosts(events: &[NetworkEvent]) -> usize {
    let mut hosts = BTreeSet::new();
    for event in events {
        if !matches!(event.direction, NetworkDirection::Outbound) {
            continue;
        }
        hosts.insert(
            event
                .host
                .as_deref()
                .unwrap_or(event.ip.as_str())
                .to_string(),
        );
    }
    hosts.len()
}

pub(crate) fn risk_tags(path: &str, executable: bool) -> Vec<String> {
    let mut tags = Vec::new();
    let lower = path.to_ascii_lowercase();

    if lower.ends_with(".zshrc")
        || lower.ends_with(".bashrc")
        || lower.ends_with(".profile")
        || lower.contains("config.fish")
    {
        tags.push("shell-startup".to_string());
    }
    if lower.contains(".config") || lower.ends_with(".toml") || lower.ends_with(".service") {
        tags.push("config".to_string());
    }
    if lower.contains(".ssh") || lower.contains(".env") || lower.contains("credentials") {
        tags.push("sensitive".to_string());
    }
    if executable || lower.contains("/bin/") || lower.starts_with("bin/") {
        tags.push("executable".to_string());
    }

    tags
}

fn risks_from_files(files: &[FileChange]) -> Vec<RiskNote> {
    let mut risks = Vec::new();
    for file in files {
        if file.risk_tags.iter().any(|tag| tag == "shell-startup") {
            risks.push(RiskNote {
                id: format!("shell-startup-{}", slugify(&file.path)),
                title: "Shell startup file changed".to_string(),
                detail: "This file can run code whenever a new interactive shell session starts."
                    .to_string(),
                severity: Severity::Warning,
                evidence: vec![RiskEvidence {
                    kind: "file_change".to_string(),
                    value: file.path.clone(),
                    path: Some(file.path.clone()),
                    event_id: None,
                }],
                recommendation: Some(
                    "Review the diff carefully before keeping this startup-file change."
                        .to_string(),
                ),
            });
        }

        if file.risk_tags.iter().any(|tag| tag == "executable")
            && matches!(file.change_type, FileChangeType::Created)
        {
            risks.push(RiskNote {
                id: format!("executable-created-{}", slugify(&file.path)),
                title: "Created executable-like file".to_string(),
                detail: "A new executable or script-like file appeared during the receipt window."
                    .to_string(),
                severity: Severity::Warning,
                evidence: vec![RiskEvidence {
                    kind: "file_change".to_string(),
                    value: file.path.clone(),
                    path: Some(file.path.clone()),
                    event_id: None,
                }],
                recommendation: Some(
                    "Confirm this binary or script should be trusted before keeping it."
                        .to_string(),
                ),
            });
        }
    }
    risks
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{derive_risks, unique_hosts};
    use crate::{
        DockerPortChange, DockerSummary, FileChange, FileChangeType, NetworkDirection,
        NetworkEvent, RunStatus,
    };

    #[test]
    fn unique_hosts_counts_only_outbound_hosts() {
        let now = Utc::now();
        let events = vec![
            NetworkEvent {
                host: Some("registry.npmjs.org".to_string()),
                ip: "104.16.0.0".to_string(),
                port: 443,
                protocol: "tcp".to_string(),
                pid: Some(1),
                process_name: Some("npm".to_string()),
                first_seen: now,
                last_seen: now,
                count: 2,
                direction: NetworkDirection::Outbound,
            },
            NetworkEvent {
                host: Some("registry.npmjs.org".to_string()),
                ip: "104.16.0.1".to_string(),
                port: 443,
                protocol: "tcp".to_string(),
                pid: Some(1),
                process_name: Some("npm".to_string()),
                first_seen: now,
                last_seen: now,
                count: 1,
                direction: NetworkDirection::Outbound,
            },
            NetworkEvent {
                host: None,
                ip: "0.0.0.0".to_string(),
                port: 3000,
                protocol: "tcp".to_string(),
                pid: Some(2),
                process_name: Some("node".to_string()),
                first_seen: now,
                last_seen: now,
                count: 1,
                direction: NetworkDirection::Listening,
            },
        ];

        assert_eq!(unique_hosts(&events), 1);
    }

    #[test]
    fn derive_risks_flags_public_ports_and_failed_commands() {
        let now = Utc::now();
        let risks = derive_risks(
            &[FileChange {
                path: "bin/install.sh".to_string(),
                change_type: FileChangeType::Created,
                before_hash: None,
                after_hash: Some("after".to_string()),
                before_size: None,
                after_size: Some(12),
                is_text: true,
                diff: None,
                risk_tags: vec!["executable".to_string()],
                before_artifact_path: None,
                after_artifact_path: None,
                before_executable: None,
                after_executable: Some(true),
            }],
            &[NetworkEvent {
                host: None,
                ip: "0.0.0.0".to_string(),
                port: 8080,
                protocol: "tcp".to_string(),
                pid: Some(22),
                process_name: Some("docker-proxy".to_string()),
                first_seen: now,
                last_seen: now,
                count: 1,
                direction: NetworkDirection::Listening,
            }],
            Some(&DockerSummary {
                containers_created: Vec::new(),
                containers_removed: Vec::new(),
                containers_changed: Vec::new(),
                images_pulled: Vec::new(),
                volumes_created: Vec::new(),
                networks_created: Vec::new(),
                ports_published: vec![DockerPortChange {
                    host_ip: "0.0.0.0".to_string(),
                    host_port: 8080,
                    container_port: 8080,
                    protocol: "tcp".to_string(),
                }],
            }),
            &RunStatus::Completed,
            Some(1),
        );

        assert!(risks.iter().any(|risk| risk.id == "command-failed"));
        assert!(risks.iter().any(|risk| risk.id == "public-port-observed"));
        assert!(risks.iter().any(|risk| risk.id == "docker-public-port"));
        assert!(risks
            .iter()
            .any(|risk| risk.id.starts_with("executable-created-")));
    }
}
