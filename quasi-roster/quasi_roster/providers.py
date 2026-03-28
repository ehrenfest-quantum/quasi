"""Query provider /v1/models endpoints to list available models."""

import os

import requests

# Provider configs: (env_var, models_url, is_openai_format)
PROVIDER_CONFIGS = {
    "openrouter": {
        "env_var": "OPENROUTER_API_KEY",
        "url": "https://openrouter.ai/api/v1/models",
    },
    "groq": {
        "env_var": "GROQ_API_KEY",
        "url": "https://api.groq.com/openai/v1/models",
        "extra_headers": {"User-Agent": "quasi-agent/1.0 (https://quasi.arvak.io)"},
    },
    "together": {
        "env_var": "TOGETHER_API_KEY",
        "url": "https://api.together.xyz/v1/models",
    },
    "deepinfra": {
        "env_var": "DEEPINFRA_API_KEY",
        "url": "https://api.deepinfra.com/v1/openai/models",
    },
    "mistral": {
        "env_var": "MISTRAL_API_KEY",
        "url": "https://api.mistral.ai/v1/models",
    },
    "fireworks": {
        "env_var": "FIREWORKS_API_KEY",
        "url": "https://api.fireworks.ai/inference/v1/models",
    },
    "cerebras": {
        "env_var": "CEREBRAS_API_KEY",
        "url": "https://api.cerebras.ai/v1/models",
    },
    "huggingface": {
        "env_var": "HF_TOKEN",
        "url": "https://huggingface.co/api/models?pipeline_tag=text-generation&sort=downloads&limit=100",
        "custom_parser": True,
    },
}

# Providers with no listing API
SKIP_PROVIDERS = {"sarvam", "swissai"}


def list_models(provider_name):
    """Fetch model list from a provider. Returns list of model ID strings."""
    if provider_name in SKIP_PROVIDERS:
        return []

    config = PROVIDER_CONFIGS.get(provider_name)
    if config is None:
        return []

    api_key = os.environ.get(config["env_var"], "")
    if not api_key:
        print(f"  [skip] {provider_name}: no {config['env_var']} set")
        return []

    headers = {"Authorization": f"Bearer {api_key}"}
    headers.update(config.get("extra_headers", {}))

    try:
        resp = requests.get(config["url"], headers=headers, timeout=30)
        resp.raise_for_status()
        data = resp.json()
    except Exception as e:
        print(f"  [error] {provider_name}: {e}")
        return []

    if config.get("custom_parser"):
        return _parse_huggingface(data)

    # Standard OpenAI format: { "data": [{ "id": "..." }, ...] }
    # Some providers (Together) return a bare list instead.
    if isinstance(data, list):
        items = data
    else:
        items = data.get("data", [])
    return [item["id"] for item in items if isinstance(item, dict) and "id" in item]


def _parse_huggingface(data):
    """HuggingFace returns a different shape — list of model objects."""
    models = []
    if isinstance(data, list):
        for item in data:
            model_id = item.get("id") or item.get("modelId")
            if model_id:
                models.append(model_id)
    return models


def list_all_providers():
    """Return list of provider names that have API keys configured."""
    available = []
    for name, config in PROVIDER_CONFIGS.items():
        key = os.environ.get(config["env_var"], "")
        if key:
            available.append(name)
    return available
