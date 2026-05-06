use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, Result};
use runglass_core::{
    delete_report, list_reports, load_report, render_markdown_receipt, render_reverse_patch,
    RunReport,
};
use serde_json::json;
use tiny_http::{Method, Response, Server, StatusCode};

use crate::http::{binary_response, html_response, json_status_response};
use crate::jobs::{
    cancel_run_job, job_exists, read_revert_request, read_run_request, revert_apply_response,
    revert_preview_response, run_job_event_stream_response, run_job_response, start_run_job,
    JobStore,
};
use crate::ui::render_html;

const RUNGLASS_ICON_PNG: &[u8] = include_bytes!("../assets/branding/runglass_icon.png");
const RUNGLASS_ICON_SVG: &[u8] = include_bytes!("../assets/branding/runglass_icon.svg");
const RUNGLASS_FAVICON_32: &[u8] = include_bytes!("../assets/branding/runglass_favicon_32.png");
const RUNGLASS_APPLE_TOUCH: &[u8] = include_bytes!("../assets/branding/runglass_apple_touch.png");
const RUNGLASS_LOGO_PNG: &[u8] = include_bytes!("../assets/branding/runglass_wordmark.png");
const RUNGLASS_LOGO_SVG: &[u8] = include_bytes!("../assets/branding/runglass_wordmark.svg");

