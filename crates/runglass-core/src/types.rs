use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub schema_version: String,
    pub run: RunMeta,
    pub summary: Summary,
    pub events: Vec<TimelineEvent>,
    pub processes: Vec<ProcessInfo>,
    pub files: Vec<FileChange>,
    pub network: Vec<NetworkEvent>,
    pub docker: Option<DockerSummary>,
    pub risks: Vec<RiskNote>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeta {
    pub id: String,
    pub command_display: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub shell: Option<String>,
    #[serde(default)]
    pub mode: ObservationMode,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub status: RunStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ObservationMode {
    #[default]
    Normal,
    Deep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Interrupted,
    FailedToStart,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub files_changed: usize,
    pub files_created: usize,
    pub files_modified: usize,
    pub files_deleted: usize,
    pub processes_seen: usize,
    pub network_hosts: usize,
    pub ports_opened: usize,
    pub docker_containers_created: usize,
    pub docker_images_pulled: usize,
    pub docker_volumes_created: usize,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub at: DateTime<Utc>,
    pub kind: String,
    pub title: String,
    pub detail: Option<String>,
    pub severity: Severity,
    pub related_path: Option<String>,
    pub related_pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub command: String,
    pub argv: Vec<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub exited_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub observed_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: FileChangeType,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub before_size: Option<u64>,
    pub after_size: Option<u64>,
    pub is_text: bool,
    pub diff: Option<TextDiff>,
    pub risk_tags: Vec<String>,
    pub before_artifact_path: Option<String>,
    pub after_artifact_path: Option<String>,
    pub before_executable: Option<bool>,
    pub after_executable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDiff {
    pub format: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    pub host: Option<String>,
    pub ip: String,
    pub port: u16,
    pub protocol: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub count: usize,
    pub direction: NetworkDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDirection {
    Outbound,
    Listening,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerSummary {
    pub containers_created: Vec<DockerContainerChange>,
    pub containers_removed: Vec<DockerContainerChange>,
    pub containers_changed: Vec<DockerContainerChange>,
    pub images_pulled: Vec<DockerImageChange>,
    pub volumes_created: Vec<DockerVolumeChange>,
    pub networks_created: Vec<DockerNetworkChange>,
    pub ports_published: Vec<DockerPortChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerContainerChange {
    pub name: String,
    pub image: String,
    pub state: String,
    pub ports: Vec<String>,
    pub mounts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerImageChange {
    pub tag: String,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerVolumeChange {
    pub name: String,
    pub mountpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerNetworkChange {
    pub name: String,
    pub driver: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockerPortChange {
    pub host_ip: String,
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskNote {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub severity: Severity,
    pub evidence: Vec<RiskEvidence>,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskEvidence {
    pub kind: String,
    pub value: String,
    pub path: Option<String>,
    pub event_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunPaths {
    pub run_dir: PathBuf,
    pub report_path: PathBuf,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SnapshotEntry {
    pub path: String,
    pub hash: String,
    pub size: u64,
    pub bytes: Vec<u8>,
    pub text: Option<String>,
    pub is_text: bool,
    pub executable: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotDirectoryStats {
    pub skipped_large_files: Vec<SkippedSnapshotFile>,
}

#[derive(Debug, Clone)]
pub struct SkippedSnapshotFile {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevertConflictPolicy {
    Abort,
    SkipChanged,
    Force,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertOptions {
    pub policy: RevertConflictPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertPreview {
    pub receipt_id: String,
    pub target_count: usize,
    pub restore_modified: usize,
    pub delete_created: usize,
    pub restore_deleted: usize,
    pub safe: Vec<RevertFileStatus>,
    pub conflicts: Vec<RevertFileStatus>,
    pub already_reverted: Vec<RevertFileStatus>,
    pub missing_artifacts: Vec<RevertFileStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertFileStatus {
    pub path: String,
    pub change_type: FileChangeType,
    pub status: RevertFileState,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevertFileState {
    Safe,
    ChangedSinceReceipt,
    AlreadyReverted,
    MissingArtifacts,
}

#[derive(Debug, Clone)]
pub struct ProcessSample {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub command: String,
    pub argv: Vec<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NetworkSample {
    pub ip: String,
    pub port: u16,
    pub protocol: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub direction: NetworkDirection,
}

#[derive(Debug, Clone)]
pub struct DockerSnapshot {
    pub containers: HashMap<String, DockerContainerChange>,
    pub images: HashMap<String, DockerImageChange>,
    pub volumes: HashMap<String, DockerVolumeChange>,
    pub networks: HashMap<String, DockerNetworkChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunProgress {
    pub command_display: String,
    pub mode: ObservationMode,
    pub started_at: DateTime<Utc>,
    pub elapsed_ms: u64,
    pub stdout_preview: String,
    pub stderr_preview: String,
    pub summary: Summary,
    pub processes_seen: usize,
    pub files: Vec<FileChange>,
    pub network_hosts: usize,
    pub ports_opened: usize,
    pub processes: Vec<ProcessInfo>,
    pub network: Vec<NetworkEvent>,
    pub docker: Option<DockerSummary>,
    pub risks: Vec<RiskNote>,
    pub events: Vec<TimelineEvent>,
}

#[cfg(test)]
mod tests {
    use super::{ObservationMode, RunReport};
    use crate::fixture::sample_report;

    #[test]
    fn run_report_round_trips_through_pretty_json() {
        let mut report = sample_report("json-export-roundtrip".to_string());
        report.run.mode = ObservationMode::Deep;

        let json = serde_json::to_string_pretty(&report).expect("serialize receipt");
        assert!(json.contains("\"command_display\": \"docker compose up -d\""));
        assert!(json.contains("\"mode\": \"deep\""));

        let restored: RunReport = serde_json::from_str(&json).expect("deserialize receipt");
        assert_eq!(restored.run.id, report.run.id);
        assert_eq!(restored.run.command_display, report.run.command_display);
        assert_eq!(restored.summary.files_changed, report.summary.files_changed);
    }
}
