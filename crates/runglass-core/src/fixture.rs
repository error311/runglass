use chrono::{DateTime, Duration, TimeZone, Utc};

use crate::{
    DockerContainerChange, DockerImageChange, DockerNetworkChange, DockerPortChange, DockerSummary,
    DockerVolumeChange, FileChange, FileChangeType, NetworkDirection, NetworkEvent,
    ObservationMode, ProcessInfo, RiskEvidence, RiskLevel, RiskNote, RunMeta, RunReport, RunStatus,
    Severity, Summary, TextDiff, TimelineEvent,
};

pub fn sample_report(run_id: String) -> RunReport {
    sample_report_at(run_id, dt(2026, 5, 3, 20, 22, 31))
}

pub fn sample_report_at(run_id: String, started_at: DateTime<Utc>) -> RunReport {
    let ended_at = started_at + Duration::milliseconds(12_480);

    let files = vec![
        modified(
            "compose.yaml",
            r#"@@ -1,6 +1,12 @@
 services:
-  web:
-    image: nginx:alpine
+  app:
+    image: ghcr.io/acme/demo-app:1.4.2
+    ports:
+      - "127.0.0.1:8080:8080"
+    depends_on:
+      - db
+  db:
+    image: postgres:16-alpine
+    volumes:
+      - demo_pgdata:/var/lib/postgresql/data
"#,
            vec!["config".to_string()],
        ),
        created(".env"),
        created("compose.override.yaml"),
        created(".docker/compose-state.json"),
    ];

    let network = vec![
        net("registry-1.docker.io", "34.194.164.123", 443, 4, "docker"),
        net("auth.docker.io", "34.225.208.41", 443, 2, "docker"),
        net(
            "production.cloudflare.docker.com",
            "104.18.124.25",
            443,
            3,
            "docker",
        ),
        net("ghcr.io", "140.82.121.33", 443, 1, "docker"),
        listening("127.0.0.1", 8080, "docker-proxy"),
        listening("127.0.0.1", 5432, "docker-proxy"),
    ];

    let processes = vec![
        proc(
            81231,
            None,
            "docker",
            vec!["docker", "compose", "up", "-d"],
            started_at,
            ended_at,
        ),
        proc(
            81232,
            Some(81231),
            "docker-compose",
            vec!["docker-compose", "up", "-d"],
            started_at + Duration::seconds(1),
            started_at + Duration::seconds(11),
        ),
        proc(
            81233,
            Some(81231),
            "docker-proxy",
            vec![
                "docker-proxy",
                "-host-ip",
                "127.0.0.1",
                "-host-port",
                "8080",
            ],
            started_at + Duration::seconds(6),
            ended_at,
        ),
        proc(
            81234,
            Some(81231),
            "docker-proxy",
            vec![
                "docker-proxy",
                "-host-ip",
                "127.0.0.1",
                "-host-port",
                "5432",
            ],
            started_at + Duration::seconds(7),
            ended_at,
        ),
        proc(
            81235,
            Some(81231),
            "containerd-shim",
            vec!["containerd-shim", "demo-app-1"],
            started_at + Duration::seconds(5),
            ended_at,
        ),
    ];

    let events = vec![
        event(
            started_at,
            "command_started",
            "Process started",
            Some("docker compose up -d"),
            Severity::Info,
            None,
            Some(81231),
        ),
        event(
            started_at + Duration::seconds(1),
            "process_spawned",
            "docker-compose",
            Some("Compose planner started"),
            Severity::Info,
            None,
            Some(81232),
        ),
        event(
            started_at + Duration::seconds(2),
            "network_contact",
            "Contacted Docker registry",
            Some("registry-1.docker.io"),
            Severity::Success,
            None,
            Some(81232),
        ),
        event(
            started_at + Duration::seconds(4),
            "docker_image_pulled",
            "Pulled image",
            Some("ghcr.io/acme/demo-app:1.4.2"),
            Severity::Info,
            None,
            Some(81232),
        ),
        event(
            started_at + Duration::seconds(6),
            "docker_container_created",
            "Created container",
            Some("demo-app-1"),
            Severity::Success,
            None,
            Some(81235),
        ),
        event(
            started_at + Duration::seconds(7),
            "docker_port_published",
            "Published port",
            Some("127.0.0.1:8080 -> 8080/tcp"),
            Severity::Warning,
            None,
            Some(81233),
        ),
        event(
            started_at + Duration::seconds(9),
            "file_modified",
            "Modified",
            Some("compose.yaml"),
            Severity::Info,
            Some("compose.yaml"),
            Some(81232),
        ),
        event(
            ended_at,
            "command_exited",
            "Process exited",
            Some("Exit code: 0"),
            Severity::Success,
            None,
            Some(81231),
        ),
    ];

    let risks = vec![
        RiskNote {
            id: "published-local-port".to_string(),
            title: "Published local service ports".to_string(),
            detail: "The compose stack exposed local service ports for the app and database."
                .to_string(),
            severity: Severity::Warning,
            evidence: vec![
                RiskEvidence {
                    kind: "docker_port".to_string(),
                    value: "127.0.0.1:8080->8080/tcp".to_string(),
                    path: None,
                    event_id: None,
                },
                RiskEvidence {
                    kind: "docker_port".to_string(),
                    value: "127.0.0.1:5432->5432/tcp".to_string(),
                    path: None,
                    event_id: None,
                },
            ],
            recommendation: Some(
                "Confirm these ports should be reachable from the host before sharing the stack."
                    .to_string(),
            ),
        },
        RiskNote {
            id: "compose-config-changed".to_string(),
            title: "Modified Compose configuration".to_string(),
            detail: "The receipt shows working-directory changes to Compose inputs.".to_string(),
            severity: Severity::Warning,
            evidence: vec![RiskEvidence {
                kind: "file_change".to_string(),
                value: "compose.yaml".to_string(),
                path: Some("compose.yaml".to_string()),
                event_id: None,
            }],
            recommendation: Some(
                "Review environment values and published ports before committing the Compose files."
                    .to_string(),
            ),
        },
        RiskNote {
            id: "docker-images-pulled".to_string(),
            title: "Pulled container images".to_string(),
            detail: "The stack pulled new images from remote registries during startup."
                .to_string(),
            severity: Severity::Info,
            evidence: vec![
                RiskEvidence {
                    kind: "docker_image".to_string(),
                    value: "ghcr.io/acme/demo-app:1.4.2".to_string(),
                    path: None,
                    event_id: None,
                },
                RiskEvidence {
                    kind: "network_host".to_string(),
                    value: "registry-1.docker.io".to_string(),
                    path: None,
                    event_id: None,
                },
                RiskEvidence {
                    kind: "network_host".to_string(),
                    value: "ghcr.io".to_string(),
                    path: None,
                    event_id: None,
                },
            ],
            recommendation: Some(
                "Confirm image tags and digests match what you expect before promoting the stack."
                    .to_string(),
            ),
        },
        RiskNote {
            id: "no-secrets-touched".to_string(),
            title: "No known secret files touched".to_string(),
            detail: "The watched working-directory diff did not include common secret file paths."
                .to_string(),
            severity: Severity::Success,
            evidence: Vec::new(),
            recommendation: None,
        },
    ];

    RunReport {
        schema_version: "0.1.0".to_string(),
        run: RunMeta {
            id: run_id,
            command_display: "docker compose up -d".to_string(),
            argv: vec![
                "docker".to_string(),
                "compose".to_string(),
                "up".to_string(),
                "-d".to_string(),
            ],
            cwd: "~/projects/demo-stack".to_string(),
            shell: Some("/bin/zsh".to_string()),
            mode: ObservationMode::Normal,
            started_at,
            ended_at: Some(ended_at),
            duration_ms: Some(12_480),
            exit_code: Some(0),
            status: RunStatus::Completed,
        },
        summary: Summary {
            files_changed: 4,
            files_created: 3,
            files_modified: 1,
            files_deleted: 0,
            processes_seen: 5,
            network_hosts: 4,
            ports_opened: 2,
            docker_containers_created: 2,
            docker_images_pulled: 2,
            docker_volumes_created: 1,
            risk_level: RiskLevel::Medium,
        },
        events,
        processes,
        files,
        network,
        docker: Some(DockerSummary {
            containers_created: vec![
                DockerContainerChange {
                    name: "demo-app-1".to_string(),
                    image: "ghcr.io/acme/demo-app:1.4.2".to_string(),
                    state: "running".to_string(),
                    ports: vec!["127.0.0.1:8080->8080/tcp".to_string()],
                    mounts: vec!["./compose.yaml:/app/compose.yaml:ro".to_string()],
                },
                DockerContainerChange {
                    name: "demo-db-1".to_string(),
                    image: "postgres:16-alpine".to_string(),
                    state: "running".to_string(),
                    ports: vec!["127.0.0.1:5432->5432/tcp".to_string()],
                    mounts: vec!["demo_pgdata:/var/lib/postgresql/data".to_string()],
                },
            ],
            containers_removed: Vec::new(),
            containers_changed: Vec::new(),
            images_pulled: vec![
                DockerImageChange {
                    tag: "ghcr.io/acme/demo-app:1.4.2".to_string(),
                    digest: Some("sha256:app123".to_string()),
                },
                DockerImageChange {
                    tag: "postgres:16-alpine".to_string(),
                    digest: Some("sha256:pg123".to_string()),
                },
            ],
            volumes_created: vec![DockerVolumeChange {
                name: "demo_pgdata".to_string(),
                mountpoint: Some("/var/lib/docker/volumes/demo_pgdata".to_string()),
            }],
            networks_created: vec![DockerNetworkChange {
                name: "demo-stack_default".to_string(),
                driver: "bridge".to_string(),
            }],
            ports_published: vec![
                DockerPortChange {
                    host_ip: "127.0.0.1".to_string(),
                    host_port: 8080,
                    container_port: 8080,
                    protocol: "tcp".to_string(),
                },
                DockerPortChange {
                    host_ip: "127.0.0.1".to_string(),
                    host_port: 5432,
                    container_port: 5432,
                    protocol: "tcp".to_string(),
                },
            ],
        }),
        risks,
        stdout_path: Some("stdout.log".to_string()),
        stderr_path: Some("stderr.log".to_string()),
        stdout: Some(
            r#"[+] Running 5/5
 ✔ Network demo-stack_default      Created
 ✔ Volume demo_pgdata             Created
 ✔ Container demo-db-1            Started
 ✔ Container demo-app-1           Started
"#
            .to_string(),
        ),
        stderr: Some(String::new()),
        limitations: vec![
            "This is a built-in demo receipt shaped to show the product story.".to_string(),
            "Processes and network activity in the demo are illustrative fixture data.".to_string(),
        ],
    }
}

