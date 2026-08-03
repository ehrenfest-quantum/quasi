// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! B.1 Agentic Issue Solver.
//!
//! Replaces the single-shot blind find/replace solver (`solver.rs`) with a
//! tool loop that works in a real checkout: the model reads files, writes
//! whole files, and runs the workspace test suite itself before finishing.
//!
//! The loop is deliberately simple: one JSON action per model call, a growing
//! transcript re-sent as the user prompt each step (no message-array API),
//! and whole-file writes only — there is no find/replace action, because
//! blind structural surgery on source it has never seen is the failure mode
//! this module exists to eliminate (see issue #1118's brace mismatch).
//!
//! Testability: the workspace root is a parameter and the model call is an
//! injected closure (the same pattern as `ledger::submit_proposal_with`), so
//! every piece except the clone and the real LLM call is testable without
//! network access.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::github::GitHubClient;
use crate::types::{Role, RotationEntry, SolveResult};

/// Maximum number of model calls (steps) in one agentic solve session.
///
/// The floor for a real fix is about six steps: read, write, test, read the
/// failure, write again, test again. Twelve left almost no room for a wrong
/// turn, and on issue #1118 the agent hit the budget mid-iteration rather than
/// converging — it was still working, not stuck. The wall-clock deadline below
/// is the real guard against a runaway session; the step budget only needs to
/// be generous enough that exhausting it means something.
const MAX_STEPS: u32 = 30;

/// Wall-clock ceiling for one agent session, independent of the step budget.
///
/// The step budget bounds how many times the model is consulted; it does not
/// bound how long each consultation takes. On issue #1118 a hanging provider
/// (minimax-m3) sat on a single call for 22 minutes and would have consumed the
/// whole 50-minute process timeout while producing zero file changes. A step
/// budget multiplies exposure to that by MAX_STEPS; a deadline does not.
/// Raised alongside MAX_STEPS: a `test` action runs the whole workspace suite
/// and costs minutes on its own, so thirty steps do not fit in fifteen minutes.
const MAX_WALL_CLOCK: std::time::Duration = std::time::Duration::from_secs(1500);

/// Cap on `read` results, in characters (after line-number prefixing).
const READ_CAP: usize = 120_000;

/// How many trailing characters of cargo output are returned on test failure.
const TEST_TAIL_CAP: usize = 3000;

/// Maximum number of `error`-prefixed lines surfaced before the output tail.
const TEST_ERROR_LINES: usize = 20;

/// Same repo URL used by `pipeline::pre_review_cargo_check`.
const REPO_URL: &str = "https://github.com/ehrenfest-quantum/quasi.git";

// ── Actions ─────────────────────────────────────────────────────────────────

/// One step requested by the model. Parsed from a single JSON object.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum Action {
    /// List directory entries, one per line.
    List { path: String },
    /// Read a file with line numbers prefixed.
    Read { path: String },
    /// Replace a file's ENTIRE contents. Creates parent directories.
    Write { path: String, content: String },
    /// Run `cargo test --workspace --all-targets` in the workspace.
    Test,
    /// End the loop with a one-sentence summary.
    Finish { summary: String },
}

/// Parse the model's response as a single JSON action. Tolerates prose and
/// markdown fences via `provider::parse_json_response`. Unknown actions and
/// malformed JSON both come back as `Err` — the caller appends the error to
/// the transcript and consumes a step rather than aborting the loop.
fn parse_action(raw: &str) -> Result<Action> {
    crate::provider::parse_json_response::<Action>(raw)
}

/// The outcome of executing one model response.
enum StepOutcome {
    /// Loop continues; the string is the action result shown to the model.
    Continue(String),
    /// Model called `finish`; the string is its summary.
    Finish(String),
}

// ── Path safety ─────────────────────────────────────────────────────────────
//
// The `path` field is untrusted model input. Anything absolute, containing
// `..`, or resolving outside the workspace root (e.g. through a symlink) is
// rejected with an error result — never touched on disk.

