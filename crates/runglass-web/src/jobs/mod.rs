use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

use chrono::{DateTime, Utc};
use runglass_core::{ObservationMode, RunProgress};
use serde_json::json;

mod revert_api;
mod run_jobs;
mod sse;

pub(crate) use revert_api::{
    read_revert_request, read_run_request, revert_apply_response, revert_preview_response,
};
pub(crate) use run_jobs::{cancel_run_job, job_exists, run_job_response, start_run_job};
pub(crate) use sse::run_job_event_stream_response;

#[derive(Debug, Clone)]
pub(crate) struct RunJob {
    pub(crate) id: String,
    pub(crate) command: String,
    pub(crate) mode: ObservationMode,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) finished_at: Option<DateTime<Utc>>,
    pub(crate) progress: Option<RunProgress>,
    pub(crate) cancel_requested: bool,
    pub(crate) cancel_flag: Arc<AtomicBool>,
    pub(crate) state: RunJobState,
}

#[derive(Debug, Clone)]
pub(crate) enum RunJobState {
    Running,
    Completed { run_id: String },
    Cancelled { run_id: String },
    Failed { error: String },
}

pub(crate) type JobStore = Arc<Mutex<HashMap<String, RunJob>>>;

#[derive(Debug, Clone)]
pub(crate) struct RevertRequest {
    pub(crate) run_id: String,
    pub(crate) files: Vec<String>,
    pub(crate) policy: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RunRequest {
    pub(crate) command: String,
    pub(crate) mode: ObservationMode,
}

pub(crate) fn run_job_payload(job: &RunJob) -> serde_json::Value {
    let base = json!({
        "id": job.id,
        "command": job.command,
        "mode": job.mode,
        "started_at": job.started_at,
        "finished_at": job.finished_at,
    });
    let mut base = base;
    base["cancel_requested"] = json!(job.cancel_requested);
    if let Some(progress) = &job.progress {
        base["elapsed_ms"] = json!(progress.elapsed_ms);
        base["stdout_preview"] = json!(progress.stdout_preview);
        base["stderr_preview"] = json!(progress.stderr_preview);
        base["summary"] = json!(progress.summary);
        base["processes_seen"] = json!(progress.processes_seen);
        base["files"] = json!(progress.files);
        base["network_hosts"] = json!(progress.network_hosts);
        base["ports_opened"] = json!(progress.ports_opened);
        base["processes"] = json!(progress.processes);
        base["network"] = json!(progress.network);
        base["docker"] = json!(progress.docker);
        base["risks"] = json!(progress.risks);
        base["events"] = json!(progress.events);
    }

    match &job.state {
        RunJobState::Running => {
            let mut payload = base;
            payload["status"] = json!(if job.cancel_requested {
                "cancelling"
            } else {
                "running"
            });
            if payload.get("elapsed_ms").is_none() {
                payload["elapsed_ms"] =
                    json!((Utc::now() - job.started_at).num_milliseconds().max(0));
            }
            payload
        }
        RunJobState::Completed { run_id } => {
            let mut payload = base;
            payload["status"] = json!("completed");
            payload["run_id"] = json!(run_id);
            payload
        }
        RunJobState::Cancelled { run_id } => {
            let mut payload = base;
            payload["status"] = json!("cancelled");
            payload["run_id"] = json!(run_id);
            payload
        }
        RunJobState::Failed { error } => {
            let mut payload = base;
            payload["status"] = json!("failed");
            payload["error"] = json!(error);
            payload
        }
    }
}
