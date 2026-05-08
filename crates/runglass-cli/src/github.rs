use std::env;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use runglass_core::{render_summary_markdown_receipt, RunReport};
use serde_json::json;

const COMMENT_MARKER: &str = "<!-- runglass-receipt-comment:v1 -->";
const DEFAULT_API_URL: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2026-03-10";

pub(crate) struct GithubCommentOptions {
    pub repo: Option<String>,
    pub pr: Option<u64>,
    pub auto: bool,
    pub dry_run: bool,
    pub api_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubContext {
    repo: Option<String>,
    pr: Option<u64>,
    sha: Option<String>,
    run_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepoParts {
    owner: String,
    repo: String,
}

pub(crate) fn detect_command(repo: Option<String>, pr: Option<u64>) -> Result<()> {
    let context = detect_context(repo, pr)?;
    println!("GitHub context");
    print_detected("Repository", context.repo.as_deref());
    print_detected(
        "Pull request",
        context.pr.map(|value| value.to_string()).as_deref(),
    );
    print_detected("Commit SHA", context.sha.as_deref());
    print_detected("Run URL", context.run_url.as_deref());
    print_detected("Token", token_source().as_deref());
    Ok(())
}

pub(crate) fn comment_command(report: &RunReport, options: GithubCommentOptions) -> Result<()> {
    let context = detect_context(options.repo.clone(), options.pr)?;
    let repo = context.repo.as_deref().ok_or_else(|| {
        anyhow!("missing GitHub repository; pass --repo owner/name or use --auto in GitHub Actions")
    })?;
    let pr = context
        .pr
        .ok_or_else(|| anyhow!("missing pull request number; pass --pr <number> or use --auto in a pull_request GitHub Actions run"))?;
    let repo = parse_repo(repo)?;
    let body = render_comment_body(report, context.run_url.as_deref());

    if options.dry_run {
        print!("{body}");
        return Ok(());
    }

    if !options.auto && options.repo.is_none() && options.pr.is_none() {
        return Err(anyhow!(
            "posting requires explicit --repo and --pr, or --auto in GitHub Actions"
        ));
    }

    let token = resolve_token()?;
    let client = GithubApiClient::new(options.api_url.as_deref().unwrap_or(DEFAULT_API_URL), token);
    match client.find_marker_comment(&repo, pr)? {
        Some(comment_id) => {
            client.update_comment(&repo, comment_id, &body)?;
            println!(
                "Updated RunGlass PR receipt comment on {}/{}#{}",
                repo.owner, repo.repo, pr
            );
        }
        None => {
            client.create_comment(&repo, pr, &body)?;
            println!(
                "Created RunGlass PR receipt comment on {}/{}#{}",
                repo.owner, repo.repo, pr
            );
        }
    }

    Ok(())
}

fn detect_context(repo: Option<String>, pr: Option<u64>) -> Result<GithubContext> {
    let repo = repo
        .or_else(|| env::var("GITHUB_REPOSITORY").ok())
        .or_else(|| git_remote_repo().ok().flatten());
    let pr = pr.or_else(detect_github_actions_pr);
    let sha = env::var("GITHUB_SHA").ok();
    let run_url = github_run_url();

    Ok(GithubContext {
        repo,
        pr,
        sha,
        run_url,
    })
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

fn normalize_repo(value: &str) -> Option<String> {
    let mut parts = value.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
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

fn parse_pull_ref(value: &str) -> Option<u64> {
    let mut parts = value.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("refs"), Some("pull"), Some(number)) => number.parse().ok(),
        _ => None,
    }
}

fn render_comment_body(report: &RunReport, run_url: Option<&str>) -> String {
    let mut body = render_summary_markdown_receipt(report)
        .trim_end()
        .to_string();
    if let Some(run_url) = run_url.filter(|value| !value.trim().is_empty()) {
        body.push_str("\n\nCI run: ");
        body.push_str(run_url);
    }
    body.push_str("\n\n");
    body.push_str(COMMENT_MARKER);
    body.push('\n');
    body
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

fn print_detected(name: &str, value: Option<&str>) {
    println!("{}\t{}", name, value.unwrap_or("not detected"));
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

#[cfg(test)]
mod tests {
    use super::{
        parse_github_remote, parse_pull_ref, parse_repo, render_comment_body, COMMENT_MARKER,
    };
    use runglass_core::fixture::sample_report;

    #[test]
    fn parses_common_github_remote_urls() {
        assert_eq!(
            parse_github_remote("git@github.com:error311/runglass.git").as_deref(),
            Some("error311/runglass")
        );
        assert_eq!(
            parse_github_remote("https://github.com/error311/runglass.git").as_deref(),
            Some("error311/runglass")
        );
        assert_eq!(parse_github_remote("https://example.com/repo.git"), None);
    }

    #[test]
    fn parses_repo_owner_and_name() {
        let repo = parse_repo("error311/runglass").expect("repo");
        assert_eq!(repo.owner, "error311");
        assert_eq!(repo.repo, "runglass");
    }

    #[test]
    fn parses_pull_request_refs() {
        assert_eq!(parse_pull_ref("refs/pull/123/merge"), Some(123));
        assert_eq!(parse_pull_ref("refs/heads/main"), None);
    }

    #[test]
    fn comment_body_includes_marker_and_run_url() {
        let report = sample_report("github-comment-test".to_string());
        let body = render_comment_body(
            &report,
            Some("https://github.com/error311/runglass/actions/runs/1"),
        );
        assert!(body.contains("## RunGlass Receipt"));
        assert!(body.contains("CI run: https://github.com/error311/runglass/actions/runs/1"));
        assert!(body.contains(COMMENT_MARKER));
    }
}
