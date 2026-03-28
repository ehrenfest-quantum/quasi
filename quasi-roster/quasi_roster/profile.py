"""Profile: run trial prompts to assign roles to quarantined models."""

import json
import os

import requests

from . import toml_io, db

# Chat completions endpoints per provider (reuse from quasi-senate config)
PROVIDER_ENDPOINTS = {
    "openrouter": ("https://openrouter.ai/api/v1/chat/completions", "OPENROUTER_API_KEY"),
    "groq": ("https://api.groq.com/openai/v1/chat/completions", "GROQ_API_KEY"),
    "together": ("https://api.together.xyz/v1/chat/completions", "TOGETHER_API_KEY"),
    "deepinfra": ("https://api.deepinfra.com/v1/openai/chat/completions", "DEEPINFRA_API_KEY"),
    "mistral": ("https://api.mistral.ai/v1/chat/completions", "MISTRAL_API_KEY"),
    "fireworks": ("https://api.fireworks.ai/inference/v1/chat/completions", "FIREWORKS_API_KEY"),
    "cerebras": ("https://api.cerebras.ai/v1/chat/completions", "CEREBRAS_API_KEY"),
    "huggingface": ("https://router.huggingface.co/v1/chat/completions", "HF_TOKEN"),
    "sarvam": ("https://api.sarvam.ai/v1/chat/completions", "SARVAM_API_KEY"),
}

EXTRA_HEADERS = {
    "openrouter": {"HTTP-Referer": "https://quasi.arvak.io", "X-Title": "QUASI Roster Profiler"},
    "groq": {"User-Agent": "quasi-agent/1.0 (https://quasi.arvak.io)"},
    "huggingface": {"User-Agent": "quasi-agent/1.0 (https://quasi.arvak.io)"},
}


def _extract_json(text):
    """Extract JSON from text that may contain markdown fences."""
    text = text.strip()
    if "```" in text:
        start = text.find("```")
        end = text.rfind("```")
        if start != end:
            inner = text[start:end]
            inner = inner.split("\n", 1)[-1] if "\n" in inner else inner[3:]
            text = inner.strip()
    brace_start = text.find("{")
    brace_end = text.rfind("}")
    if brace_start >= 0 and brace_end > brace_start:
        text = text[brace_start:brace_end + 1]
    return json.loads(text)


def _check_council(text):
    try:
        data = _extract_json(text)
        goals = data.get("goals", [])
        if len(goals) < 3:
            return False
        return all(len(g.get("description", "")) > 50 for g in goals)
    except Exception:
        return False


def _check_drafter(text):
    try:
        data = _extract_json(text)
        criteria = data.get("acceptance_criteria", [])
        desc = data.get("description", "")
        return len(criteria) >= 2 and len(desc) > 100
    except Exception:
        return False


def _check_gate(text):
    try:
        data = _extract_json(text)
        verdict = data.get("verdict", "").lower()
        reasoning = data.get("reasoning", "")
        return verdict in ("approve", "reject") and len(reasoning) > 30
    except Exception:
        return False


def _check_solver(text):
    return "fn " in text and len(text) > 200


def _check_reviewer(text):
    try:
        data = _extract_json(text)
        verdict = data.get("verdict", "").lower()
        reasoning = data.get("reasoning", "")
        text_lower = text.lower()
        catches_bug = any(w in text_lower for w in ["subtract", "minus", "a - b", "bug", "incorrect"])
        return verdict in ("approve", "reject", "request_changes") and len(reasoning) > 30 and catches_bug
    except Exception:
        return False


