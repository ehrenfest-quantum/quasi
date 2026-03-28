"""Discovery: find new models on providers not yet in rotation.toml."""

import re

from . import providers, toml_io, db


# Model families relevant for chat/instruct tasks (open-weight models)
RELEVANT_FAMILIES = re.compile(
    r"(llama|qwen|mistral|deepseek|gemma|phi|command-r|hermes|"
    r"nemotron|olmo|jamba|glm|kimi|ernie|cogito|swallow|sea-lion|"
    r"granite|yi-|starcoder|codestral|wizardlm|internlm|baichuan|"
    r"devstral|dolphin|hunyuan|rnj-|longcat|ministral)",
    re.IGNORECASE,
)

# Indicators of chat/instruct capability
CHAT_INDICATORS = re.compile(
    r"(instruct|chat|it$|it-|conversational)", re.IGNORECASE
)

# Hard exclusions: not text-generation, not functional, or not open
EXCLUDE_PATTERNS = re.compile(
    r"(embed|rerank|whisper|audio|tts|e5-|"
    r"diffusion|vae|clip|encoder|decoder|"
    r"gguf|ggml|gptq|awq|exl2|fp8|fp16|bf16|int[48]|fp4|"
    r"guard|shield|moderation|safety|judge|"
    r"tiny-random|trl-internal|test|dummy|"
    r"mlx-|lmstudio-|"
    r":extended|:floor|:nitro|"
    r"qwen-image|image-to-|"
    r"lumimaid|larp-|"
    r"-base$|-base-|/base$|"
    r"-pt$)",
    re.IGNORECASE,
)

# Closed-source / proprietary models — not open weights
PROPRIETARY_EXCLUDE = re.compile(
    r"(openai/|anthropic/|google/gemini|"
    r"x-ai/grok|perplexity/|amazon/nova|"
    r"qwen/qwen-max|qwen/qwen-plus|qwen/qwen-turbo|"
    r"qwen/qwen-vl-max|qwen/qwen-vl-plus|"
    r"qwen3-max|qwen3-coder-next|qwen3-coder-plus|qwen3-coder-flash|"
    r"tongyi-deepresearch|"
    r"aion-rp-)",
    re.IGNORECASE,
)

# Vision-only models (multimodal VL models that can also do text are OK)
VISION_ONLY = re.compile(r"(vision-only|image-gen)", re.IGNORECASE)

# Minimum size filter (if inferrable from name)
# Match "7b", "70b", "1.7b", "3.7b" etc. — capture the full decimal number
SIZE_PATTERN_B = re.compile(r"(?:^|[-_/])(\d+(?:\.\d+)?)[bB]")
SIZE_PATTERN_M = re.compile(r"(\d+)[mM](?!o|i|e)")  # 270m but not "models" or "mistral"
MIN_SIZE_B = 7

# Skip duplicates: OpenRouter :free/:exacto suffix, turbo/tput suffixes
DEDUP_SUFFIXES = re.compile(r"(:free$|:exacto$|-tput$|-reference$|-lite$)", re.IGNORECASE)

# Old superseded versions we already cover or don't need
SUPERSEDED = re.compile(
    r"(llama-2-|"
    r"mistral-7b-instruct-v0\.[12]|"
    r"mixtral-8x7b-v0\.[1]|"
    r"gemma-2-|"
    r"hermes-2-|"
    r"codellama/|"
    r"solar-10|"
    r"phi-2$|phi-2-|"
    r"mistral-7b-v0\.[13]$|"
    r"texteval|salesforce/|"
    r"nim/)",
    re.IGNORECASE,
)