pub fn serve_report(report: RunReport, open_browser: bool) -> Result<()> {
    let html = render_html(&report)?;
    let json = serde_json::to_string_pretty(&report)?;
    let stdout = report.stdout.clone().unwrap_or_default();
    let stderr = report.stderr.clone().unwrap_or_default();
    let jobs: JobStore = Arc::new(Mutex::new(HashMap::new()));

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let server = Server::from_listener(listener, None).map_err(|err| anyhow!(err.to_string()))?;
    let url = format!("http://{address}");

    if open_browser {
        let _ = webbrowser::open(&url);
    }

    println!("Serving RunGlass receipt at {url}");

    for mut request in server.incoming_requests() {
        let path = request
            .url()
            .split('?')
            .next()
            .unwrap_or(request.url())
            .to_string();
        if request.method() == &Method::Get
            && path.starts_with("/api/jobs/")
            && path.ends_with("/events")
        {
            let job_id = path
                .strip_prefix("/api/jobs/")
                .and_then(|value| value.strip_suffix("/events"));
            match job_id {
                Some(job_id) if job_exists(&jobs, job_id) => {
                    let jobs = Arc::clone(&jobs);
                    let job_id = job_id.to_string();
                    thread::spawn(move || {
                        let response = run_job_event_stream_response(&jobs, &job_id);
                        let _ = request.respond(response);
                    });
                }
                Some(_) => {
                    let response =
                        Response::from_string("Not Found").with_status_code(StatusCode(404));
                    request.respond(response)?;
                }
                None => {
                    let response =
                        Response::from_string("Not Found").with_status_code(StatusCode(404));
                    request.respond(response)?;
                }
            }
            continue;
        }

        let response = match (request.method(), path.as_str()) {
            (&Method::Post, "/api/run") => match read_run_request(&mut request) {
                Ok(Some(payload)) => start_run_job(&jobs, payload),
                Ok(None) => json_status_response(
                    StatusCode(400),
                    &json!({ "error": "missing command" }).to_string(),
                ),
                Err(error) => json_status_response(
                    StatusCode(400),
                    &json!({ "error": error.to_string() }).to_string(),
                ),
            },
            (&Method::Post, "/api/revert/preview") => match read_revert_request(&mut request) {
                Ok(payload) => revert_preview_response(&payload),
                Err(error) => json_status_response(
                    StatusCode(400),
                    &json!({ "error": error.to_string() }).to_string(),
                ),
            },
            (&Method::Post, "/api/revert/apply") => match read_revert_request(&mut request) {
                Ok(payload) => revert_apply_response(&payload),
                Err(error) => json_status_response(
                    StatusCode(400),
                    &json!({ "error": error.to_string() }).to_string(),
                ),
            },
            (&Method::Post, _) if path.starts_with("/api/jobs/") && path.contains("/cancel") => {
                let normalized = path.trim_end_matches('/');
                let job_id = normalized
                    .strip_prefix("/api/jobs/")
                    .and_then(|value| value.strip_suffix("/cancel"));
                match job_id {
                    Some(job_id) => cancel_run_job(&jobs, job_id),
                    None => Response::from_string("Not Found").with_status_code(StatusCode(404)),
                }
            }
            (&Method::Get, _) if path.starts_with("/api/jobs/") && path.ends_with(".json") => {
                let job_id = path
                    .strip_prefix("/api/jobs/")
                    .and_then(|value| value.strip_suffix(".json"));
                match job_id {
                    Some(job_id) => run_job_response(&jobs, job_id),
                    None => Response::from_string("Not Found").with_status_code(StatusCode(404)),
                }
            }
            (&Method::Delete, _) if path.starts_with("/api/reports/") => {
                let run_id = path.strip_prefix("/api/reports/");
                match run_id {
                    Some(run_id) => match delete_report(run_id) {
                        Ok(path) => json_status_response(
                            StatusCode(200),
                            &json!({ "deleted": run_id, "path": path }).to_string(),
                        ),
                        Err(error) => json_status_response(
                            StatusCode(404),
                            &json!({ "error": error.to_string() }).to_string(),
                        ),
                    },
                    None => Response::from_string("Not Found").with_status_code(StatusCode(404)),
                }
            }
            (_, "/") => html_response(&html, "text/html; charset=utf-8"),
            (_, "/report.json") => html_response(&json, "application/json"),
            (_, "/runs.json") => {
                let runs = list_reports()?;
                let payload = serde_json::to_string_pretty(&report_index_payload(&runs))?;
                html_response(&payload, "application/json")
            }
            (_, "/stdout") => html_response(&stdout, "text/plain; charset=utf-8"),
            (_, "/stderr") => html_response(&stderr, "text/plain; charset=utf-8"),
            (_, "/export/report.html") => html_response(&html, "text/html; charset=utf-8"),
            (_, "/assets/runglass_icon.png") => binary_response(RUNGLASS_ICON_PNG, "image/png"),
            (_, "/assets/runglass_icon.svg") => binary_response(RUNGLASS_ICON_SVG, "image/svg+xml"),
            (_, "/assets/runglass_favicon_32.png") => {
                binary_response(RUNGLASS_FAVICON_32, "image/png")
            }
            (_, "/assets/runglass_apple_touch.png") => {
                binary_response(RUNGLASS_APPLE_TOUCH, "image/png")
            }
            (_, "/assets/runglass_wordmark.png") => binary_response(RUNGLASS_LOGO_PNG, "image/png"),
            (_, "/assets/runglass_wordmark.svg") => {
                binary_response(RUNGLASS_LOGO_SVG, "image/svg+xml")
            }
            _ => {
                if let Some(run_id) = path
                    .strip_prefix("/reports/")
                    .and_then(|value| value.strip_suffix(".json"))
                {
                    match load_report(run_id) {
                        Ok(report) => {
                            let payload = serde_json::to_string_pretty(&report)?;
                            html_response(&payload, "application/json")
                        }
                        Err(_) => {
                            Response::from_string("Not Found").with_status_code(StatusCode(404))
                        }
                    }
                } else if let Some(run_id) = path
                    .strip_prefix("/reports/")
                    .and_then(|value| value.strip_suffix("/export.html"))
                {
                    match load_report(run_id) {
                        Ok(report) => {
                            let html = render_html(&report)?;
                            html_response(&html, "text/html; charset=utf-8")
                        }
                        Err(_) => {
                            Response::from_string("Not Found").with_status_code(StatusCode(404))
                        }
                    }
                } else if let Some(run_id) = path
                    .strip_prefix("/reports/")
                    .and_then(|value| value.strip_suffix("/receipt.md"))
                {
                    match load_report(run_id) {
                        Ok(report) => {
                            let markdown = render_markdown_receipt(&report);
                            html_response(&markdown, "text/markdown; charset=utf-8")
                        }
                        Err(_) => {
                            Response::from_string("Not Found").with_status_code(StatusCode(404))
                        }
                    }
                } else if let Some(run_id) = path
                    .strip_prefix("/reports/")
                    .and_then(|value| value.strip_suffix("/reverse.patch"))
                {
                    match load_report(run_id).and_then(|report| render_reverse_patch(&report)) {
                        Ok(patch) => html_response(&patch, "text/plain; charset=utf-8"),
                        Err(_) => {
                            Response::from_string("Not Found").with_status_code(StatusCode(404))
                        }
                    }
                } else {
                    Response::from_string("Not Found").with_status_code(StatusCode(404))
                }
            }
        };
        request.respond(response)?;
    }

    Ok(())
}

fn report_index_payload(reports: &[RunReport]) -> Vec<serde_json::Value> {
    reports
        .iter()
        .map(|report| {
            json!({
                "id": report.run.id,
                "command_display": report.run.command_display,
                "started_at": report.run.started_at,
                "status": report.run.status,
                "risk_level": report.summary.risk_level,
                "files_changed": report.summary.files_changed,
                "processes_seen": report.summary.processes_seen,
                "network_hosts": report.summary.network_hosts,
                "mode": report.run.mode,
                "is_demo": is_demo_report(report),
            })
        })
        .collect()
}

fn is_demo_report(report: &RunReport) -> bool {
    report.run.id.ends_with("_demo-receipt")
        || report
            .limitations
            .iter()
            .any(|item| item.to_ascii_lowercase().contains("fixture"))
}
