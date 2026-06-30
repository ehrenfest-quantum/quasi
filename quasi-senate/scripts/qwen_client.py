"""Hardened client for the local Qwen3-235B mlx_lm.server endpoint.

Why this module exists
----------------------
`mlx_lm.server` (0.29.x) has no built-in KV-cache cap and no built-in
prompt-size cap. On macOS 26.3 a sufficiently large single prefill can
trigger a kernel panic in `IOGPUGroupMemory.cpp:528` ("pending memory
object unexpectedly found in non pending hash") — we observed exactly
this signature on 2026-06-29.

This module is the chokepoint. All Python callers in the workspace that
talk to the local Qwen endpoint should go through `call_qwen()` here.
It enforces a hard pre-flight token estimate and refuses any request
that would push too much into a single prefill.

Cap design
----------
- Hard limit: 12,000 estimated tokens (system + user combined).
- Estimator: `len(text) / 3.5` — empirically reasonable for English+Rust
  source mixed prompts; conservative (over-estimates tokens slightly).
- Refusal returns `(None, {"error": "..."}, 0.0)` so callers see the
  miss without an exception.

The cap pairs with a server-side LaunchAgent daily-recycle (04:00 local)
that bounds long-running KV growth — see
`com.danielhinderink.mlx-qwen3.plist` on the Mac Studio.
"""
from __future__ import annotations

import json
import logging
import re
import time
import urllib.request
import urllib.error
from typing import Any

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

ENDPOINT = "http://mac-studio:8080/v1/chat/completions"
MODEL = "mlx-community/Qwen3-235B-A22B-Instruct-2507-3bit-DWQ"

#: Hard pre-flight cap on the combined system+user prompt, in estimated
#: tokens. Refusals happen before any bytes hit the network.
MAX_PROMPT_TOKENS = 12_000

#: chars-per-token used for the pre-flight estimate. 3.5 is conservative
#: for English+code; the actual Qwen tokenizer averages closer to 3.8–4.0.
CHARS_PER_TOKEN = 3.5

#: Trailer that mlx-lm 0.29.x sometimes leaks into `choices[0].message.content`.
IM_END_TRAILER = re.compile(r"<\|im_end\|>\s*$")


#: Canonical solver system prompt. Distilled from four iterations on
#: real backlog issues (#1072, #1074, #1075, #1076). Pass to `call_qwen`
#: as the `system_prompt`, or extend it with task-specific addenda.
#:
#: Lessons baked in:
#: - Plain-JSON envelope, no markdown fences (mlx-lm leaks `<|im_end|>` — handled below)
#: - "find" must be unique and verbatim (anchor unambiguity)
#: - Extend-list rule: anchor on LAST item + closing brace, never on opening
#:   header — prevents the "duplicated body" failure mode seen in passes 2/3
#:   of #1076 (enum body duplicated) and the T3 freshness_ratio eval failure
#:   (duplicate `mod tests` block).
#: - Tests go INSIDE the existing `#[cfg(test)] mod tests` block, not a new one.
#: - When a project has multiple validate-style methods at different strictness
#:   levels, strict checks belong in opt-in variants, not in the base method
#:   (extending `validate()` with strict checks broke 6 lowering tests in
#:   #1076 pass 3 before being moved to `validate_circuit_emittable()`).
#: - Permission to refuse hallucinated ACs (CLI subcommands / file formats /
#:   infrastructure that doesn't exist) — Qwen used this in #1076 to skip
#:   AC2 cleanly rather than fabricating a non-existent CLI.
DEFAULT_SOLVER_SYSTEM_PROMPT = """You are an autonomous software engineer solving a GitHub issue in a Rust workspace.

Spec-pushback: if an acceptance criterion references infrastructure that does NOT exist in the provided source files (a CLI subcommand the binary doesn't implement, a file format with no parser, a flag absent from the code), do NOT fabricate matching infrastructure. Implement the achievable ACs cleanly and list the skipped ACs in your reasoning.

Respond with ONLY a JSON object (no markdown fences):
{
  "reasoning": "what you did + which ACs you skipped and why",
  "edits": [{"file": "path", "find": "exact substring", "replace": "what it becomes"}],
  "new_files": {}
}

CRITICAL find/replace rules:

1. "find" must be an EXACT VERBATIM substring of the current file content
   that occurs EXACTLY ONCE. Preserve indentation and whitespace precisely.

2. When ADDING TO AN EXISTING LIST (enum variants, match arms, methods
   inside an impl block, tests inside a mod block), anchor on the LAST
   EXISTING ITEM + the closing brace `}` of the list. NEVER anchor only
   on the opening of the list — that duplicates the entire body. Example:

       WRONG (causes duplicate variants):
         find:    "pub enum Color {"
         replace: "pub enum Color {\\n    Red,\\n    Blue,\\n    Green,\\n}"

       RIGHT:
         find:    "    Blue,\\n}"
         replace: "    Blue,\\n    Green,\\n}"

3. When MODIFYING AN EXISTING METHOD BODY, anchor on the LAST LINE of
   the body + the closing `}` of the method.

4. Make MINIMAL edits. Keep find anchors small and replace strings small.

5. When adding tests, place them INSIDE the existing `#[cfg(test)] mod tests`
   block using rule 2. Do NOT create a new `mod tests` — that causes
   "name `tests` defined multiple times" (E0428) compile errors.

6. If the project already has multiple validate-style methods at different
   strictness levels (e.g. `validate()` + `validate_normalized()` +
   `validate_fully_fused()`), strict new checks belong in a NEW opt-in
   variant — do NOT tighten the base `validate()`. The base validator is
   typically called from many places that produce un-optimised intermediate
   graphs which legitimately violate strict constraints.
"""


