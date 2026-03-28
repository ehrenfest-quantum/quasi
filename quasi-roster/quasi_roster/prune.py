"""Prune: quarantine failing or delisted models."""

from . import providers, toml_io, db


def run_prune(toml_path=None, threshold=3, apply=False):
    """Identify and quarantine failing/delisted models."""
    doc = toml_io.load_rotation(toml_path)
    entries = doc.get("rotation", [])

    actions = []

    with db.db_cursor() as cur:
        # 1. Check health failures
        health_failures = db.query_health_failures(cur, threshold=threshold)
        health_fail_set = {(r["model_id"], r["provider"]) for r in health_failures}

        # 2. Check telemetry errors
        telemetry_errors = db.query_telemetry_errors(cur)
        telem_warn_set = set()
        for r in telemetry_errors:
            if r["total"] < 5:
                continue
            error_rate = (r["errors"] + r["parse_fails"]) / r["total"]
            if error_rate > 0.5:
                telem_warn_set.add((r["model_id"], r["provider"]))

        # 3. Check for delisted models (in TOML but not in provider listing)
        # Skip HuggingFace — its API only returns top-N by downloads, not all models.
        provider_models = {}
        for prov in set(e["provider"] for e in entries):
            if prov in providers.SKIP_PROVIDERS or prov == "huggingface":
                continue
            models = providers.list_models(prov)
            if models:  # only flag delisted if we got a successful listing
                provider_models[prov] = set(models)

        for entry in entries:
            if entry.get("quarantined"):
                continue

            model_id = entry["id"]
            provider = entry["provider"]
            reason = None

            # Health failure check
            if (model_id, provider) in health_fail_set:
                reason = f"health: {threshold}+ consecutive failures, zero successes"

            # Delisted check
            if provider in provider_models:
                if entry["model"] not in provider_models[provider]:
                    reason = f"delisted: model '{entry['model']}' not in {provider} listing"

            if reason:
                actions.append({
                    "id": model_id,
                    "provider": provider,
                    "action": "quarantine",
                    "reason": reason,
                })

            # Warn-only for high error rate in telemetry
            if (model_id, provider) in telem_warn_set and reason is None:
                actions.append({
                    "id": model_id,
                    "provider": provider,
                    "action": "warn",
                    "reason": "telemetry: >50% error rate but some successes",
                })

    # Print results
    if not actions:
        print("No models to quarantine or warn about.")
        return []

    quarantines = [a for a in actions if a["action"] == "quarantine"]
    warnings = [a for a in actions if a["action"] == "warn"]

    if quarantines:
        print(f"\nQuarantine candidates ({len(quarantines)}):")
        print(f"  {'ID':<28} {'Provider':<14} {'Reason'}")
        print("  " + "-" * 80)
        for a in quarantines:
            print(f"  {a['id']:<28} {a['provider']:<14} {a['reason']}")

    if warnings:
        print(f"\nWarnings ({len(warnings)}):")
        for a in warnings:
            print(f"  {a['id']:<28} {a['provider']:<14} {a['reason']}")

    if apply and quarantines:
        path = toml_path or toml_io.DEFAULT_TOML_PATH
        for a in quarantines:
            toml_io.update_field(doc, a["id"], "quarantined", True)
        toml_io.save_rotation(path, doc)
        print(f"\nQuarantined {len(quarantines)} models in {path}")

        with db.db_cursor() as cur:
            for a in quarantines:
                db.record_event(
                    cur, "quarantined", a["id"], a["provider"],
                    details={"reason": a["reason"]}, applied=True,
                )

    return actions
