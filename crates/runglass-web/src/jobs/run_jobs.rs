use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

use chrono::Utc;
use runglass_core::{make_run_id, run_observed_shell_command_with_control_in_mode};
use serde_json::json;
use tiny_http::{Response, StatusCode};

use crate::http::{html_response, json_status_response};

use super::{run_job_payload, JobStore, RunJob, RunJobState, RunRequest};

pub(crate) fn start_run_job(
    jobs: &JobStore,
    request: RunRequest,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let job_id = format!("job_{}", make_run_id(&request.command));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let job = RunJob {
        id: job_id.clone(),
        command: request.command.clone(),
        mode: request.mode,
        started_at: Utc::now(),
        finished_at: None,
        progress: None,
        cancel_requested: false,
        cancel_flag: Arc::clone(&cancel_flag),
        state: RunJobState::Running,
    };

    if let Ok(mut store) = jobs.lock() {
        store.insert(job_id.clone(), job.clone());
    } else {
        return json_status_response(
            StatusCode(500),
            &json!({ "error": "failed to access run job store" }).to_string(),
        );
    }

    let jobs = Arc::clone(jobs);
    thread::spawn(move || {
        let jobs_for_progress = Arc::clone(&jobs);
        let progress_job_id = job_id.clone();
        let cancel_for_runner = Arc::clone(&cancel_flag);
        let next_state = match run_observed_shell_command_with_control_in_mode(
            request.command,
            request.mode,
            move |progress| {
                if let Ok(mut store) = jobs_for_progress.lock() {
                    if let Some(job) = store.get_mut(&progress_job_id) {
                        job.progress = Some(progress);
                    }
                }
            },
            move || cancel_for_runner.load(Ordering::Relaxed),
        ) {
            Ok((report, _paths)) => {
                if matches!(report.run.status, runglass_core::RunStatus::Interrupted) {
                    RunJobState::Cancelled {
                        run_id: report.run.id,
                    }
                } else {
                    RunJobState::Completed {
                        run_id: report.run.id,
                    }
                }
            }
            Err(error) => RunJobState::Failed {
                error: error.to_string(),
            },
        };
        let finished_at = Utc::now();

        if let Ok(mut store) = jobs.lock() {
            if let Some(job) = store.get_mut(&job_id) {
                job.finished_at = Some(finished_at);
                job.state = next_state;
            }
        }
    });

    json_status_response(StatusCode(202), &run_job_payload(&job).to_string())
}

pub(crate) fn cancel_run_job(jobs: &JobStore, job_id: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    match jobs.lock() {
        Ok(mut store) => match store.get_mut(job_id) {
            Some(job) => {
                job.cancel_requested = true;
                job.cancel_flag.store(true, Ordering::Relaxed);
                html_response(&run_job_payload(job).to_string(), "application/json")
            }
            None => Response::from_string("Not Found").with_status_code(StatusCode(404)),
        },
        Err(_) => json_status_response(
            StatusCode(500),
            &json!({ "error": "failed to access run job store" }).to_string(),
        ),
    }
}

pub(crate) fn run_job_response(
    jobs: &JobStore,
    job_id: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match jobs.lock() {
        Ok(store) => match store.get(job_id) {
            Some(job) => html_response(&run_job_payload(job).to_string(), "application/json"),
            None => Response::from_string("Not Found").with_status_code(StatusCode(404)),
        },
        Err(_) => json_status_response(
            StatusCode(500),
            &json!({ "error": "failed to access run job store" }).to_string(),
        ),
    }
}

pub(crate) fn job_exists(jobs: &JobStore, job_id: &str) -> bool {
    jobs.lock()
        .ok()
        .and_then(|store| store.get(job_id).map(|_| ()))
        .is_some()
}