fn event(
    at: chrono::DateTime<Utc>,
    kind: &str,
    title: &str,
    detail: Option<&str>,
    severity: Severity,
    related_path: Option<&str>,
    related_pid: Option<u32>,
) -> TimelineEvent {
    TimelineEvent {
        at,
        kind: kind.to_string(),
        title: title.to_string(),
        detail: detail.map(ToString::to_string),
        severity,
        related_path: related_path.map(ToString::to_string),
        related_pid,
    }
}

fn proc(
    pid: u32,
    ppid: Option<u32>,
    command: &str,
    argv: Vec<&str>,
    started_at: chrono::DateTime<Utc>,
    ended_at: chrono::DateTime<Utc>,
) -> ProcessInfo {
    ProcessInfo {
        pid,
        ppid,
        command: command.to_string(),
        argv: argv.into_iter().map(ToString::to_string).collect(),
        started_at: Some(started_at),
        exited_at: Some(ended_at),
        exit_code: Some(0),
        observed_by: "proc_polling".to_string(),
    }
}

fn created(path: &str) -> FileChange {
    FileChange {
        path: path.to_string(),
        change_type: FileChangeType::Created,
        before_hash: None,
        after_hash: Some("sha256-demo".to_string()),
        before_size: None,
        after_size: Some(128),
        is_text: true,
        diff: None,
        risk_tags: vec!["config".to_string()],
        before_artifact_path: None,
        after_artifact_path: None,
        before_executable: None,
        after_executable: Some(false),
    }
}

