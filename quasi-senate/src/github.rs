// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Async GitHub REST API client for the Senate Loop pipeline.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::info;

use crate::types::{FileContent, Issue, PullRequest};

pub struct GitHubClient {
    client: Client,
    token: String,
    repo: String,
}

impl GitHubClient {
    pub fn new(token: String, repo: String) -> Self {
        Self {
            client: Client::new(),
            token,
            repo,
        }
    }

    fn base_url(&self) -> String {
        format!("https://api.github.com/repos/{}", self.repo)
    }

    fn default_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github+json".parse().unwrap(),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            "2022-11-28".parse().unwrap(),
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            "quasi-senate/0.1".parse().unwrap(),
        );
        headers
    }

    /// Extract a GitHub error message from a JSON error response body.
    async fn github_error(&self, resp: reqwest::Response) -> anyhow::Error {
        let status = resp.status();
        match resp.json::<Value>().await {
            Ok(body) => {
                let msg = body
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown GitHub error");
                anyhow!("GitHub API error {status}: {msg}")
            }
            Err(_) => anyhow!("GitHub API error {status}"),
        }
    }

    pub async fn get_issue(&self, number: u32) -> Result<Issue> {
        let url = format!("{}/issues/{number}", self.base_url());
        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .context("GET issue: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let issue: Issue = resp.json().await.context("GET issue: deserialise")?;
        Ok(issue)
    }

    pub async fn list_open_issues(&self, max: u32) -> Result<Vec<Issue>> {
        let mut collected: Vec<Issue> = Vec::new();
        let mut next_url: Option<String> = Some(format!(
            "{}/issues?state=open&per_page=100",
            self.base_url()
        ));

        while let Some(url) = next_url.take() {
            if collected.len() >= max as usize {
                break;
            }

            let resp = self
                .client
                .get(&url)
                .headers(self.default_headers())
                .send()
                .await
                .context("list_open_issues: network error")?;

            if !resp.status().is_success() {
                return Err(self.github_error(resp).await);
            }

            // Parse Link header before consuming the body.
            next_url = parse_next_link(resp.headers());

            let page: Vec<Issue> = resp.json().await.context("list_open_issues: deserialise")?;

            for issue in page {
                if collected.len() >= max as usize {
                    break;
                }
                collected.push(issue);
            }
        }

        info!("list_open_issues: collected {} issues", collected.len());
        Ok(collected)
    }

    /// List issues closed in the last `days` days (for dedup context).
    pub async fn list_recently_closed_issues(&self, days: u32) -> Result<Vec<Issue>> {
        let since = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let since_str = since.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let url = format!(
            "{}/issues?state=closed&since={}&per_page=100&sort=updated&direction=desc",
            self.base_url(),
            since_str,
        );

        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .context("list_recently_closed_issues: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let issues: Vec<Issue> = resp
            .json()
            .await
            .context("list_recently_closed_issues: deserialise")?;

        info!("list_recently_closed_issues: {} issues closed in last {} days", issues.len(), days);
        Ok(issues)
    }

    pub async fn create_issue(
        &self,
        title: &str,
        body: &str,
        labels: &[&str],
    ) -> Result<Issue> {
        let url = format!("{}/issues", self.base_url());
        let payload = json!({
            "title": title,
            "body": body,
            "labels": labels,
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.default_headers())
            .json(&payload)
            .send()
            .await
            .context("create_issue: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let issue: Issue = resp.json().await.context("create_issue: deserialise")?;
        info!("create_issue: created #{}", issue.number);
        Ok(issue)
    }

    pub async fn get_file(&self, path: &str, branch: &str) -> Result<FileContent> {
        let url = format!(
            "{}/contents/{}?ref={}",
            self.base_url(),
            path,
            branch
        );

        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .context("get_file: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let raw: Value = resp.json().await.context("get_file: deserialise")?;

        let encoded = raw
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("get_file: missing 'content' field"))?;

        let sha = raw
            .get("sha")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("get_file: missing 'sha' field"))?
            .to_string();

        let encoding = raw
            .get("encoding")
            .and_then(|v| v.as_str())
            .unwrap_or("base64")
            .to_string();

        // GitHub wraps long lines with "\n" — strip them before decoding.
        let cleaned: String = encoded.chars().filter(|c| *c != '\n').collect();
        let decoded_bytes = BASE64
            .decode(cleaned.as_bytes())
            .context("get_file: base64 decode")?;
        let content = String::from_utf8(decoded_bytes).context("get_file: UTF-8 decode")?;

        Ok(FileContent { content, sha, encoding })
    }

    pub async fn list_files(&self, path: &str) -> Result<Vec<String>> {
        let url = format!("{}/contents/{}", self.base_url(), path);

        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .context("list_files: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let entries: Vec<Value> = resp.json().await.context("list_files: deserialise")?;
        let names: Vec<String> = entries
            .into_iter()
            .filter_map(|e| e.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect();

        Ok(names)
    }

    pub async fn create_branch(&self, name: &str, from_sha: &str) -> Result<()> {
        let url = format!("{}/git/refs", self.base_url());
        let payload = json!({
            "ref": format!("refs/heads/{name}"),
            "sha": from_sha,
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.default_headers())
            .json(&payload)
            .send()
            .await
            .context("create_branch: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        info!("create_branch: created '{name}' from {from_sha}");
        Ok(())
    }

    pub async fn create_or_update_file(
        &self,
        path: &str,
        content: &str,
        message: &str,
        branch: &str,
        sha: Option<&str>,
    ) -> Result<()> {
        let url = format!("{}/contents/{}", self.base_url(), path);
        let encoded = BASE64.encode(content.as_bytes());

        let mut payload = json!({
            "message": message,
            "content": encoded,
            "branch": branch,
        });

        if let Some(s) = sha {
            payload["sha"] = json!(s);
        }

        let resp = self
            .client
            .put(&url)
            .headers(self.default_headers())
            .json(&payload)
            .send()
            .await
            .context("create_or_update_file: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        info!("create_or_update_file: committed '{path}' on branch '{branch}'");
        Ok(())
    }

    /// Return the set of issue numbers that already have an open senate PR.
    /// Checks for PRs whose title contains `(closes #NNN)`.
    pub async fn issues_with_open_prs(&self) -> Result<std::collections::HashSet<u32>> {
        let url = format!(
            "{}/pulls?state=open&per_page=100",
            self.base_url()
        );
        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .context("issues_with_open_prs: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let prs: Vec<Value> = resp.json().await.context("issues_with_open_prs: deserialise")?;
        let re = Regex::new(r"\(closes #(\d+)\)").expect("valid regex");
        let mut covered = std::collections::HashSet::new();
        for pr in &prs {
            if let Some(title) = pr["title"].as_str() {
                if let Some(caps) = re.captures(title) {
                    if let Ok(n) = caps[1].parse::<u32>() {
                        covered.insert(n);
                    }
                }
            }
        }
        info!("issues_with_open_prs: {} issues already have open PRs", covered.len());
        Ok(covered)
    }

    pub async fn create_pull_request(
        &self,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<PullRequest> {
        let url = format!("{}/pulls", self.base_url());
        let payload = json!({
            "title": title,
            "body": body,
            "head": head,
            "base": base,
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.default_headers())
            .json(&payload)
            .send()
            .await
            .context("create_pull_request: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let pr: PullRequest = resp.json().await.context("create_pull_request: deserialise")?;
        info!("create_pull_request: opened PR #{}", pr.number);
        Ok(pr)
    }

    pub async fn list_merged_prs_since(&self, since: &str) -> Result<Vec<PullRequest>> {
        let url = format!(
            "{}/pulls?state=closed&per_page=100",
            self.base_url()
        );

        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .context("list_merged_prs_since: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let items: Vec<Value> = resp.json().await.context("list_merged_prs_since: deserialise")?;

        let mut prs: Vec<PullRequest> = Vec::new();
        for item in items {
            let merged_at = item
                .get("merged_at")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if merged_at.is_empty() {
                continue;
            }

            // String comparison works for ISO 8601 timestamps.
            if merged_at >= since {
                let number = item
                    .get("number")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("list_merged_prs_since: missing number"))? as u32;
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let html_url = item
                    .get("html_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                prs.push(PullRequest { number, title, html_url });
            }
        }

        Ok(prs)
    }

    /// Search code in this repository via the GitHub code search API.
    /// Returns the paths of matching files.
    pub async fn search_code(&self, query: &str) -> Result<Vec<String>> {
        let url = format!(
            "https://api.github.com/search/code?q={}&per_page=10",
            urlencode(&format!("{query} repo:{}", self.repo)),
        );

        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .context("search_code: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let body: Value = resp.json().await.context("search_code: deserialise")?;
        let paths = body
            .get("items")
            .and_then(|items| items.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|it| it.get("path").and_then(|p| p.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(paths)
    }

    /// Post a comment on an issue.
    pub async fn comment_on_issue(&self, number: u32, body: &str) -> Result<()> {
        let url = format!("{}/issues/{number}/comments", self.base_url());
        let payload = json!({ "body": body });

        let resp = self
            .client
            .post(&url)
            .headers(self.default_headers())
            .json(&payload)
            .send()
            .await
            .context("comment_on_issue: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        info!("comment_on_issue: commented on #{number}");
        Ok(())
    }

    /// Add labels to an issue.
    pub async fn add_labels(&self, number: u32, labels: &[&str]) -> Result<()> {
        let url = format!("{}/issues/{number}/labels", self.base_url());
        let payload = json!({ "labels": labels });

        let resp = self
            .client
            .post(&url)
            .headers(self.default_headers())
            .json(&payload)
            .send()
            .await
            .context("add_labels: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        info!("add_labels: added {labels:?} to #{number}");
        Ok(())
    }

    pub async fn get_default_branch_sha(&self) -> Result<String> {
        let url = format!("{}/git/refs/heads/main", self.base_url());

        let resp = self
            .client
            .get(&url)
            .headers(self.default_headers())
            .send()
            .await
            .context("get_default_branch_sha: network error")?;

        if !resp.status().is_success() {
            return Err(self.github_error(resp).await);
        }

        let body: Value = resp.json().await.context("get_default_branch_sha: deserialise")?;

        let sha = body
            .get("object")
            .and_then(|o| o.get("sha"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("get_default_branch_sha: missing object.sha"))?
            .to_string();

        Ok(sha)
    }
}

/// Percent-encode a string for use in a URL query parameter.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Parse the `Link` response header and return the URL for `rel="next"`, if any.
fn parse_next_link(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let link = headers.get(reqwest::header::LINK)?.to_str().ok()?;
    // Pattern: <url>; rel="next"
    let re = Regex::new(r#"<([^>]+)>;\s*rel="next""#).ok()?;
    let caps = re.captures(link)?;
    Some(caps.get(1)?.as_str().to_string())
}
