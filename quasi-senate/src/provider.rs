// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! LLM provider dispatch and JSON repair utilities.
//!
//! `call_model` is the single entry point for all LLM calls in the Senate Loop.
//! It handles provider lookup, API key resolution, context-window truncation,
//! retry-on-429/503, and optional model-identity verification via response headers.
//!
//! `parse_json_response` is a best-effort JSON repair helper that handles the
//! common ways that LLMs produce malformed JSON (markdown fences, raw newlines
//! inside strings, JS comments, Python-style quotes, bad escape sequences).

use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::str::FromStr;
use tokio_retry::{strategy::ExponentialBackoff, Retry};
use tracing::{info, warn};

use crate::config::get_provider;
use crate::types::RotationEntry;

// ── Request body ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: &'static str,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
}

// ── Public interface ──────────────────────────────────────────────────────────

/// Rich result from a single LLM call, carrying telemetry metadata.
#[derive(Debug, Clone)]
pub struct CallResult {
    /// The text content from choices[0].message.content
    pub content: String,
    /// Wall-clock duration of the HTTP call (including retries) in milliseconds
    pub latency_ms: u64,
    /// Final HTTP status code (0 if unknown)
    pub http_status: u16,
    /// Number of retry attempts before success (0 = first attempt succeeded)
    pub retries: u32,
    /// Whether served_model matched the requested model (OpenRouter only)
    pub model_verified: Option<bool>,
    /// The x-finalized-model header value if present (OpenRouter only)
    pub served_model: Option<String>,
    /// Total prompt length in characters (system + user), for input_tokens_approx.
    pub input_len: u64,
}

/// Error type wrapping a successful HTTP call that failed JSON parsing.
///
/// Returned (as an `anyhow::Error`) when `call_model` succeeds but
/// `parse_json_response` fails. The embedded `CallResult` lets the pipeline
/// write a telemetry row with `json_parse_ok = false` before propagating.
#[derive(Debug)]
pub struct ParseFailure {
    pub call: CallResult,
    pub entry: &'static crate::types::RotationEntry,
    pub error: String,
}

impl std::fmt::Display for ParseFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON parse failure (model={}): {}", self.entry.id, self.error)
    }
}

impl std::error::Error for ParseFailure {}

