use std::env;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use runglass_core::{load_report, render_summary_markdown_receipt, RunReport};
use serde_json::json;
use tiny_http::{Response, StatusCode};

use crate::http::{html_response, json_status_response};

const COMMENT_MARKER: &str = "<!-- runglass-receipt-comment:v1 -->";
const DEFAULT_API_URL: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2026-03-10";

#[derive(Debug, Clone)]
pub(crate) struct GithubWebRequest {
    pub run_id: String,
    pub repo: Option<String>,
    pub pr: Option<u64>,
    pub confirm: bool,
    pub api_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubContext {
    repo: Option<String>,
    pr: Option<u64>,
    run_url: Option<String>,
    token_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepoParts {
    owner: String,
    repo: String,
}

pub(crate) fn read_github_request(request: &mut tiny_http::Request) -> Result<GithubWebRequest> {
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body)?;
    let payload: serde_json::Value = serde_json::from_str(&body)?;
    Ok(GithubWebRequest {
        run_id: payload
            .get("run_id")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("missing run_id"))?,
        repo: payload
            .get("repo")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        pr: payload.get("pr").and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
        }),
        confirm: payload
            .get("confirm")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        api_url: payload
            .get("api_url")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

pub(crate) fn github_preview_response(
    request: &GithubWebRequest,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match load_report(&request.run_id).and_then(|report| preview_payload(&report, request)) {
        Ok(payload) => html_response(&payload.to_string(), "application/json"),
        Err(error) => json_status_response(
            StatusCode(400),
            &json!({ "error": error.to_string() }).to_string(),
        ),
    }
}

pub(crate) fn github_comment_response(
    request: &GithubWebRequest,
) -> Response<std::io::Cursor<Vec<u8>>> {
    if !request.confirm {
        return json_status_response(
            StatusCode(400),
            &json!({ "error": "posting requires explicit confirmation" }).to_string(),
        );
    }

    let result = load_report(&request.run_id).and_then(|report| {
        let context = detect_context(&report, request);
        let repo = context
            .repo
            .as_deref()
            .ok_or_else(|| anyhow!("missing GitHub repository; enter owner/name"))?;
        let pr = context
            .pr
            .ok_or_else(|| anyhow!("missing pull request number"))?;
        let repo_parts = parse_repo(repo)?;
        let body = render_comment_body(&report, context.run_url.as_deref());
        let token = resolve_token()?;
        let client =
            GithubApiClient::new(request.api_url.as_deref().unwrap_or(DEFAULT_API_URL), token);
        let action = match client.find_marker_comment(&repo_parts, pr)? {
            Some(comment_id) => {
                client.update_comment(&repo_parts, comment_id, &body)?;
                "updated"
            }
            None => {
                client.create_comment(&repo_parts, pr, &body)?;
                "created"
            }
        };
        Ok(json!({
            "ok": true,
            "action": action,
            "repo": format!("{}/{}", repo_parts.owner, repo_parts.repo),
            "pr": pr,
            "pr_url": pr_url(Some(&format!("{}/{}", repo_parts.owner, repo_parts.repo)), Some(pr)),
        }))
    });

    match result {
        Ok(payload) => html_response(&payload.to_string(), "application/json"),
        Err(error) => json_status_response(
            StatusCode(400),
            &json!({ "error": error.to_string() }).to_string(),
        ),
    }
}

fn preview_payload(report: &RunReport, request: &GithubWebRequest) -> Result<serde_json::Value> {
    let context = detect_context(report, request);
    let body = render_comment_body(report, context.run_url.as_deref());
    let repo = context.repo.as_deref();
    let pr = context.pr;
    let can_post = repo.is_some() && pr.is_some() && context.token_source.is_some();
    Ok(json!({
        "body": body,
        "context": {
            "repo": context.repo,
            "pr": context.pr,
            "run_url": context.run_url,
            "pr_url": pr_url(repo, pr),
            "token_source": context.token_source,
            "token_available": context.token_source.is_some(),
            "can_post": can_post,
        },
        "snippets": github_snippets(report, repo, pr),
    }))
}

fn detect_context(report: &RunReport, request: &GithubWebRequest) -> GithubContext {
    let repo = request
        .repo
        .clone()
        .or_else(|| report.ci.as_ref().and_then(|ci| ci.repository.clone()))
        .or_else(|| env::var("GITHUB_REPOSITORY").ok())
        .or_else(|| git_remote_repo().ok().flatten());
    let pr = request
        .pr
        .or_else(|| report.ci.as_ref().and_then(|ci| ci.pull_request))
        .or_else(detect_github_actions_pr);
    let run_url = report
        .ci
        .as_ref()
        .and_then(|ci| ci.run_url.clone())
        .or_else(github_run_url);

    GithubContext {
        repo,
        pr,
        run_url,
        token_source: token_source(),
    }
}

fn github_snippets(report: &RunReport, repo: Option<&str>, pr: Option<u64>) -> serde_json::Value {
    let receipt = shell_quote(&report.run.id);
    let repo = repo.unwrap_or("owner/repo");
    let pr = pr
        .map(|value| value.to_string())
        .unwrap_or_else(|| "123".to_string());
    json!({
        "dry_run": format!(
            "runglass github comment --receipt {receipt} --repo {} --pr {} --dry-run",
            shell_quote(repo),
            pr
        ),
        "post": format!(
            "runglass github comment --receipt {receipt} --repo {} --pr {}",
            shell_quote(repo),
            pr
        ),
        "ci_auto": "runglass github comment --receipt runglass-receipt/receipt.json --auto",
        "workflow": GITHUB_ACTIONS_WORKFLOW.trim(),
    })
}

fn render_comment_body(report: &RunReport, run_url: Option<&str>) -> String {
    let run_url = run_url.filter(|value| !value.trim().is_empty());
    let mut body = render_summary_markdown_receipt(report)
        .trim_end()
        .to_string();
    let footer = if run_url.is_some() {
        let artifact = markdown_inline_code_text(&ci_artifact_name(report));
        format!("Full receipt: see the uploaded `{artifact}` artifact in the CI run.")
    } else {
        "Full receipt: generated locally by RunGlass.".to_string()
    };
    body = body.replace("Full receipt: see attached artifact.", &footer);
    if let Some(run_url) = run_url {
        body.push_str("\n\nCI run: ");
        body.push_str(run_url);
    }
    body.push_str("\n\n");
    body.push_str(COMMENT_MARKER);
    body.push('\n');
    body
}

fn ci_artifact_name(report: &RunReport) -> String {
    report
        .ci
        .as_ref()
        .and_then(|ci| ci.artifact_name.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("runglass-receipt")
        .to_string()
}

fn markdown_inline_code_text(value: &str) -> String {
    value.replace('`', "\\`")
}

fn parse_repo(repo: &str) -> Result<RepoParts> {
    let normalized = normalize_repo(repo.trim().trim_end_matches(".git"))
        .ok_or_else(|| anyhow!("repository must be in owner/name form"))?;
    let (owner, repo) = normalized
        .split_once('/')
        .ok_or_else(|| anyhow!("repository must be in owner/name form"))?;
    Ok(RepoParts {
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

fn normalize_repo(value: &str) -> Option<String> {
    let mut parts = value.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

fn git_remote_repo() -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .context("failed to run git remote get-url origin")?;
    if !output.status.success() {
        return Ok(None);
    }
    let remote = String::from_utf8_lossy(&output.stdout);
    Ok(parse_github_remote(remote.trim()))
}

fn parse_github_remote(remote: &str) -> Option<String> {
    let value = remote.trim().trim_end_matches(".git");
    if let Some(rest) = value.strip_prefix("git@github.com:") {
        return normalize_repo(rest);
    }
    if let Some(rest) = value.strip_prefix("https://github.com/") {
        return normalize_repo(rest);
    }
    if let Some(rest) = value.strip_prefix("http://github.com/") {
        return normalize_repo(rest);
    }
    None
}

fn detect_github_actions_pr() -> Option<u64> {
    if let Some(number) = env::var("GITHUB_EVENT_PATH")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
        .and_then(|value| {
            value
                .pointer("/pull_request/number")
                .or_else(|| value.get("number"))
                .and_then(|number| number.as_u64())
        })
    {
        return Some(number);
    }

    env::var("GITHUB_REF")
        .ok()
        .and_then(|value| parse_pull_ref(&value))
}

fn parse_pull_ref(value: &str) -> Option<u64> {
    let mut parts = value.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("refs"), Some("pull"), Some(number)) => number.parse().ok(),
        _ => None,
    }
}

fn github_run_url() -> Option<String> {
    let server = env::var("GITHUB_SERVER_URL").ok()?;
    let repo = env::var("GITHUB_REPOSITORY").ok()?;
    let run_id = env::var("GITHUB_RUN_ID").ok()?;
    Some(format!(
        "{}/{}/actions/runs/{}",
        server.trim_end_matches('/'),
        repo,
        run_id
    ))
}

fn pr_url(repo: Option<&str>, pr: Option<u64>) -> Option<String> {
    Some(format!("https://github.com/{}/pull/{}", repo?, pr?))
}

fn token_source() -> Option<String> {
    if env::var_os("GITHUB_TOKEN").is_some() {
        Some("GITHUB_TOKEN".to_string())
    } else if env::var_os("GH_TOKEN").is_some() {
        Some("GH_TOKEN".to_string())
    } else if gh_auth_token().is_ok_and(|token| !token.trim().is_empty()) {
        Some("gh auth token".to_string())
    } else {
        None
    }
}

fn resolve_token() -> Result<String> {
    if let Ok(token) = env::var("GITHUB_TOKEN") {
        if !token.trim().is_empty() {
            return Ok(token);
        }
    }
    if let Ok(token) = env::var("GH_TOKEN") {
        if !token.trim().is_empty() {
            return Ok(token);
        }
    }
    if let Ok(token) = gh_auth_token() {
        if !token.trim().is_empty() {
            return Ok(token);
        }
    }
    Err(anyhow!(
        "missing GitHub token; set GITHUB_TOKEN or GH_TOKEN, or run gh auth login"
    ))
}

fn gh_auth_token() -> Result<String> {
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .context("failed to run gh auth token")?;
    if !output.status.success() {
        return Err(anyhow!("gh auth token did not return a token"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct GithubApiClient {
    api_url: String,
    token: String,
}

impl GithubApiClient {
    fn new(api_url: &str, token: String) -> Self {
        Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            token,
        }
    }

    fn find_marker_comment(&self, repo: &RepoParts, pr: u64) -> Result<Option<u64>> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/comments?per_page=100",
            self.api_url, repo.owner, repo.repo, pr
        );
        let response = self
            .request("GET", &url)
            .call()
            .map_err(sanitize_ureq_error)?;
        let comments: serde_json::Value = response.into_json()?;
        let Some(items) = comments.as_array() else {
            return Ok(None);
        };
        Ok(items.iter().find_map(|comment| {
            let body = comment.get("body").and_then(|value| value.as_str())?;
            if body.contains(COMMENT_MARKER) {
                comment.get("id").and_then(|value| value.as_u64())
            } else {
                None
            }
        }))
    }

    fn create_comment(&self, repo: &RepoParts, pr: u64, body: &str) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/comments",
            self.api_url, repo.owner, repo.repo, pr
        );
        self.request("POST", &url)
            .send_json(json!({ "body": body }))
            .map_err(sanitize_ureq_error)?;
        Ok(())
    }

    fn update_comment(&self, repo: &RepoParts, comment_id: u64, body: &str) -> Result<()> {
        let url = format!(
            "{}/repos/{}/{}/issues/comments/{}",
            self.api_url, repo.owner, repo.repo, comment_id
        );
        self.request("PATCH", &url)
            .send_json(json!({ "body": body }))
            .map_err(sanitize_ureq_error)?;
        Ok(())
    }

    fn request(&self, method: &str, url: &str) -> ureq::Request {
        ureq::request(method, url)
            .set("Accept", "application/vnd.github+json")
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .set("User-Agent", "runglass")
    }
}

fn sanitize_ureq_error(error: ureq::Error) -> anyhow::Error {
    match error {
        ureq::Error::Status(status, response) => {
            let body = response
                .into_string()
                .unwrap_or_else(|_| "request failed".to_string());
            anyhow!("GitHub API request failed with status {status}: {body}")
        }
        ureq::Error::Transport(error) => anyhow!("GitHub API request failed: {error}"),
    }
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

const GITHUB_ACTIONS_WORKFLOW: &str = r#"
name: RunGlass PR Receipt

on:
  pull_request:
  workflow_dispatch:

permissions:
  contents: read
  issues: write
  pull-requests: write

jobs:
  receipt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install RunGlass
        run: cargo install runglass --locked
      - name: Run command with RunGlass
        run: runglass ci --provider github --output runglass-receipt -- npm test
      - name: Upload RunGlass receipt
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: runglass-receipt
          path: runglass-receipt/
      - name: Comment RunGlass receipt on PR
        if: always() && github.event_name == 'pull_request' && !github.event.pull_request.head.repo.fork && hashFiles('runglass-receipt/receipt.json') != ''
        run: |
          if ! runglass github comment --receipt runglass-receipt/receipt.json --auto; then
            echo "::warning::RunGlass could not post a PR comment. The receipt artifact was uploaded; check workflow token permissions."
          fi
        env:
          GITHUB_TOKEN: ${{ github.token }}
"#;

#[cfg(test)]
mod tests {
    use super::{github_snippets, parse_github_remote, render_comment_body, GithubWebRequest};
    use runglass_core::fixture::sample_report;

    #[test]
    fn preview_body_uses_local_footer_without_run_url() {
        let report = sample_report("web-github-preview".to_string());
        let body = render_comment_body(&report, None);
        assert!(body.contains("Full receipt: generated locally by RunGlass."));
        assert!(body.contains("<!-- runglass-receipt-comment:v1 -->"));
    }

    #[test]
    fn parses_github_remote_urls() {
        assert_eq!(
            parse_github_remote("git@github.com:error311/runglass.git").as_deref(),
            Some("error311/runglass")
        );
        assert_eq!(
            parse_github_remote("https://github.com/error311/runglass.git").as_deref(),
            Some("error311/runglass")
        );
    }

    #[test]
    fn snippets_include_safe_pr_commands() {
        let report = sample_report("web-github-snippets".to_string());
        let snippets = github_snippets(&report, Some("error311/runglass"), Some(1));
        assert!(snippets["dry_run"].as_str().unwrap().contains("--dry-run"));
        assert!(snippets["ci_auto"].as_str().unwrap().contains("--auto"));
        assert!(snippets["workflow"]
            .as_str()
            .unwrap()
            .contains("permissions:"));
    }

    #[test]
    fn request_defaults_to_unconfirmed() {
        let request = GithubWebRequest {
            run_id: "abc".to_string(),
            repo: None,
            pr: None,
            confirm: false,
            api_url: None,
        };
        assert!(!request.confirm);
    }
}
