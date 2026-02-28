from __future__ import annotations

from typing import Callable


def _normalize_distribution(distribution: dict[str, float]) -> dict[str, float]:
    total = float(sum(distribution.values()))
    if total <= 0:
        raise ValueError("distribution must have positive weight")
    return {bitstring: value / total for bitstring, value in distribution.items()}


def _counts_to_probabilities(counts: dict[str, int]) -> dict[str, float]:
    if not counts:
        raise ValueError("counts must not be empty")
    return _normalize_distribution({bitstring: float(value) for bitstring, value in counts.items()})


def fidelity(counts: dict[str, int], ideal_distribution: dict[str, float]) -> float:
    """Return the Bhattacharyya fidelity between observed counts and ideal probabilities."""
    observed = _counts_to_probabilities(counts)
    ideal = _normalize_distribution(ideal_distribution)
    outcomes = set(observed) | set(ideal)
    return sum((observed.get(key, 0.0) * ideal.get(key, 0.0)) ** 0.5 for key in outcomes) ** 2


def mitigate_measurement_error(counts: dict[str, int], confusion_rate: float = 0.1) -> dict[str, int]:
    """Invert a symmetric confusion model to recover a cleaner counts estimate."""
    if not 0.0 <= confusion_rate < 1.0:
        raise ValueError("confusion_rate must be in [0, 1)")

    probabilities = _counts_to_probabilities(counts)
    keys = sorted(probabilities)
    n_states = len(keys)
    if n_states == 1 or confusion_rate == 0.0:
        return dict(counts)

    off_diagonal = confusion_rate / (n_states - 1)
    diagonal = 1.0 - confusion_rate
    gap = diagonal - off_diagonal
    if gap <= 0:
        raise ValueError("confusion_rate is too large for the observed state space")

    alpha = 1.0 / gap
    beta = -off_diagonal / (gap * (diagonal + (n_states - 1) * off_diagonal))
    total_prob = sum(probabilities.values())

    corrected_probs = {
        key: max(0.0, alpha * probabilities[key] + beta * total_prob)
        for key in keys
    }
    corrected_probs = _normalize_distribution(corrected_probs)

    shots = sum(counts.values())
    corrected_counts = {
        key: int(round(corrected_probs[key] * shots))
        for key in keys
    }
    delta = shots - sum(corrected_counts.values())
    corrected_counts[keys[0]] += delta
    return corrected_counts


def mitigate_randomized_compiling(samples: list[dict[str, int]]) -> dict[str, int]:
    """Average multiple compiled runs by summing counts across randomized variants."""
    if not samples:
        raise ValueError("samples must not be empty")

    combined: dict[str, int] = {}
    for sample in samples:
        for bitstring, count in sample.items():
            combined[bitstring] = combined.get(bitstring, 0) + int(count)
    return combined


def mitigate_ibm_execution(
    executor: Callable[[], dict[str, int] | list[dict[str, int]]],
    ideal_distribution: dict[str, float],
    strategy: str = "measurement",
    **kwargs,
) -> dict[str, object]:
    """Run an execution callback and apply the requested mitigation strategy."""
    raw_result = executor()

    if strategy == "measurement":
        if not isinstance(raw_result, dict):
            raise ValueError("measurement mitigation expects a single counts dictionary")
        mitigated = mitigate_measurement_error(raw_result, confusion_rate=kwargs.get("confusion_rate", 0.1))
        before = raw_result
    elif strategy == "randomized":
        if not isinstance(raw_result, list):
            raise ValueError("randomized mitigation expects a list of counts dictionaries")
        mitigated = mitigate_randomized_compiling(raw_result)
        before = raw_result[0]
    else:
        raise ValueError(f"unknown mitigation strategy: {strategy}")

    return {
        "strategy": strategy,
        "raw_counts": raw_result,
        "mitigated_counts": mitigated,
        "fidelity_before": fidelity(before, ideal_distribution),
        "fidelity_after": fidelity(mitigated, ideal_distribution),
    }