/// Call an LLM via its provider using the OpenAI-compatible chat completions API.
///
/// Returns a `CallResult` with the response text and telemetry metadata.
/// When the provider sends a model-identity verification header (anti-masking
/// check), the result is compared against the requested model and logged via
/// `tracing::warn!` on mismatch.
///
/// Retries up to 3 times on HTTP 429 / 503 with exponential backoff starting
/// at 2 seconds.
pub async fn call_model(
    entry: &RotationEntry,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
    max_tokens: u32,
) -> Result<CallResult> {
    // 1. Resolve provider config.
    let provider = get_provider(entry.provider)
        .ok_or_else(|| anyhow!("Unknown provider '{}' for model '{}'", entry.provider, entry.id))?;

    // 2. Read API key from environment.
    let api_key = std::env::var(provider.env_var).with_context(|| {
        format!(
            "Environment variable '{}' is not set (required for provider '{}')",
            provider.env_var, entry.provider
        )
    })?;
    if api_key.trim().is_empty() {
        return Err(anyhow!(
            "Environment variable '{}' is empty (required for provider '{}')",
            provider.env_var,
            entry.provider
        ));
    }

    // 3. Apply context-window truncation.
    let user_prompt_truncated: String = match entry.max_context {
        Some(max_ctx) if user_prompt.len() > max_ctx as usize => {
            warn!(
                model = entry.id,
                original_len = user_prompt.len(),
                truncated_to = max_ctx,
                "Truncating user prompt to fit model context window"
            );
            user_prompt[..max_ctx as usize].to_string()
        }
        _ => user_prompt.to_string(),
    };

    let input_len = (system_prompt.len() + user_prompt_truncated.len()) as u64;

    // 4. Build the request body.
    let request_body = ChatRequest {
        model: entry.model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user",
                content: user_prompt_truncated,
            },
        ],
        max_tokens,
        temperature,
    };

    let request_json =
        serde_json::to_string(&request_body).context("Failed to serialize chat request")?;

    // 5. Build headers.
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", api_key))
            .context("Invalid API key — cannot build Authorization header")?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    for (name, value) in provider.extra_headers {
        let header_name =
            HeaderName::from_str(name).with_context(|| format!("Invalid header name: {}", name))?;
        let header_value = HeaderValue::from_str(value)
            .with_context(|| format!("Invalid header value for {}: {}", name, value))?;
        headers.insert(header_name, header_value);
    }

    // 6. Execute the HTTP call with retry.
    let url = provider.url;
    let timeout_secs = provider.timeout_secs;
    let verify_header = provider.verify_header;
    let expected_model = entry.model;
    let model_id = entry.id;

    // ExponentialBackoff::from_millis(base) yields: base, base*2, base*4, …
    // We want 2 s, 4 s, 8 s → 3 attempts max.
    let retry_strategy = ExponentialBackoff::from_millis(2000).take(3);

    let start_time = std::time::Instant::now();
    let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    let inner_result = Retry::spawn(retry_strategy, || {
        // Clone what we need for each attempt.
        let headers = headers.clone();
        let request_json = request_json.clone();
        let attempt_count = attempt_count.clone();

        async move {
            let attempt = attempt_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let _ = attempt; // used for retry tracking

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(timeout_secs))
                .build()
                .context("Failed to build HTTP client")?;

            info!(
                model = model_id,
                provider = entry.provider,
                url = url,
                "Calling LLM"
            );

            let response = client
                .post(url)
                .headers(headers)
                .body(request_json)
                .send()
                .await
                .context("HTTP request failed")?;

            let status = response.status();

            // Surface the response headers before consuming the body.
            let response_headers = response.headers().clone();

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
            {
                // Propagate as an error so tokio-retry can handle it.
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "(unreadable body)".to_string());
                return Err(anyhow!(
                    "Provider returned {} (retryable): {}",
                    status,
                    &body[..body.len().min(200)]
                ));
            }

            if !status.is_success() {
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "(unreadable body)".to_string());
                return Err(anyhow!(
                    "Provider '{}' returned {}: {}",
                    entry.provider,
                    status,
                    &body[..body.len().min(400)]
                ));
            }

            // 7. Parse the response body.
            let body_text = response
                .text()
                .await
                .context("Failed to read response body")?;

            let json: Value =
                serde_json::from_str(&body_text).context("Provider response is not valid JSON")?;

            let content = json
                .pointer("/choices/0/message/content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    anyhow!(
                        "Provider response missing choices[0].message.content: {}",
                        &body_text[..body_text.len().min(400)]
                    )
                })?
                .to_string();

            let status_code = status.as_u16();

            // 8. Verify model identity from the response JSON body.
            //
            // All OpenAI-compatible APIs return a top-level "model" field in the
            // response. We compare it against the requested model string. This
            // replaces the previous header-based check (OpenRouter dropped the
            // x-finalized-model header) and works for all providers.
            let (model_verified, served_model_val) = {
                let served = json
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                // Also check the header as fallback (some providers may still send it).
                let served = served.or_else(|| {
                    verify_header.and_then(|hdr| {
                        response_headers
                            .get(hdr)
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string())
                    })
                });
                let verified = served.as_ref().map(|s| {
                    // Match if the served model starts with or equals the requested model.
                    // OpenRouter returns expanded names like "meta-llama/llama-4-maverick-17b-128e-instruct"
                    // for a request of "meta-llama/llama-4-maverick". Groq returns the exact model string.
                    s == expected_model || s.starts_with(expected_model)
                });
                if let Some(false) = verified {
                    warn!(
                        requested_model = expected_model,
                        served_model = served.as_deref().unwrap_or("(none)"),
                        "Provider served a different model than requested"
                    );
                }
                (verified, served)
            };

            Ok::<(String, u16, Option<bool>, Option<String>), anyhow::Error>((content, status_code, model_verified, served_model_val))
        }
    })
    .await;

    let (content, http_status, model_verified, served_model) = inner_result?;
    let latency_ms = start_time.elapsed().as_millis() as u64;
    let retries = attempt_count.load(std::sync::atomic::Ordering::SeqCst).saturating_sub(1);

    info!(
        model = model_id,
        response_chars = content.len(),
        latency_ms = latency_ms,
        retries = retries,
        "LLM call complete"
    );

    Ok(CallResult {
        content,
        latency_ms,
        http_status,
        retries,
        model_verified,
        served_model,
        input_len,
    })
}