fn resolve_within(root: &Path, rel: &str) -> std::result::Result<PathBuf, String> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(format!("path '{rel}' is absolute — only repo-relative paths are allowed"));
    }
    for comp in rel_path.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err(format!(
                    "path '{rel}' contains '..', a root, or a prefix component — not allowed"
                ));
            }
        }
    }
    if rel_path.components().next().is_none() {
        return Err("path is empty".to_string());
    }

    let candidate = root.join(rel_path);

    // Canonicalize the deepest existing ancestor and verify it stays under
    // the canonical workspace root. This catches symlinks inside the
    // workspace that point outside it.
    let canon_root = root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize workspace root: {e}"))?;
    let mut ancestor: &Path = &candidate;
    loop {
        if ancestor.exists() {
            break;
        }
        match ancestor.parent() {
            Some(p) => ancestor = p,
            None => return Err(format!("path '{rel}' has no existing ancestor")),
        }
    }
    let canon_ancestor = ancestor
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize '{}': {e}", ancestor.display()))?;
    if !canon_ancestor.starts_with(&canon_root) {
        return Err(format!(
            "path '{rel}' resolves outside the workspace root — not allowed"
        ));
    }

    Ok(candidate)
}

// ── Action execution ────────────────────────────────────────────────────────

/// Execute one parsed model response against the workspace.
fn step(workspace: &Path, raw: &str) -> StepOutcome {
    match parse_action(raw) {
        Err(e) => StepOutcome::Continue(format!(
            "ERROR: your response was not a single valid JSON action: {e}\n\
             Respond with EXACTLY ONE JSON object, e.g. \
             {{\"action\":\"read\",\"path\":\"afana/src/lib.rs\"}} — nothing else."
        )),
        Ok(Action::Finish { summary }) => StepOutcome::Finish(summary),
        Ok(Action::List { path }) => StepOutcome::Continue(list_action(workspace, &path)),
        Ok(Action::Read { path }) => StepOutcome::Continue(read_action(workspace, &path)),
        Ok(Action::Write { path, content }) => {
            StepOutcome::Continue(write_action(workspace, &path, &content))
        }
        Ok(Action::Test) => StepOutcome::Continue(test_action(workspace)),
    }
}

fn list_action(workspace: &Path, path: &str) -> String {
    let full = match resolve_within(workspace, path) {
        Ok(p) => p,
        Err(e) => return format!("ERROR: {e}"),
    };
    let entries = match std::fs::read_dir(&full) {
        Ok(rd) => rd,
        Err(e) => return format!("ERROR: cannot list '{path}': {e}"),
    };
    let mut names: Vec<String> = entries
        .filter_map(|de| {
            let de = de.ok()?;
            let mut name = de.file_name().to_string_lossy().into_owned();
            if de.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                name.push('/');
            }
            Some(name)
        })
        .collect();
    names.sort();
    if names.is_empty() {
        format!("'{path}' is empty (or contains no readable entries)")
    } else {
        names.join("\n")
    }
}

fn read_action(workspace: &Path, path: &str) -> String {
    let full = match resolve_within(workspace, path) {
        Ok(p) => p,
        Err(e) => return format!("ERROR: {e}"),
    };
    let content = match std::fs::read_to_string(&full) {
        Ok(c) => c,
        Err(e) => return format!("ERROR: cannot read '{path}': {e}"),
    };

    // Prefix 1-based line numbers so the model can refer to lines.
    let mut out = String::new();
    let mut truncated = false;
    for (i, line) in content.lines().enumerate() {
        if out.len() + line.len() + 16 > READ_CAP {
            truncated = true;
            break;
        }
        out.push_str(&format!("{:>6}\t{}\n", i + 1, line));
    }
    // A single pathological line longer than the cap: hard-truncate it.
    if out.len() > READ_CAP {
        let mut end = READ_CAP;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        truncated = true;
    }
    if truncated {
        out.push_str(&format!(
            "\n[TRUNCATED: '{path}' is {} chars total; only the first ~{READ_CAP} are shown]",
            content.len()
        ));
    }
    out
}

fn write_action(workspace: &Path, path: &str, content: &str) -> String {
    let full = match resolve_within(workspace, path) {
        Ok(p) => p,
        Err(e) => return format!("ERROR: {e}"),
    };
    if let Some(parent) = full.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return format!("ERROR: cannot create parent directories for '{path}': {e}");
        }
    }
    match std::fs::write(&full, content) {
        Ok(()) => format!(
            "OK: wrote {} bytes to '{path}' (whole-file replace)",
            content.len()
        ),
        Err(e) => format!("ERROR: cannot write '{path}': {e}"),
    }
}

