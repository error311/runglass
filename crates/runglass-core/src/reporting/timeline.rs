use chrono::{DateTime, Utc};

use crate::{
    DockerSummary, FileChange, FileChangeType, NetworkDirection, NetworkEvent, ProcessInfo,
    Severity, TimelineEvent,
};

pub(crate) fn file_events(files: &[FileChange], at: DateTime<Utc>) -> Vec<TimelineEvent> {
    files
        .iter()
        .map(|file| {
            let (kind, title, severity) = match file.change_type {
                FileChangeType::Created => (
                    "file_created",
                    "Working-directory file created",
                    Severity::Success,
                ),
                FileChangeType::Modified => (
                    "file_modified",
                    "Working-directory file modified",
                    Severity::Warning,
                ),
                FileChangeType::Deleted => (
                    "file_deleted",
                    "Working-directory file deleted",
                    Severity::Warning,
                ),
            };

            TimelineEvent {
                at,
                kind: kind.to_string(),
                title: title.to_string(),
                detail: Some(file.path.clone()),
                severity,
                related_path: Some(file.path.clone()),
                related_pid: None,
            }
        })
        .collect()
}

pub(crate) fn process_events(
    processes: &[ProcessInfo],
    root_pid: Option<u32>,
) -> Vec<TimelineEvent> {
    let mut events = Vec::new();
    for process in processes {
        if Some(process.pid) == root_pid {
            continue;
        }

        if let Some(at) = process.started_at {
            events.push(TimelineEvent {
                at,
                kind: "process_spawned".to_string(),
                title: format!("Observed process: {}", process.command),
                detail: Some(process.argv.join(" ")),
                severity: Severity::Info,
                related_path: None,
                related_pid: Some(process.pid),
            });
        }

        if let Some(at) = process.exited_at {
            events.push(TimelineEvent {
                at,
                kind: "process_exited".to_string(),
                title: format!("{} exited", process.command),
                detail: Some(format!("PID {}", process.pid)),
                severity: Severity::Success,
                related_path: None,
                related_pid: Some(process.pid),
            });
        }
    }

    events
}

pub(crate) fn network_events(network: &[NetworkEvent]) -> Vec<TimelineEvent> {
    network
        .iter()
        .map(|event| {
            let (kind, title, severity, detail) = match event.direction {
                NetworkDirection::Listening => (
                    "port_listening",
                    "Listening port observed".to_string(),
                    Severity::Warning,
                    format!(
                        "{}:{} ({})",
                        event.ip,
                        event.port,
                        event.process_name.as_deref().unwrap_or("unknown")
                    ),
                ),
                NetworkDirection::Outbound => (
                    "network_connection",
                    "Outbound connection observed".to_string(),
                    Severity::Info,
                    format!(
                        "{}:{} via {}{}",
                        event.host.as_deref().unwrap_or(event.ip.as_str()),
                        event.port,
                        event.protocol,
                        event
                            .process_name
                            .as_ref()
                            .map(|name| format!(" ({name})"))
                            .unwrap_or_default()
                    ),
                ),
                NetworkDirection::Unknown => (
                    "network_observed",
                    "Network activity observed".to_string(),
                    Severity::Info,
                    format!("{}:{} via {}", event.ip, event.port, event.protocol),
                ),
            };

            TimelineEvent {
                at: event.first_seen,
                kind: kind.to_string(),
                title,
                detail: Some(detail),
                severity,
                related_path: None,
                related_pid: event.pid,
            }
        })
        .collect()
}

pub(crate) fn docker_events(
    docker: Option<&DockerSummary>,
    at: DateTime<Utc>,
) -> Vec<TimelineEvent> {
    let Some(docker) = docker else {
        return Vec::new();
    };

    let mut events = Vec::new();
    for container in &docker.containers_created {
        events.push(TimelineEvent {
            at,
            kind: "docker_container_created".to_string(),
            title: "Docker container created".to_string(),
            detail: Some(format!("{} ({})", container.name, container.image)),
            severity: Severity::Info,
            related_path: None,
            related_pid: None,
        });
    }
    for image in &docker.images_pulled {
        events.push(TimelineEvent {
            at,
            kind: "docker_image_pulled".to_string(),
            title: "Docker image pulled".to_string(),
            detail: Some(image.tag.clone()),
            severity: Severity::Info,
            related_path: None,
            related_pid: None,
        });
    }
    for port in &docker.ports_published {
        events.push(TimelineEvent {
            at,
            kind: "docker_port_published".to_string(),
            title: "Docker published port".to_string(),
            detail: Some(format!(
                "{}:{}->{}/{}",
                if port.host_ip.is_empty() {
                    "0.0.0.0"
                } else {
                    port.host_ip.as_str()
                },
                port.host_port,
                port.container_port,
                port.protocol
            )),
            severity: Severity::Warning,
            related_path: None,
            related_pid: None,
        });
    }

    events
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{file_events, network_events, process_events};
    use crate::{FileChange, FileChangeType, NetworkDirection, NetworkEvent, ProcessInfo};

    #[test]
    fn file_and_process_timeline_titles_are_receipt_oriented() {
        let now = Utc::now();
        let file_events = file_events(
            &[FileChange {
                path: "config/app.toml".to_string(),
                change_type: FileChangeType::Modified,
                before_hash: None,
                after_hash: None,
                before_size: None,
                after_size: None,
                is_text: true,
                diff: None,
                risk_tags: vec!["config".to_string()],
                before_artifact_path: None,
                after_artifact_path: None,
                before_executable: None,
                after_executable: None,
            }],
            now,
        );
        let process_events = process_events(
            &[ProcessInfo {
                pid: 42,
                ppid: Some(1),
                command: "npm".to_string(),
                argv: vec!["npm".to_string(), "install".to_string()],
                started_at: Some(now),
                exited_at: None,
                exit_code: None,
                observed_by: "proc".to_string(),
            }],
            None,
        );

        assert_eq!(file_events[0].title, "Working-directory file modified");
        assert_eq!(process_events[0].title, "Observed process: npm");
    }

    #[test]
    fn outbound_network_timeline_uses_host_and_process_details() {
        let now = Utc::now();
        let events = network_events(&[NetworkEvent {
            host: Some("registry.npmjs.org".to_string()),
            ip: "104.16.0.0".to_string(),
            port: 443,
            protocol: "tcp".to_string(),
            pid: Some(77),
            process_name: Some("npm".to_string()),
            first_seen: now,
            last_seen: now,
            count: 1,
            direction: NetworkDirection::Outbound,
        }]);

        assert_eq!(events[0].title, "Outbound connection observed");
        assert!(events[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("registry.npmjs.org:443 via tcp (npm)")));
    }
}