# Trial prompts per role — check functions defined above
TRIALS = {
    "a1_council": {
        "prompt": (
            "You are a software architecture council. Propose 3 prioritized development goals "
            "for a quantum compiler project. Respond ONLY with JSON: "
            '{"goals": [{"rank": 1, "area": "...", "description": "..."}]}'
        ),
        "check": _check_council,
    },
    "a2_drafter": {
        "prompt": (
            "Draft a GitHub issue for implementing CBOR deserialization validation in a Rust compiler. "
            "Respond ONLY with JSON: "
            '{"title": "...", "description": "...", "acceptance_criteria": ["..."]}'
        ),
        "check": _check_drafter,
    },
    "a3_gate": {
        "prompt": (
            "Review this issue proposal and decide if it should be approved or rejected:\n"
            "Title: Add CBOR schema versioning\n"
            "Description: Implement schema version field in CBOR headers for backward compat.\n"
            "Respond ONLY with JSON: "
            '{"verdict": "approve" or "reject", "reasoning": "..."}'
        ),
        "check": _check_gate,
    },
    "b1_solver": {
        "prompt": (
            "Write a Rust function that validates a TOML configuration file has no duplicate IDs "
            "in a [[rotation]] array. Include proper error handling."
        ),
        "check": _check_solver,
    },
    "b2_reviewer": {
        "prompt": (
            "Review this Rust function for correctness:\n"
            "```rust\nfn add(a: i32, b: i32) -> i32 { a - b }\n```\n"
            "Respond ONLY with JSON: "
            '{"verdict": "approve" or "reject", "reasoning": "...", "issues": ["..."]}'
        ),
        "check": _check_reviewer,
    },
}


def _call_model(provider, model, prompt, timeout=60):
    """Send a chat completion request to a provider."""
    endpoint_info = PROVIDER_ENDPOINTS.get(provider)
    if not endpoint_info:
        return None
    url, env_var = endpoint_info
    api_key = os.environ.get(env_var, "")
    if not api_key:
        return None

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    }
    headers.update(EXTRA_HEADERS.get(provider, {}))

    payload = {
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.3,
        "max_tokens": 1000,
    }

    try:
        resp = requests.post(url, json=payload, headers=headers, timeout=timeout)
        resp.raise_for_status()
        data = resp.json()
        return data["choices"][0]["message"]["content"]
    except Exception as e:
        print(f"    Error calling {provider}/{model}: {e}")
        return None


def run_profile(toml_path=None, target_model=None, apply=False):
    """Profile quarantined models with empty roles (or a specific model)."""
    doc = toml_io.load_rotation(toml_path)
    entries = doc.get("rotation", [])

    targets = []
    for entry in entries:
        if target_model and entry["id"] != target_model:
            continue
        if entry.get("quarantined") and not entry.get("roles", []):
            targets.append(entry)
        elif target_model and entry["id"] == target_model:
            targets.append(entry)

    if not targets:
        print("No quarantined models with empty roles to profile.")
        return []

    results = []

    for entry in targets:
        model_id = entry["id"]
        model_string = entry["model"]
        provider = entry["provider"]
        print(f"\n--- Profiling {model_id} ({provider}/{model_string}) ---")

        passed_roles = []
        for role_key, trial in TRIALS.items():
            print(f"  Trial: {role_key} ... ", end="", flush=True)
            response = _call_model(provider, model_string, trial["prompt"])
            if response is None:
                print("SKIP (no response)")
                continue
            if trial["check"](response):
                print("PASS")
                passed_roles.append(role_key)
            else:
                print("FAIL")

        results.append({
            "id": model_id,
            "provider": provider,
            "roles": passed_roles,
        })

        print(f"  Assigned roles: {passed_roles or '(none)'}")

    if apply and results:
        path = toml_path or toml_io.DEFAULT_TOML_PATH
        for r in results:
            if r["roles"]:
                toml_io.update_field(doc, r["id"], "roles", r["roles"])
                toml_io.update_field(doc, r["id"], "quarantined", False)
        toml_io.save_rotation(path, doc)
        print(f"\nUpdated roles in {path}")

        with db.db_cursor() as cur:
            for r in results:
                if r["roles"]:
                    db.record_event(
                        cur, "profiled", r["id"], r["provider"],
                        details={"roles": r["roles"]}, applied=True,
                    )

    return results
