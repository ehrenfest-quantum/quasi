from afana.backends.error_mitigation import mitigate_ibm_execution


def test_measurement_error_mitigation_improves_fidelity():
    ideal = {"0": 0.85, "1": 0.15}

    report = mitigate_ibm_execution(
        executor=lambda: {"0": 710, "1": 290},
        ideal_distribution=ideal,
        strategy="measurement",
        confusion_rate=0.2,
    )

    assert report["strategy"] == "measurement"
    assert report["fidelity_after"] > report["fidelity_before"]


def test_randomized_compiling_aggregation_improves_fidelity():
    ideal = {"0": 0.5, "1": 0.5}

    report = mitigate_ibm_execution(
        executor=lambda: [
            {"0": 600, "1": 400},
            {"0": 400, "1": 600},
        ],
        ideal_distribution=ideal,
        strategy="randomized",
    )

    assert report["strategy"] == "randomized"
    assert report["mitigated_counts"] == {"0": 1000, "1": 1000}
    assert report["fidelity_after"] > report["fidelity_before"]