/// Run the workspace test suite under `timeout 900` and report the result.
fn test_action(workspace: &Path) -> String {
    // Ensure cargo + rustc are in PATH — the daemon user may not have
    // /root/.cargo/bin (same fix as pipeline::pre_review_cargo_check).
    let mut path = std::env::var("PATH").unwrap_or_default();
    if !path.contains(".cargo/bin") {
        path = format!("/root/.cargo/bin:{path}");
    }

    let output = Command::new("timeout")
        .arg("900")
        .arg("cargo")
        .args(["test", "--workspace", "--all-targets"])
        .current_dir(workspace)
        .env("CARGO_TERM_COLOR", "never")
        .env("PATH", &path)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => return format!("ERROR: failed to run cargo test: {e}"),
    };

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    if output.status.code() == Some(124) {
        return format!(
            "cargo test TIMED OUT after 900 seconds (timeout exit code 124).\n{}",
            tail(&combined, TEST_TAIL_CAP)
        );
    }
    if output.status.success() {
        "cargo test --workspace --all-targets: OK — all tests passed.".to_string()
    } else {
        format_test_failure(&combined)
    }
}

/// The last `cap` characters of `s`, on a char boundary.
fn tail(s: &str, cap: usize) -> &str {
    if s.len() <= cap {
        return s;
    }
    let mut start = s.len() - cap;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

/// Format failing cargo output for the model: up to `TEST_ERROR_LINES` lines
/// beginning with `error` FIRST, then the last `TEST_TAIL_CAP` characters.
///
/// The error lines come first because cargo's tail is usually progress noise
/// ("test result: FAILED", compiling lines) while the real `error[Exxxx]:`
/// block sits much earlier — on issue #1118 a tail-only view swallowed the
/// actual failure entirely.
fn format_test_failure(output: &str) -> String {
    let mut result = String::from("cargo test FAILED.\n");

    let error_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.starts_with("error"))
        .take(TEST_ERROR_LINES)
        .collect();
    if !error_lines.is_empty() {
        result.push_str("\nError lines (up to 20, shown first — the tail below is mostly progress noise):\n");
        for line in error_lines {
            result.push_str(line);
            result.push('\n');
        }
    }

    result.push_str(&format!(
        "\nOutput tail (last {TEST_TAIL_CAP} characters):\n{}",
        tail(output, TEST_TAIL_CAP)
    ));
    result
}

// ── Changed-file collection ─────────────────────────────────────────────────

