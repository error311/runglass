use anyhow::{anyhow, Result};
use runglass_core::{
    apply_revert, load_report, preview_revert, ObservationMode, RevertConflictPolicy, RevertOptions,
};
use serde_json::json;
use tiny_http::{Response, StatusCode};

use crate::http::{html_response, json_status_response};

use super::{RevertRequest, RunRequest};

pub(crate) fn read_run_request(request: &mut tiny_http::Request) -> Result<Option<RunRequest>> {
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;
    let payload: serde_json::Value = serde_json::from_str(&body)?;
    let command = payload
        .get("command")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    Ok(command.map(|command| RunRequest {
        command,
        mode: match payload.get("mode").and_then(|value| value.as_str()) {
            Some("deep") => ObservationMode::Deep,
            _ => ObservationMode::Normal,
        },
    }))
}

pub(crate) fn read_revert_request(request: &mut tiny_http::Request) -> Result<RevertRequest> {
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;
    let payload: serde_json::Value = serde_json::from_str(&body)?;
    Ok(RevertRequest {
        run_id: payload
            .get("run_id")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("missing run_id"))?,
        files: payload
            .get("files")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        policy: payload
            .get("policy")
            .and_then(|value| value.as_str())
            .map(str::to_owned),
    })
}

pub(crate) fn revert_preview_response(
    payload: &RevertRequest,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match load_report(&payload.run_id).and_then(|report| {
        let selected = (!payload.files.is_empty()).then_some(payload.files.as_slice());
        preview_revert(&report, selected)
    }) {
        Ok(preview) => html_response(
            &serde_json::to_string(&preview).unwrap_or_else(|_| "{}".to_string()),
            "application/json",
        ),
        Err(error) => json_status_response(
            StatusCode(400),
            &json!({ "error": error.to_string() }).to_string(),
        ),
    }
}

pub(crate) fn revert_apply_response(payload: &RevertRequest) -> Response<std::io::Cursor<Vec<u8>>> {
    let policy = match payload.policy.as_deref() {
        Some("force") => RevertConflictPolicy::Force,
        Some("skip_changed") => RevertConflictPolicy::SkipChanged,
        _ => RevertConflictPolicy::Abort,
    };
    match load_report(&payload.run_id).and_then(|report| {
        let selected = (!payload.files.is_empty()).then_some(payload.files.as_slice());
        apply_revert(&report, selected, RevertOptions { policy })
    }) {
        Ok(result) => html_response(
            &serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
            "application/json",
        ),
        Err(error) => json_status_response(
            StatusCode(400),
            &json!({ "error": error.to_string() }).to_string(),
        ),
    }
}