def estimate_tokens(text: str) -> int:
    """Cheap pre-flight token estimate. Always rounds up."""
    return int(len(text) / CHARS_PER_TOKEN + 0.999)


def call_qwen(
    system_prompt: str,
    user_prompt: str,
    *,
    max_tokens: int = 8192,
    temperature: float = 0.1,
    timeout: int = 900,
    endpoint: str = ENDPOINT,
    model: str = MODEL,
) -> tuple[str | None, dict[str, Any], float]:
    """Call the local Qwen endpoint with pre-flight prompt-size cap.

    Returns
    -------
    (content, info, elapsed_seconds)
        content : str | None
            Generated text with the `<|im_end|>` trailer stripped, or
            ``None`` on a refusal/HTTP/network failure.
        info : dict
            On success: contains ``usage`` and ``model`` from the server.
            On refusal: contains ``error`` and ``estimated_tokens``.
            On HTTP/network failure: contains ``error``.
        elapsed_seconds : float
            Wall-clock duration of the HTTP call. ``0.0`` on pre-flight
            refusal (no call was made).
    """
    combined = (system_prompt or "") + "\n" + (user_prompt or "")
    estimated = estimate_tokens(combined)

    if estimated > MAX_PROMPT_TOKENS:
        msg = (
            f"prompt rejected: estimated {estimated} tokens exceeds hard "
            f"cap of {MAX_PROMPT_TOKENS} (chars={len(combined)})"
        )
        logger.warning(msg)
        return None, {"error": msg, "estimated_tokens": estimated}, 0.0

    payload = json.dumps({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt},
        ],
        "max_tokens": max_tokens,
        "temperature": temperature,
    }).encode("utf-8")

    req = urllib.request.Request(
        endpoint,
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )

    start = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        elapsed = time.time() - start
        body = e.read()[:500] if hasattr(e, "read") else b""
        return None, {"error": f"HTTP {e.code}: {body!r}"}, elapsed
    except (urllib.error.URLError, TimeoutError) as e:
        elapsed = time.time() - start
        return None, {"error": f"network: {e}"}, elapsed

    elapsed = time.time() - start
    raw = data["choices"][0]["message"]["content"]
    content = IM_END_TRAILER.sub("", raw).strip()
    return content, {"usage": data.get("usage", {}), "model": data.get("model")}, elapsed


def build_solver_user_prompt(
    *,
    task_title: str,
    task_body: str,
    target_file: str,
    target_file_content: str,
    extra_context: str = "",
) -> str:
    """Compose the standard solver user-prompt format.

    Pairs with `DEFAULT_SOLVER_SYSTEM_PROMPT` and the four iterations of
    lessons baked in. Use this from any caller that wants the proven shape.

    Parameters
    ----------
    task_title, task_body : str
        Issue title and body verbatim.
    target_file : str
        Path the model should reference for edits (e.g. "afana/src/zx_ir.rs").
    target_file_content : str
        Current content of the file. Included in the prompt verbatim so
        the model can produce exact-match `find` anchors.
    extra_context : str
        Optional project-convention context (e.g. "this file uses opt-in
        strict validators; see validate_normalized() and validate_fully_fused()").
    """
    extra = f"\n\nPROJECT CONTEXT:\n{extra_context}" if extra_context else ""
    return (
        f"TASK: {task_title}\n\n"
        f"{task_body}\n"
        f"{extra}\n\n"
        f"CURRENT CONTENT OF {target_file}:\n"
        f"```rust\n{target_file_content}\n```\n\n"
        f"Produce the minimal edits. Emit ONLY the JSON object."
    )


if __name__ == "__main__":
    # Smoke test — useful when verifying the endpoint is reachable.
    logging.basicConfig(level=logging.INFO)
    content, info, elapsed = call_qwen(
        "You output strict JSON.",
        'Reply with ONLY this JSON: {"ok": true}',
        max_tokens=20,
    )
    print(f"content={content!r}")
    print(f"info={info}")
    print(f"elapsed={elapsed:.1f}s")