fn modified(path: &str, diff: &str, risk_tags: Vec<String>) -> FileChange {
    FileChange {
        path: path.to_string(),
        change_type: FileChangeType::Modified,
        before_hash: Some("sha256-before".to_string()),
        after_hash: Some("sha256-after".to_string()),
        before_size: Some(926),
        after_size: Some(981),
        is_text: true,
        diff: Some(TextDiff {
            format: "unified".to_string(),
            content: diff.to_string(),
        }),
        risk_tags,
        before_artifact_path: None,
        after_artifact_path: None,
        before_executable: Some(false),
        after_executable: Some(false),
    }
}

fn net(host: &str, ip: &str, port: u16, count: usize, process_name: &str) -> NetworkEvent {
    NetworkEvent {
        host: Some(host.to_string()),
        ip: ip.to_string(),
        port,
        protocol: "tcp".to_string(),
        pid: Some(81232),
        process_name: Some(process_name.to_string()),
        first_seen: dt(2026, 5, 3, 20, 22, 33),
        last_seen: dt(2026, 5, 3, 20, 22, 39),
        count,
        direction: NetworkDirection::Outbound,
    }
}

fn listening(ip: &str, port: u16, process_name: &str) -> NetworkEvent {
    NetworkEvent {
        host: None,
        ip: ip.to_string(),
        port,
        protocol: "tcp".to_string(),
        pid: Some(if port == 8080 { 81233 } else { 81234 }),
        process_name: Some(process_name.to_string()),
        first_seen: dt(2026, 5, 3, 20, 22, 37),
        last_seen: dt(2026, 5, 3, 20, 22, 43),
        count: 1,
        direction: NetworkDirection::Listening,
    }
}

fn dt(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .expect("valid fixture time")
}