/// Collect every file that differs from HEAD using `git status --porcelain`,
/// keyed by repo-relative path with full current content. Deleted files are
/// skipped (there is nothing to write back). Binary/unreadable files are
/// skipped with a warning rather than failing the whole solve.
fn collect_changed_files(
    workspace: &Path,
) -> std::result::Result<HashMap<String, String>, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workspace)
        .output()
        .map_err(|e| format!("git status failed to run: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files = HashMap::new();
    for line in stdout.lines() {
        // Porcelain format: two status columns, a space, then the path.
        if line.len() < 4 {
            continue;
        }
        let status = &line[..2];
        let path_part = line[3..].trim();
        // Rename entries look like "old -> new"; the new name is what exists.
        let path = path_part.rsplit(" -> ").next().unwrap_or(path_part);
        if status.contains('D') {
            tracing::warn!("agent: skipping deleted file '{path}' — nothing to collect");
            continue;
        }
        match std::fs::read_to_string(workspace.join(path)) {
            Ok(content) => {
                files.insert(path.to_string(), content);
            }
            Err(e) => {
                tracing::warn!("agent: skipping unreadable changed file '{path}': {e}");
            }
        }
    }
    Ok(files)
}

// ── The loop (testable core) ────────────────────────────────────────────────

/// What one agentic session produced.
pub struct AgentOutcome {
    /// The `finish` summary, or `None` if the step budget was exhausted.
    pub summary: Option<String>,
    /// The final model call's metadata.
    pub last_call: crate::provider::CallResult,
    /// Sum of `latency_ms` across every model call in the session.
    pub total_latency_ms: u64,
    /// The full transcript (issue, actions, results).
    pub transcript: String,
    /// How many steps (model calls) were used.
    pub steps: u32,
}

/// Run the agentic tool loop against `workspace` (a real repo checkout).
///
/// `model_call` receives the current transcript and must return the model's
/// response. Injecting it keeps the loop testable without network or a real
/// clone — the same pattern as `ledger::submit_proposal_with`.
pub async fn run_agent_loop<F, Fut>(
    workspace: &Path,
    issue_prompt: &str,
    mut model_call: F,
) -> Result<AgentOutcome>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<crate::provider::CallResult>>,
{
    let mut transcript = issue_prompt.to_string();
    let mut total_latency_ms: u64 = 0;
    let mut summary: Option<String> = None;
    let mut last_call: Option<crate::provider::CallResult> = None;
    let mut steps: u32 = 0;
    let started = std::time::Instant::now();
    let mut timed_out = false;
    let mut call_error: Option<String> = None;

    while steps < MAX_STEPS {
        // Check before spending another call, not after: the point is to stop
        // starting work we cannot afford to finish.
        if started.elapsed() >= MAX_WALL_CLOCK {
            tracing::warn!(
                elapsed_s = started.elapsed().as_secs(),
                steps_used = steps,
                "agent loop hit its wall-clock deadline — abandoning remaining steps"
            );
            timed_out = true;
            break;
        }
        steps += 1;

        // A failed call ends the session but must not discard it. Propagating
        // with `?` here threw away 146 lines of working edits on issue #1118
        // when the provider stalled on the last step: the work was on disk and
        // `git status` could see it, but the error returned before collection.
        // A model that has already produced a good edit should not be punished
        // for its provider dying afterwards.
        let call = match model_call(transcript.clone()).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    steps_used = steps,
                    "agent loop lost its model — keeping whatever it already wrote"
                );
                call_error = Some(e.to_string());
                steps = steps.saturating_sub(1); // the step bought nothing
                break;
            }
        };
        total_latency_ms = total_latency_ms.saturating_add(call.latency_ms);
        let raw = call.content.clone();
        last_call = Some(call);

        match step(workspace, &raw) {
            StepOutcome::Continue(result) => {
                transcript.push_str(&format!(
                    "\n\n## Your action (step {steps}/{MAX_STEPS})\n{raw}\n\n## Result\n{result}"
                ));
            }
            StepOutcome::Finish(s) => {
                transcript.push_str(&format!(
                    "\n\n## Your action (step {steps}/{MAX_STEPS})\n{raw}\n\n## Result\nLoop finished."
                ));
                summary = Some(s);
                break;
            }
        }
    }

    // No successful call at all means there is nothing to collect and no
    // telemetry to report, so this really is a failure — but say which kind.
    let last_call = last_call.ok_or_else(|| match &call_error {
        Some(e) => anyhow!("agent loop made no successful model call: {e}"),
        None => anyhow!("agent loop ran zero steps (MAX_STEPS={MAX_STEPS})"),
    })?;

    if summary.is_none() {
        if let Some(e) = &call_error {
            summary = Some(format!(
                "agent loop ended at step {steps} because the model call failed ({e}); \
                 any changes below are partial"
            ));
        }
    }

    if timed_out && summary.is_none() {
        // Distinguish "ran out of thinking" from "ran out of time" — the two
        // call for different responses from an operator.
        summary = Some(format!(
            "agent loop abandoned after {}s wall clock ({steps} steps used);              any changes below are partial",
            started.elapsed().as_secs()
        ));
    }

    Ok(AgentOutcome {
        summary,
        last_call,
        total_latency_ms,
        transcript,
        steps,
    })
}

// ── Temp-dir guard ──────────────────────────────────────────────────────────

/// Removes the clone directory on drop, on every exit path.
struct TempCheckout(String);

impl Drop for TempCheckout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ── Public entry point ──────────────────────────────────────────────────────

