// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! B-track pre-flight "already done" check.
//!
//! Before the solver burns a full solve on an issue, this module checks whether
//! the issue's *actual deliverable* has already landed on `main` (e.g. via a
//! squash-merge under an unrelated title).
//!
//! Two stages:
//!   1. Deterministic candidate search (no LLM): extract identifiers/paths from
//!      the issue and look them up in the repository. No match → not done, cheap.
//!   2. Semantic verdict (one LLM call): only when stage 1 found candidates, ask
//!      a model whether the *deliverable* — not merely a named symbol — exists.
//!
//! Symbol presence is a cheap filter, never the verdict: "add a test for X" is
//! not done just because X exists, and "a 16-qubit GHZ example" may be done
//! under a different name.

use anyhow::Result;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use tracing::warn;

use crate::github::GitHubClient;
use crate::types::Role;

/// Maximum number of candidate identifiers/paths searched per issue.
const MAX_CANDIDATES: usize = 6;
/// Maximum number of file excerpts passed to the semantic stage.
const MAX_FILES: usize = 5;
/// Maximum characters of file content passed to the semantic stage per file.
const MAX_FILE_CHARS: usize = 6000;
/// Maximum search-hit paths fetched per symbol candidate.
const MAX_PATHS_PER_SYMBOL: usize = 2;

/// Verdict of the already-done pre-flight check.
#[derive(Debug, Clone)]
pub struct AlreadyDoneVerdict {
    pub already_done: bool,
    /// file:line citations backing the verdict. Must be non-empty when already_done.
    pub evidence: Vec<String>,
    pub reasoning: String,
}

impl AlreadyDoneVerdict {
    fn not_done(reasoning: impl Into<String>) -> Self {
        Self {
            already_done: false,
            evidence: Vec::new(),
            reasoning: reasoning.into(),
        }
    }
}

/// Raw JSON shape expected from the semantic-stage model.
#[derive(Debug, serde::Deserialize)]
struct AlreadyDoneRaw {
    already_done: bool,
    #[serde(default)]
    evidence: Vec<String>,
    #[serde(default)]
    reasoning: String,
}

/// `(path, content excerpt)` pairs found by the deterministic stage.
type SearchHits = Vec<(String, String)>;

const SYSTEM_PROMPT: &str = "\
You are a meticulous code auditor for the QUASI repository. You decide whether an open
GitHub issue is ALREADY SATISFIED by the current state of the repository on the main branch.

CRITICAL RULES:
1. The existence of a symbol, function, type, or module named in the issue does NOT mean
   the issue is done. First identify the issue's ACTUAL DELIVERABLE:
   - If the issue asks for a test, look for THAT TEST — not the symbol it would test.
   - If the issue asks for documentation, look for the documentation — not the thing documented.
   - If the issue asks for a bug fix or added validation, look for the fixed/validated
     behaviour — not merely the function named in the issue.
   - If the issue asks for an example, config, or capability, a generic implementation
     under a different name may already satisfy it — judge by capability, not by name.
2. Answer already_done = true ONLY if the specific deliverable is present in the provided
   repository excerpts, and cite file:line evidence for the DELIVERABLE itself.
3. When uncertain, answer false. A wrong false costs one redundant solve; a wrong true
   closes legitimate work.
4. Evidence is mandatory: if you cannot cite at least one file:line, answer false.