def is_relevant(model_id):
    """Check if a model ID looks like an open-weight chat/instruct model we'd want."""
    if EXCLUDE_PATTERNS.search(model_id):
        return False

    if PROPRIETARY_EXCLUDE.search(model_id):
        return False

    if DEDUP_SUFFIXES.search(model_id):
        return False

    if SUPERSEDED.search(model_id):
        return False

    # Exclude sub-billion parameter models (270M, 500M, etc.)
    m_match = SIZE_PATTERN_M.findall(model_id)
    if m_match:
        return False

    # Check size if inferrable (in billions)
    size_match = SIZE_PATTERN_B.findall(model_id)
    if size_match:
        max_size = max(float(s) for s in size_match)
        if max_size < MIN_SIZE_B:
            return False

    # Must match a known family OR have chat/instruct indicators
    if RELEVANT_FAMILIES.search(model_id):
        return True
    if CHAT_INDICATORS.search(model_id):
        return True

    return False


def _normalize_base(model_id):
    """Extract a normalized base model name for cross-provider dedup."""
    base = model_id.split("/")[-1].lower()
    for suffix in ["-instruct", "-it", "-chat", "-hf", "-turbo"]:
        base = base.replace(suffix, "")
    # Remove provider-specific prefixes
    for prefix in ["accounts/fireworks/models/", "accounts/cogito/models/"]:
        if model_id.lower().startswith(prefix):
            base = model_id[len(prefix):].lower()
    return base


def make_entry_id(model_id, provider_name):
    """Generate a short ID from model name + provider suffix."""
    short = model_id.split("/")[-1].lower()
    for suffix in ["-instruct", "-it", "-chat"]:
        short = short.replace(suffix, "")
    short = short[:30]
    if provider_name not in ("openrouter",):
        short = f"{short}-{provider_name[:2]}"
    return short


def run_discover(toml_path=None, target_provider=None, apply=False):
    """Discover new models across providers."""
    doc = toml_io.load_rotation(toml_path)
    existing = toml_io.get_existing_models(doc)
    existing_ids = toml_io.get_existing_ids(doc)

    # Build set of base model names already in roster (for cross-provider dedup)
    existing_entries = list(doc.get("rotation", []))
    existing_bases = set()
    for entry in existing_entries:
        existing_bases.add(_normalize_base(entry["model"]))

    candidates = []
    seen_bases = set()  # dedup within this discovery run

    provider_list = [target_provider] if target_provider else list(providers.PROVIDER_CONFIGS.keys())

    for prov in provider_list:
        print(f"\n--- {prov} ---")
        models = providers.list_models(prov)
        print(f"  Found {len(models)} models total")

        for model_id in models:
            if (prov, model_id) in existing:
                continue
            if not is_relevant(model_id):
                continue

            # Cross-provider dedup: skip if same base model already in roster
            base = _normalize_base(model_id)
            if base in existing_bases:
                continue
            # Also dedup within this run (first provider wins)
            if base in seen_bases:
                continue
            seen_bases.add(base)

            entry_id = make_entry_id(model_id, prov)
            base_id = entry_id
            counter = 2
            while entry_id in existing_ids:
                entry_id = f"{base_id}-{counter}"
                counter += 1

            candidates.append({
                "id": entry_id,
                "model": model_id,
                "provider": prov,
                "license": "Unknown",
                "origin": "Unknown",
                "roles": [],
                "quarantined": True,
            })
            existing_ids.add(entry_id)

    if not candidates:
        print("\nNo new models discovered.")
        return []

    print(f"\n{'ID':<32} {'Provider':<14} {'Model':<50}")
    print("-" * 96)
    for c in candidates:
        print(f"{c['id']:<32} {c['provider']:<14} {c['model']:<50}")
    print(f"\nTotal: {len(candidates)} new candidates")

    if apply:
        path = toml_path or toml_io.DEFAULT_TOML_PATH
        for c in candidates:
            toml_io.append_entry(doc, c)
        toml_io.save_rotation(path, doc)
        print(f"\nAppended {len(candidates)} entries to {path}")

        with db.db_cursor() as cur:
            for c in candidates:
                db.record_event(
                    cur, "discovered", c["id"], c["provider"],
                    details={"model": c["model"]}, applied=True,
                )

    return candidates