// ── JSON repair ───────────────────────────────────────────────────────────────

/// Parse a JSON response from an LLM, attempting several repair strategies
/// in order before giving up.
///
/// Handles (in order):
/// 1. Markdown code fences (` ```json … ``` `)
/// 2. Plain `serde_json` parse
/// 3. Raw newlines / tabs / carriage-returns inside JSON strings
/// 4. JS-style `//` line comments
/// 5. Python triple-quoted strings → JSON strings
/// 6. Python-style single-quote dicts → double-quote
/// 7. Invalid JSON backslash escapes (`\(`, `\p`, etc.)
/// 8. First `{` … last `}` extraction
pub fn parse_json_response<T: DeserializeOwned>(raw: &str) -> Result<T> {
    // Step 1: strip markdown code fences.
    let stripped = strip_code_fences(raw);
    let text = stripped.trim();

    // Step 2: try plain parse on the stripped text.
    if let Ok(v) = serde_json::from_str::<T>(text) {
        return Ok(v);
    }

    // Step 3: fix raw newlines/tabs/carriage-returns inside JSON strings.
    let repaired = fix_literal_newlines(text);

    if let Ok(v) = serde_json::from_str::<T>(&repaired) {
        return Ok(v);
    }

    // Step 4: strip JS-style // comments.
    let comment_re = regex::Regex::new(r"//[^\n]*").expect("valid regex");
    let no_comments = comment_re.replace_all(&repaired, "").to_string();

    if let Ok(v) = serde_json::from_str::<T>(&no_comments) {
        return Ok(v);
    }

    // Step 5: replace Python triple-quoted strings with JSON-safe equivalent.
    let triple_re =
        regex::Regex::new(r#"(?s)"""(.*?)""""#).expect("valid regex");
    let no_triple = triple_re
        .replace_all(&no_comments, |caps: &regex::Captures| {
            serde_json::to_string(&caps[1]).unwrap_or_else(|_| "\"\"".to_string())
        })
        .to_string();

    if let Ok(v) = serde_json::from_str::<T>(&no_triple) {
        return Ok(v);
    }

    // Step 6: try replacing Python-style single-quote dicts with double-quote.
    if no_triple.contains('\'') {
        let single_quote_attempt = no_triple.replace('\'', "\"");
        if let Ok(v) = serde_json::from_str::<T>(&single_quote_attempt) {
            return Ok(v);
        }
    }

    // Step 7: fix invalid JSON backslash escapes.
    // Valid JSON escape sequences after `\`: " \ / b f n r t u
    // Replace `\X` (where X is anything else) with `\\X`.
    let escape_re =
        regex::Regex::new(r#"\\([^"\\\/bfnrtu])"#).expect("valid regex");
    let fixed_escapes = escape_re
        .replace_all(&no_triple, r"\\$1")
        .to_string();

    if let Ok(v) = serde_json::from_str::<T>(&fixed_escapes) {
        return Ok(v);
    }

    // Step 8: extract first `{` … last `}` and try parsing.
    let first_brace = fixed_escapes.find('{');
    let last_brace = fixed_escapes.rfind('}');

    if let (Some(start), Some(end)) = (first_brace, last_brace) {
        if end > start {
            let extracted = &fixed_escapes[start..=end];
            if let Ok(v) = serde_json::from_str::<T>(extracted) {
                return Ok(v);
            }
        }
    }

    // All attempts exhausted.
    Err(anyhow!(
        "Failed to parse LLM response as JSON after all repair attempts.\n\
         Raw output (first 500 chars):\n{}",
        &raw[..raw.len().min(500)]
    ))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Remove ` ```json ` / ` ``` ` markdown fences that some models wrap around JSON.
///
/// Strategy: collect only lines that are inside a fenced code block.
/// If the model output has no fences, return as-is. If fence counts are odd
/// (unclosed fence), fall back to extracting everything after the first fence
/// opening line up to the last ``` occurrence.
fn strip_code_fences(s: &str) -> String {
    // Fast path — no fences present.
    if !s.contains("```") {
        return s.to_string();
    }

    let mut result: Vec<&str> = Vec::new();
    let mut in_block = false;

    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_block = !in_block;
            // Skip the fence delimiter line itself.
            continue;
        }
        if in_block {
            result.push(line);
        }
        // Lines outside any code block are discarded — they are prose, not JSON.
    }

    // If we ended up inside a block (odd number of fences / unclosed), the model
    // probably just omitted the closing fence — use everything after the first
    // fence opening up to the last ``` marker.
    if in_block {
        if let Some(start) = s.find("```") {
            let after_open = &s[start..];
            // Skip the opening fence line.
            let after_newline = after_open
                .find('\n')
                .map(|i| &after_open[i + 1..])
                .unwrap_or(after_open);
            if let Some(close) = after_newline.rfind("```") {
                return after_newline[..close].trim().to_string();
            }
            // No closing ``` at all — return everything after the opening line.
            return after_newline.trim().to_string();
        }
    }

    // If result is empty (e.g., fences existed but enclosed nothing recognisable),
    // return the original so downstream repair steps can try.
    if result.is_empty() {
        return s.to_string();
    }

    result.join("\n")
}

/// Walk the string character-by-character and escape raw `\n`, `\r`, `\t`
/// that appear inside JSON string literals.
///
/// Mirrors `_fix_literal_newlines` from `quasi-agent/solve.py`.
fn fix_literal_newlines(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 64);
    let mut in_str = false;
    let mut esc_next = false;

    for ch in s.chars() {
        if esc_next {
            result.push(ch);
            esc_next = false;
        } else if ch == '\\' && in_str {
            result.push(ch);
            esc_next = true;
        } else if ch == '"' {
            in_str = !in_str;
            result.push(ch);
        } else if in_str && ch == '\n' {
            result.push_str("\\n");
        } else if in_str && ch == '\r' {
            result.push_str("\\r");
        } else if in_str && ch == '\t' {
            result.push_str("\\t");
        } else {
            result.push(ch);
        }
    }

    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Simple {
        key: String,
        value: u32,
    }

    #[test]
    fn parse_plain_json() {
        let raw = r#"{"key": "hello", "value": 42}"#;
        let result: Simple = parse_json_response(raw).unwrap();
        assert_eq!(result.key, "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn parse_fenced_json() {
        let raw = "```json\n{\"key\": \"hello\", \"value\": 42}\n```";
        let result: Simple = parse_json_response(raw).unwrap();
        assert_eq!(result.key, "hello");
        assert_eq!(result.value, 42);
    }

    #[test]
    fn parse_raw_newline_in_string() {
        // Simulate a model that emits a raw newline inside a JSON string.
        let raw = "{\"key\": \"hello\nworld\", \"value\": 1}";
        let result: Simple = parse_json_response(raw).unwrap();
        assert!(result.key.contains("world"));
    }

    #[test]
    fn parse_js_comment() {
        let raw = "{\n  // a comment\n  \"key\": \"hi\",\n  \"value\": 7\n}";
        let result: Simple = parse_json_response(raw).unwrap();
        assert_eq!(result.key, "hi");
        assert_eq!(result.value, 7);
    }

    #[test]
    fn parse_json_with_prose_around_it() {
        let raw = "Here is your JSON:\n{\"key\": \"hello\", \"value\": 42}\nDone.";
        let result: Simple = parse_json_response(raw).unwrap();
        assert_eq!(result.key, "hello");
    }

    #[test]
    fn strip_fences_basic() {
        let raw = "```json\n{\"a\": 1}\n```";
        let stripped = strip_code_fences(raw);
        assert!(stripped.contains("{\"a\": 1}"));
        assert!(!stripped.contains("```"));
    }

    #[test]
    fn fix_literal_newlines_basic() {
        let raw = "{\"msg\": \"line1\nline2\"}";
        let fixed = fix_literal_newlines(raw);
        assert!(fixed.contains("\\n"));
        // Outside of strings, real newlines should be preserved.
        assert!(!fixed.contains("\n") || fixed.find("\\n").is_some());
    }
}