/// Solve an issue agentically (B.1). Drop-in replacement for
/// `solver::solve_issue` minus `retry_feedback`: the return tuple is identical
/// so `pipeline.rs` can call either.
///
/// Clones the repo into a temp dir, runs the tool loop (read/list/write/test)
/// until the model finishes or `MAX_STEPS` is exhausted, then collects every
/// file that differs from HEAD into `SolveResult.new_files`.
#[allow(clippy::too_many_arguments)]
pub async fn solve_agentic(
    github: &GitHubClient,
    issue_number: u32,
    issue_title: &str,
    issue_body: &str,
    issue_labels: &[String],
    exclude: &[&str],
    counts: &HashMap<String, u32>,
    last_provider: Option<&str>,
    dry_run: bool,
) -> Result<(
    SolveResult,
    &'static RotationEntry,
    crate::provider::CallResult,
    String,
)> {
    // The agent works in a local clone, not through the GitHub API; the
    // parameter exists only to keep the signature aligned with
    // `solver::solve_issue` so the pipeline can call either.
    let _ = github;

    // 1. Pick model (same as solver::solve_issue).
    let entry = crate::rotation::pick_model(&Role::B1Solver, exclude, counts, last_provider)?;

    // 2. Dry-run short-circuit: no clone, no model call, no filesystem change.
    if dry_run {
        println!(
            "[dry-run] agent: would clone repo and run the agentic loop with model '{}' for issue #{} {:?}",
            entry.id, issue_number, issue_title,
        );
        let placeholder = SolveResult {
            reasoning: "Dry-run placeholder reasoning.".to_string(),
            edits: vec![],
            new_files: HashMap::new(),
            solver_model: entry.id.to_string(),
        };
        let dummy_call = crate::provider::CallResult {
            content: "dry-run".to_string(),
            latency_ms: 0,
            http_status: 0,
            retries: 0,
            model_verified: None,
            served_model: None,
            input_len: 0,
        };
        return Ok((placeholder, entry, dummy_call, String::new()));
    }

    // 3. Shallow-clone the repo into a temp dir (same approach as
    //    pipeline::pre_review_cargo_check; TempCheckout cleans up on every
    //    exit path).
    let tmp_dir = format!("/tmp/senate-agent-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let clone = Command::new("git")
        .args(["clone", "--depth", "1", REPO_URL, &tmp_dir])
        .output()
        .map_err(|e| anyhow!("agent: git clone failed to run: {e}"))?;
    if !clone.status.success() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(anyhow!(
            "agent: git clone failed: {}",
            String::from_utf8_lossy(&clone.stderr)
        ));
    }
    let _checkout = TempCheckout(tmp_dir.clone());
    let workspace = Path::new(&tmp_dir);

    // 4. Seed the transcript with the issue.
    let issue_prompt = format!(
        "## Issue #{issue_number}: {issue_title}\n\n{issue_body}\n\n\
         Labels: {}\n\n\
         You are working in a full checkout of the repository (workspace root). \
         Begin by listing and reading the files relevant to this issue.",
        issue_labels.join(", ")
    );

    // 5. Run the loop. The transcript is re-sent as the user prompt on every
    //    step — intentionally; no message-array API is added to provider.rs.
    let system = crate::prompts::agent_system_prompt();
    let max_tokens = entry.max_tokens.unwrap_or(8192);
    let outcome = run_agent_loop(workspace, &issue_prompt, move |transcript| async move {
        crate::provider::call_model(entry, system, &transcript, 0.2, max_tokens).await
    })
    .await?;

    // 6. Collect every file that differs from HEAD.
    let new_files = collect_changed_files(workspace).map_err(|e| anyhow!("agent: {e}"))?;

    // 7. Telemetry: return the FINAL call's metadata, but with latency_ms
    //    summed across the whole session so the recorded latency reflects all
    //    turns. NOTE (honest limitation): token counts (input_len, content
    //    length) are per-call, so only the final call's counts are reported —
    //    the earlier turns' input/output sizes are not aggregated anywhere.
    let mut call = outcome.last_call;
    call.latency_ms = outcome.total_latency_ms;

    // 8. No changes → ParseFailure-style error so the pipeline records a
    //    failed attempt rather than opening an empty PR.
    if new_files.is_empty() {
        return Err(crate::provider::ParseFailure {
            call,
            entry,
            error: format!(
                "agentic solver made no file changes for issue #{issue_number} after {} step(s)",
                outcome.steps
            ),
        }
        .into());
    }

    let reasoning = match outcome.summary {
        Some(s) => s,
        None => format!(
            "Step budget exhausted: the agent did not finish within {MAX_STEPS} steps; \
             these are the partial changes it left in the workspace."
        ),
    };

    tracing::info!(
        "Agentic solver complete: model={} issue=#{} steps={} files_changed={}",
        entry.id,
        issue_number,
        outcome.steps,
        new_files.len(),
    );

    let result = SolveResult {
        reasoning,
        edits: vec![],
        new_files,
        solver_model: entry.id.to_string(),
    };

    // The transcript doubles as the context string returned to the pipeline —
    // the reviewer sees what the agent actually did and observed.
    Ok((result, entry, call, outcome.transcript))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique temp workspace per test; removed on drop.
    struct TestWorkspace(PathBuf);

    impl TestWorkspace {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "senate-agent-test-{}-{}",
                name,
                &uuid::Uuid::new_v4().to_string()[..8]
            ));
            std::fs::create_dir_all(&dir).expect("create temp workspace");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // ── Path safety ─────────────────────────────────────────────────────────

    #[test]
    fn path_safety_rejects_absolute_paths() {
        let ws = TestWorkspace::new("absolute");
        let result = resolve_within(ws.path(), "/etc/passwd");
        assert!(result.is_err(), "absolute path must be rejected");
    }

    #[test]
    fn path_safety_rejects_dot_dot_traversal() {
        let ws = TestWorkspace::new("dotdot");
        for p in ["../outside.txt", "afana/../../etc/passwd", "a/./../../b"] {
            assert!(
                resolve_within(ws.path(), p).is_err(),
                "traversal path '{p}' must be rejected"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn path_safety_rejects_symlink_resolving_outside_workspace() {
        let ws = TestWorkspace::new("symlink");
        let outside = TestWorkspace::new("symlink-outside");
        std::os::unix::fs::symlink(outside.path(), ws.path().join("escape"))
            .expect("create symlink");
        let result = resolve_within(ws.path(), "escape/evil.txt");
        assert!(
            result.is_err(),
            "path resolving outside the workspace via symlink must be rejected"
        );
    }

    #[test]
    fn path_safety_accepts_normal_repo_relative_paths() {
        let ws = TestWorkspace::new("normal");
        std::fs::create_dir_all(ws.path().join("afana/src")).expect("mkdir");
        let resolved = resolve_within(ws.path(), "afana/src/optimize.rs");
        assert!(resolved.is_ok(), "normal relative path must be accepted");
        assert!(resolved.expect("checked above").starts_with(ws.path()));
    }

    // ── Action parsing ──────────────────────────────────────────────────────

    #[test]
    fn action_parsing_all_five_actions() {
        let list = parse_action(r#"{"action":"list","path":"afana/src"}"#)
            .expect("list should parse");
        assert!(matches!(list, Action::List { ref path } if path == "afana/src"));

        let read = parse_action(r#"{"action":"read","path":"afana/src/optimize.rs"}"#)
            .expect("read should parse");
        assert!(matches!(read, Action::Read { ref path } if path == "afana/src/optimize.rs"));

        let write = parse_action(r#"{"action":"write","path":"a.rs","content":"fn main() {}"}"#)
            .expect("write should parse");
        assert!(
            matches!(write, Action::Write { ref path, ref content } if path == "a.rs" && content == "fn main() {}")
        );

        let test = parse_action(r#"{"action":"test"}"#).expect("test should parse");
        assert!(matches!(test, Action::Test));

        let finish = parse_action(r#"{"action":"finish","summary":"done"}"#)
            .expect("finish should parse");
        assert!(matches!(finish, Action::Finish { ref summary } if summary == "done"));
    }

    #[test]
    fn action_parsing_unknown_action_is_an_error_not_a_panic() {
        let result = parse_action(r#"{"action":"explode","path":"x"}"#);
        assert!(result.is_err(), "unknown action must produce an error");
        // And through the step dispatcher it becomes an error result string.
        let ws = TestWorkspace::new("unknown");
        match step(ws.path(), r#"{"action":"explode","path":"x"}"#) {
            StepOutcome::Continue(msg) => assert!(msg.starts_with("ERROR:")),
            StepOutcome::Finish(_) => panic!("unknown action must not finish the loop"),
        }
    }

    #[test]
    fn action_parsing_unparseable_json_is_an_error_result() {
        let ws = TestWorkspace::new("garbage");
        match step(ws.path(), "I don't know what to do") {
            StepOutcome::Continue(msg) => assert!(msg.starts_with("ERROR:")),
            StepOutcome::Finish(_) => panic!("garbage must not finish the loop"),
        }
    }

    // ── Test-output helper ──────────────────────────────────────────────────

    #[test]
    fn test_failure_output_puts_error_lines_before_the_tail() {
        let mut output = String::from("error[E0308]: mismatched types\n  --> afana/src/lib.rs:10:5\n");
        // Fill with enough progress noise to exceed the tail cap.
        for i in 0..500 {
            output.push_str(&format!("   Compiling crate number {i} ...\n"));
        }
        output.push_str("error: aborting due to previous error\n");
        assert!(output.len() > TEST_TAIL_CAP);

        let formatted = format_test_failure(&output);

        let first_error = formatted
            .find("error[E0308]: mismatched types")
            .expect("first error line must be present");
        let tail_marker = formatted
            .find("Output tail")
            .expect("tail section must be present");
        assert!(
            first_error < tail_marker,
            "error lines must appear BEFORE the tail"
        );
        // The noise that pushed the real error out of the tail window must
        // still not have displaced it from the report.
        assert!(formatted.contains("mismatched types"));
    }

    // ── Whole-file collection ───────────────────────────────────────────────

    #[test]
    fn collect_changed_files_includes_modified_and_added_files() {
        let ws = TestWorkspace::new("collect");
        // Initialise a git repo with one committed file.
        std::fs::write(ws.path().join("existing.rs"), "fn old() {}\n").expect("write");
        let init = Command::new("git")
            .args(["init", "-q"])
            .current_dir(ws.path())
            .output()
            .expect("git init runs");
        assert!(init.status.success());
        let add = Command::new("git")
            .args(["add", "."])
            .current_dir(ws.path())
            .output()
            .expect("git add runs");
        assert!(add.status.success());
        let commit = Command::new("git")
            .args([
                "-c",
                "user.email=test@test",
                "-c",
                "user.name=test",
                "commit",
                "-q",
                "-m",
                "init",
            ])
            .current_dir(ws.path())
            .output()
            .expect("git commit runs");
        assert!(commit.status.success());

        // Modify the committed file and add a new one.
        std::fs::write(ws.path().join("existing.rs"), "fn new() {}\n").expect("modify");
        std::fs::write(ws.path().join("added.rs"), "fn added() {}\n").expect("add");

        let files = collect_changed_files(ws.path()).expect("collection succeeds");
        assert_eq!(
            files.get("existing.rs").map(String::as_str),
            Some("fn new() {}\n"),
            "modified file must appear with FULL current content"
        );
        assert_eq!(
            files.get("added.rs").map(String::as_str),
            Some("fn added() {}\n"),
            "added (untracked) file must appear with full content"
        );
    }

    // ── Read/write/list behaviour ───────────────────────────────────────────

    #[test]
    fn read_action_prefixes_line_numbers() {
        let ws = TestWorkspace::new("read");
        std::fs::write(ws.path().join("f.rs"), "a\nb\nc\n").expect("write");
        let out = read_action(ws.path(), "f.rs");
        assert!(out.contains("1\ta"));
        assert!(out.contains("2\tb"));
        assert!(out.contains("3\tc"));
    }

    #[test]
    fn write_action_creates_parent_directories() {
        let ws = TestWorkspace::new("write");
        let out = write_action(ws.path(), "deep/nested/new.rs", "fn x() {}\n");
        assert!(out.starts_with("OK:"));
        let content = std::fs::read_to_string(ws.path().join("deep/nested/new.rs"))
            .expect("file exists after write");
        assert_eq!(content, "fn x() {}\n");
    }

    #[test]
    fn list_action_lists_entries() {
        let ws = TestWorkspace::new("list");
        std::fs::create_dir_all(ws.path().join("sub")).expect("mkdir");
        std::fs::write(ws.path().join("file.rs"), "").expect("write");
        let out = list_action(ws.path(), ".");
        assert!(out.contains("file.rs"));
        assert!(out.contains("sub/"));
    }

    // ── Loop behaviour with injected model ──────────────────────────────────

    fn call_result(content: &str, latency_ms: u64) -> crate::provider::CallResult {
        crate::provider::CallResult {
            content: content.to_string(),
            latency_ms,
            http_status: 200,
            retries: 0,
            model_verified: None,
            served_model: None,
            input_len: 0,
        }
    }

    #[tokio::test]
    async fn loop_runs_until_finish_and_sums_latency() {
        let ws = TestWorkspace::new("loop-finish");
        std::fs::write(ws.path().join("f.rs"), "old\n").expect("write");

        let mut calls: u32 = 0;
        let outcome = run_agent_loop(ws.path(), "ISSUE", move |_transcript| {
            calls += 1;
            let content = match calls {
                1 => r#"{"action":"write","path":"f.rs","content":"new\n"}"#.to_string(),
                _ => r#"{"action":"finish","summary":"rewrote f.rs"}"#.to_string(),
            };
            async move { Ok(call_result(&content, 100)) }
        })
        .await
        .expect("loop succeeds");

        assert_eq!(outcome.summary.as_deref(), Some("rewrote f.rs"));
        assert_eq!(outcome.steps, 2);
        assert_eq!(outcome.total_latency_ms, 200);
        assert_eq!(outcome.last_call.latency_ms, 100);
        assert!(outcome.transcript.contains("ISSUE"));
        assert!(outcome.transcript.contains("rewrote f.rs"));
        let content = std::fs::read_to_string(ws.path().join("f.rs")).expect("read");
        assert_eq!(content, "new\n");
    }

    /// Issue #1118: the agent wrote 146 working lines, then the provider
    /// stalled on a later step and the whole session was discarded. The edits
    /// were on disk the entire time. A provider dying after good work is not a
    /// reason to throw the work away.
    #[tokio::test]
    async fn loop_keeps_its_edits_when_the_model_call_fails() {
        let ws = TestWorkspace::new("loop-call-err");
        std::fs::write(ws.path().join("f.rs"), "old\n").expect("write");

        let mut calls: u32 = 0;
        let outcome = run_agent_loop(ws.path(), "ISSUE", move |_transcript| {
            calls += 1;
            let step: Result<crate::provider::CallResult> = match calls {
                1 => Ok(call_result(
                    r#"{"action":"write","path":"f.rs","content":"good edit\n"}"#,
                    100,
                )),
                _ => Err(anyhow!("Provider openrouter did not respond within 540s")),
            };
            async move { step }
        })
        .await
        .expect("a dead provider must not fail the whole session");

        assert_eq!(
            std::fs::read_to_string(ws.path().join("f.rs")).expect("read"),
            "good edit\n",
            "the edit made before the failure must survive"
        );
        assert_eq!(outcome.steps, 1, "the failed call bought nothing");
        let summary = outcome.summary.unwrap_or_default();
        assert!(
            summary.contains("partial") && summary.contains("540s"),
            "the summary must say the work is partial and why: {summary}"
        );
    }

    /// The other half: if the very first call fails there is nothing to keep,
    /// and the session should report that rather than an empty success.
    #[tokio::test]
    async fn loop_fails_when_no_call_ever_succeeds() {
        let ws = TestWorkspace::new("loop-all-err");
        let result = run_agent_loop(ws.path(), "ISSUE", move |_transcript| async {
            Err(anyhow!("provider down"))
        })
        .await;

        // Matched rather than `expect_err`, which would need AgentOutcome: Debug.
        match result {
            Ok(_) => panic!("no successful call means there is no session to return"),
            Err(e) => assert!(
                e.to_string().contains("provider down"),
                "the cause must survive into the error: {e}"
            ),
        }
    }

    #[tokio::test]
    async fn loop_bad_response_consumes_a_step_without_aborting() {
        let ws = TestWorkspace::new("loop-bad");
        let mut calls: u32 = 0;
        let outcome = run_agent_loop(ws.path(), "ISSUE", move |_transcript| {
            calls += 1;
            let content = match calls {
                1 => "total garbage, not json".to_string(),
                _ => r#"{"action":"finish","summary":"recovered"}"#.to_string(),
            };
            async move { Ok(call_result(&content, 10)) }
        })
        .await
        .expect("loop succeeds despite one bad response");

        assert_eq!(outcome.summary.as_deref(), Some("recovered"));
        assert_eq!(outcome.steps, 2);
        assert!(outcome.transcript.contains("ERROR:"));
    }

    #[tokio::test]
    async fn loop_exhausts_step_budget() {
        let ws = TestWorkspace::new("loop-budget");
        let outcome = run_agent_loop(ws.path(), "ISSUE", move |_transcript| async move {
            Ok(call_result(r#"{"action":"list","path":"."}"#, 5))
        })
        .await
        .expect("loop returns after budget exhaustion");

        assert_eq!(outcome.steps, MAX_STEPS);
        assert_eq!(outcome.summary, None, "no finish summary when budget runs out");
        assert_eq!(outcome.total_latency_ms, 5 * MAX_STEPS as u64);
    }

    // ── Dry-run ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn dry_run_makes_no_model_call_and_no_filesystem_change() {
        // pick_model needs at least one eligible provider: groq entries carry
        // the b1_solver role and a cloud provider is "live" with just a key.
        // The guard is scoped to the env setup so it is not held across the
        // await below (clippy::await_holding_lock).
        {
            let _guard = crate::availability::TEST_LOCK
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            crate::config::init_rotation();
            std::env::set_var("GROQ_API_KEY", "test-key");
        }

        let github = GitHubClient::new("token".to_string(), "owner/repo".to_string());
        let labels: Vec<String> = vec![];
        let counts: HashMap<String, u32> = HashMap::new();

        let before = std::fs::read_dir("/tmp")
            .expect("read /tmp")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("senate-agent-"))
            .count();

        let (result, _entry, call, context) = solve_agentic(
            &github,
            9999,
            "dry run issue",
            "body",
            &labels,
            &[],
            &counts,
            None,
            true,
        )
        .await
        .expect("dry-run solve succeeds");

        assert_eq!(result.reasoning, "Dry-run placeholder reasoning.");
        assert!(result.edits.is_empty());
        assert!(result.new_files.is_empty());
        assert_eq!(call.content, "dry-run", "no real model call happened");
        assert_eq!(call.latency_ms, 0);

        let after = std::fs::read_dir("/tmp")
            .expect("read /tmp")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("senate-agent-"))
            .count();
        assert_eq!(before, after, "dry-run must not create a checkout");
        let _ = context;
    }
}
