from spec.tools.validate import ValidationError, validate_program


def _program() -> dict:
    return {
        "version": 2,
        "system": {"n_qubits": 2},
        "hamiltonian": {
            "terms": [
                {"coefficient": {"param": "theta_0"}, "paulis": [{"qubit": 0, "axis": 3}]},
            ],
            "constant_offset": 0.0,
        },
        "evolution": {"total_us": 1.0, "steps": 1, "dt_us": 1.0},
        "observables": [{"type": "E"}],
        "noise": {"t1_us": 100.0, "t2_us": 80.0},
        "parameters": {"theta_0": 0.0},
    }


def test_validate_program_accepts_parametric_terms():
    validate_program(_program())


def test_validate_program_rejects_unknown_parameter_ref():
    bad = _program()
    bad["hamiltonian"]["terms"][0]["coefficient"] = {"param": "theta_missing"}
    try:
        validate_program(bad)
    except ValidationError:
        return
    raise AssertionError("expected ValidationError")