Respond with ONLY a JSON object of the form:
{\"already_done\": true|false, \"evidence\": [\"path/to/file.rs:123\", ...], \"reasoning\": \"...\"}";

/// Check whether `issue` is already satisfied by the repository on `main`.
///
/// Cheap fast-path: when no candidate identifier or path from the issue matches
/// anything in the repository, returns `already_done: false` without any LLM call.
/// When `dry_run` is true, the LLM stage is skipped and `already_done: false` is
/// returned with a reasoning string saying the semantic check was skipped.
pub async fn check_already_done(
    github: &GitHubClient,
    issue_title: &str,
    issue_body: &str,
    issue_labels: &[String],
    dry_run: bool,
) -> Result<AlreadyDoneVerdict> {
    let candidates = extract_candidates(issue_title, issue_body);
    let search = |cands: Vec<String>| async move { Ok(collect_stage1_matches(github, &cands).await) };
    let llm = |system: String, user: String| async move { call_semantic_model(&system, &user).await };
    run_check(
        issue_title,
        issue_body,
        issue_labels,
        &candidates,
        dry_run,
        search,
        llm,
    )
    .await
}

/// Core two-stage flow, generic over the search and LLM steps so tests can
/// substitute them without touching the network.
async fn run_check<S, SFut, L, LFut>(
    issue_title: &str,
    issue_body: &str,
    issue_labels: &[String],
    candidates: &[String],
    dry_run: bool,
    search: S,
    llm: L,
) -> Result<AlreadyDoneVerdict>
where
    S: FnOnce(Vec<String>) -> SFut,
    SFut: Future<Output = Result<SearchHits>>,
    L: FnOnce(String, String) -> LFut,
    LFut: Future<Output = Result<AlreadyDoneRaw>>,
{
    // Stage 1 — deterministic, no LLM.
    if candidates.is_empty() {
        return Ok(AlreadyDoneVerdict::not_done(
            "stage 1: no candidate identifiers or paths found in the issue text",
        ));
    }
    let matched = search(candidates.to_vec()).await?;
    if matched.is_empty() {
        return Ok(AlreadyDoneVerdict::not_done(
            "stage 1: no candidate identifier or path matched anything in the repository",
        ));
    }

    if dry_run {
        return Ok(AlreadyDoneVerdict {
            already_done: false,
            evidence: matched.iter().map(|(path, _)| path.clone()).collect(),
            reasoning: format!(
                "dry-run: skipped the semantic LLM check; stage 1 found {} candidate match(es) that would have been reviewed",
                matched.len()
            ),
        });
    }

    // Stage 2 — semantic, one LLM call.
    let user = build_user_prompt(issue_title, issue_body, issue_labels, &matched);
    let raw = llm(SYSTEM_PROMPT.to_string(), user).await?;
    Ok(verdict_from_raw(raw, issue_title, issue_body))
}

/// Does this issue's deliverable appear to be a test?
///
/// Used to enforce the symbol-presence trap mechanically rather than trusting
/// the model to have honoured the instruction in its prompt.
fn asks_for_a_test(issue_title: &str, issue_body: &str) -> bool {
    let haystack = format!("{issue_title}\n{issue_body}").to_lowercase();
    ["add test", "add unit test", "unit test", "add tests", "test coverage",
     "cover ", "pin down", "regression test", "#[test]"]
        .iter()
        .any(|needle| haystack.contains(needle))
}

/// Does this evidence line look like it points at an actual test?
fn evidence_looks_like_a_test(evidence: &str) -> bool {
    let e = evidence.to_lowercase();
    e.contains("#[test]")
        || e.contains("#[tokio::test]")
        || e.contains("mod tests")
        || e.contains("/tests/")
        || e.contains("_test.rs")
        || e.contains("fn test_")
}

/// Enforce the evidence contract: a claim of `already_done` without at least one
/// file:line citation is treated as not done. Additionally enforces the
/// symbol-presence trap mechanically for test-shaped issues.
fn verdict_from_raw(raw: AlreadyDoneRaw, issue_title: &str, issue_body: &str) -> AlreadyDoneVerdict {
    if raw.already_done && raw.evidence.is_empty() {
        return AlreadyDoneVerdict::not_done(format!(
            "model claimed already-done but cited no evidence; treating as not done. Original reasoning: {}",
            raw.reasoning
        ));
    }

    // Mechanical defence against the symbol-presence trap. An issue asking for a
    // test is NOT satisfied by the symbol under test already existing — the
    // classic false positive is "Add unit test for `GateName::as_str()`", where
    // the symbol is present but the test is not. The prompt says so, but a prompt
    // is not enforcement: if the deliverable is a test, the cited evidence must
    // actually point at a test, or we refuse the positive verdict.
    if raw.already_done
        && asks_for_a_test(issue_title, issue_body)
        && !raw.evidence.iter().any(|e| evidence_looks_like_a_test(e))
    {
        return AlreadyDoneVerdict::not_done(format!(
            "issue asks for a test, but the cited evidence does not reference one \
             (evidence: {:?}); a symbol existing is not a test. Treating as not done. \
             Original reasoning: {}",
            raw.evidence, raw.reasoning
        ));
    }

    AlreadyDoneVerdict {
        already_done: raw.already_done,
        evidence: raw.evidence,
        reasoning: raw.reasoning,
    }
}

/// Extract candidate identifiers and paths from the issue title and body:
/// backtick spans, `snake_case` / `CamelCase` / `Type::method` tokens, and
/// path-like strings containing `/` or ending in `.rs`.
fn extract_candidates(title: &str, body: &str) -> Vec<String> {
    let text = format!("{title}\n{body}");
    let token_re = match Regex::new(
        r"[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+|[A-Za-z][A-Za-z0-9_]*(?:::[A-Za-z][A-Za-z0-9_]*)+|[A-Za-z][A-Za-z0-9_]*",
    ) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for m in token_re.find_iter(&text) {
        let tok = m.as_str().trim_matches('.');
        if tok.len() < 3 {
            continue;
        }
        let is_path = tok.contains('/');
        let looks_like_symbol = tok.contains("::")
            || tok.ends_with(".rs")
            || (tok.contains('_') && tok.len() >= 4)
            || tok.chars().skip(1).any(|c| c.is_ascii_uppercase());
        if !is_path && !looks_like_symbol {
            continue;
        }
        // Skip URL-like matches (domain before the first slash).
        if is_path
            && tok
                .split('/')
                .next()
                .map(|head| head.contains('.'))
                .unwrap_or(false)
        {
            continue;
        }
        if seen.insert(tok.to_string()) {
            out.push(tok.to_string());
        }
        if out.len() >= MAX_CANDIDATES {
            break;
        }
    }
    out
}

/// Stage 1 against the real repository: look each candidate up via the GitHub
/// API and collect `(path, excerpt)` pairs. Per-candidate failures are logged
/// and tolerated — a failed lookup simply counts as "no match".
async fn collect_stage1_matches(github: &GitHubClient, candidates: &[String]) -> SearchHits {
    let mut seen_paths: HashSet<String> = HashSet::new();
    let mut hits: SearchHits = Vec::new();

    for candidate in candidates {
        if hits.len() >= MAX_FILES {
            break;
        }
        let paths: Vec<String> = if candidate.contains('/') || candidate.ends_with(".rs") {
            // Path-like candidate: probe the file directly.
            vec![candidate.clone()]
        } else {
            // Symbol candidate: code search. For `Type::method`, search for the
            // type — legacy code search does not handle `::` queries well.
            let query = candidate.split("::").next().unwrap_or(candidate);
            match github.search_code(query).await {
                Ok(paths) => paths.into_iter().take(MAX_PATHS_PER_SYMBOL).collect(),
                Err(e) => {
                    warn!("already_done: code search for '{query}' failed: {e}");
                    continue;
                }
            }
        };

        for path in paths {
            if hits.len() >= MAX_FILES || !seen_paths.insert(path.clone()) {
                continue;
            }
            match github.get_file(&path, "main").await {
                Ok(fc) => hits.push((path, truncate_chars(&fc.content, MAX_FILE_CHARS))),
                Err(_) => {
                    // File does not exist on main (or is unreadable) — no match.
                    seen_paths.remove(&path);
                }
            }
        }
    }
    hits
}

/// Stage 2 against the real model rotation: pick via the `a3_gate` role and call
/// through the shared provider path so retries and telemetry behave like other roles.
async fn call_semantic_model(system: &str, user: &str) -> Result<AlreadyDoneRaw> {
    let entry = crate::rotation::pick_model(&Role::A3Gate, &[], &HashMap::new(), None)?;
    let max_tokens = entry.max_tokens.unwrap_or(2048);
    let call = crate::provider::call_model(entry, system, user, 0.2, max_tokens).await?;
    crate::provider::parse_json_response::<AlreadyDoneRaw>(&call.content).map_err(|e| {
        crate::provider::ParseFailure {
            call: call.clone(),
            entry,
            error: e.to_string(),
        }
        .into()
    })
}

/// Build the stage-2 user prompt: the issue plus line-numbered file excerpts.
fn build_user_prompt(
    title: &str,
    body: &str,
    labels: &[String],
    matched: &[(String, String)],
) -> String {
    let mut s = format!(
        "## Issue\n\nTitle: {title}\n\nLabels: {}\n\nBody:\n{}\n\n",
        labels.join(", "),
        truncate_chars(body, 3000),
    );
    s.push_str("## Repository excerpts (from main, line-numbered)\n\n");
    for (path, content) in matched {
        s.push_str(&format!("### {path}\n```\n"));
        for (i, line) in content.lines().enumerate() {
            s.push_str(&format!("{}: {}\n", i + 1, line));
        }
        s.push_str("```\n\n");
    }
    s.push_str(
        "Does the repository already satisfy this issue's ACTUAL DELIVERABLE (per the system rules)? Answer with the JSON object only.",
    );
    s
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn search_none(_cands: Vec<String>) -> Result<SearchHits> {
        Ok(Vec::new())
    }

    async fn search_ast_rs(_cands: Vec<String>) -> Result<SearchHits> {
        Ok(vec![(
            "afana/src/ast.rs".to_string(),
            "impl GateName {\n    pub fn as_str(&self) -> &'static str {\n        match self {\n            GateName::H => \"h\",\n        }\n    }\n}".to_string(),
        )])
    }

    async fn llm_must_not_be_called(_system: String, _user: String) -> Result<AlreadyDoneRaw> {
        Err(anyhow::anyhow!("LLM must not be called in this scenario"))
    }

    /// Stage 1 fast path: no candidate symbol matches → not done, no LLM call.
    #[tokio::test]
    async fn stage1_no_match_returns_not_done_without_llm() {
        let verdict = run_check(
            "Add frobnicate support",
            "Please add frobnicate support to the parser.",
            &[],
            &["frobnicate_parser".to_string()],
            false,
            search_none,
            llm_must_not_be_called,
        )
        .await
        .expect("check should succeed");

        assert!(!verdict.already_done);
        assert!(verdict.reasoning.contains("stage 1"));
    }

    /// The GateName::as_str() trap: the issue asks for a TEST of an existing
    /// symbol. Symbol presence alone must not close the issue — the stage-2
    /// prompt must force the model to look for the test deliverable, and a
    /// "test not found" answer yields not-done.
    #[tokio::test]
    async fn test_request_for_existing_symbol_is_not_already_done() {
        let llm = |system: String, user: String| async move {
            // The prompt must explicitly defuse the symbol-presence trap.
            assert!(
                system.contains("does NOT mean"),
                "system prompt must warn that a symbol's existence is not the deliverable"
            );
            assert!(
                system.contains("look for THAT TEST"),
                "system prompt must direct the model at the test deliverable"
            );
            assert!(
                user.contains("Add unit test for `GateName::as_str()`"),
                "user prompt must carry the issue title"
            );
            assert!(user.contains("afana/src/ast.rs"));
            // A correct model: the symbol exists, but its test does not.
            Ok(AlreadyDoneRaw {
                already_done: false,
                evidence: vec![],
                reasoning: "GateName::as_str exists in afana/src/ast.rs, but no unit test for it was found".to_string(),
            })
        };

        let verdict = run_check(
            "Add unit test for `GateName::as_str()`",
            "GateName::as_str in afana/src/ast.rs has no unit test. Add one covering every variant.",
            &[],
            &["GateName::as_str".to_string()],
            false,
            search_ast_rs,
            llm,
        )
        .await
        .expect("check should succeed");

        assert!(!verdict.already_done);
    }

    /// Evidence is mandatory: already_done=true with empty evidence → not done.
    #[tokio::test]
    async fn already_done_without_evidence_is_rejected() {
        let raw = AlreadyDoneRaw {
            already_done: true,
            evidence: vec![],
            reasoning: "trust me".to_string(),
        };
        let verdict = verdict_from_raw(raw, "Add GHZ example", "body");
        assert!(!verdict.already_done);
        assert!(verdict.reasoning.contains("no evidence"));

        let raw_with_evidence = AlreadyDoneRaw {
            already_done: true,
            evidence: vec!["examples/ghz.paul:1".to_string()],
            reasoning: "GHZ example present".to_string(),
        };
        let verdict = verdict_from_raw(raw_with_evidence, "Add GHZ example", "body");
        assert!(verdict.already_done);
        assert_eq!(verdict.evidence, vec!["examples/ghz.paul:1".to_string()]);
    }

    /// The symbol-presence trap, enforced in code rather than in the prompt.
    ///
    /// Here the model gets it WRONG — it claims the issue is already done and
    /// cites the symbol's own definition as evidence. Because the issue asks for
    /// a test and the evidence points at a plain function, the verdict must be
    /// overruled. This is the test the prompt-only version could not provide,
    /// since stubbing the model to answer correctly proves nothing.
    #[test]
    fn test_issue_is_overruled_when_evidence_is_not_a_test() {
        let raw = AlreadyDoneRaw {
            already_done: true,
            evidence: vec!["afana/src/ast.rs:62".to_string()],
            reasoning: "GateName::as_str exists".to_string(),
        };
        let verdict = verdict_from_raw(
            raw,
            "Add unit test for `GateName::as_str()`",
            "GateName::as_str has no unit test. Add one covering every variant.",
        );
        assert!(
            !verdict.already_done,
            "a symbol definition is not a test; the positive verdict must be refused"
        );
        assert!(verdict.reasoning.contains("does not reference one"));
    }

    /// The same guard must not fire when the evidence really is a test.
    #[test]
    fn test_issue_is_accepted_when_evidence_is_a_test() {
        let raw = AlreadyDoneRaw {
            already_done: true,
            evidence: vec!["afana/src/ast.rs:540 mod tests / fn as_str_returns_canonical_name".to_string()],
            reasoning: "the test already exists".to_string(),
        };
        let verdict = verdict_from_raw(
            raw,
            "Add unit test for `GateName::as_str()`",
            "GateName::as_str has no unit test.",
        );
        assert!(verdict.already_done, "genuine test evidence must be accepted");
    }

    /// A non-test issue is unaffected by the test-evidence guard.
    #[test]
    fn non_test_issue_unaffected_by_test_guard() {
        let raw = AlreadyDoneRaw {
            already_done: true,
            evidence: vec!["afana/src/zx_ir.rs:348".to_string()],
            reasoning: "find_fusible_pairs already implemented".to_string(),
        };
        let verdict = verdict_from_raw(
            raw,
            "Implement find_fusible_pairs on ZxGraph",
            "Add a method that returns fusible spider pairs.",
        );
        assert!(verdict.already_done);
    }

    /// dry_run: stage 1 runs, but the LLM stage is skipped → not done.
    #[tokio::test]
    async fn dry_run_skips_llm_call() {
        let verdict = run_check(
            "Add unit test for `GateName::as_str()`",
            "body",
            &[],
            &["GateName::as_str".to_string()],
            true,
            search_ast_rs,
            llm_must_not_be_called,
        )
        .await
        .expect("check should succeed");

        assert!(!verdict.already_done);
        assert!(verdict.reasoning.contains("dry-run"));
        assert!(verdict.reasoning.contains("skipped"));
    }

    #[test]
    fn extract_candidates_finds_symbols_and_paths() {
        let cands = extract_candidates(
            "Add unit test for `GateName::as_str()`",
            "The method lives in afana/src/ast.rs next to parse_gate_name. See https://github.com/ehrenfest-quantum/quasi for context.",
        );
        assert!(cands.iter().any(|c| c == "GateName::as_str"));
        assert!(cands.iter().any(|c| c == "afana/src/ast.rs"));
        assert!(cands.iter().any(|c| c == "parse_gate_name"));
        // Plain English words and URL hosts are not candidates.
        assert!(!cands.iter().any(|c| c == "Add"));
        assert!(!cands.iter().any(|c| c.contains("github.com")));
    }
}
