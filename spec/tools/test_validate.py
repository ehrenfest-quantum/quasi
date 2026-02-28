from __future__ import annotations

import pytest

from spec.tools.validate import ValidationError, validate_program


def _base_program() -> dict:
    return {
        "version": 1,
        "system": {"n_qubits": 2},
        "hamiltonian": {
            "terms": [
                {
                    "coefficient": 1.0,
                    "paulis": [{"qubit": 0, "axis": 3}],
                }
            ],
            "constant_offset": 0.0,
        },
        "evolution": {
            "total_us": 1.0,
            "steps": 10,
            "dt_us": 0.1,
        },
        "observables": [{"type": "SZ", "qubit": 0}],
        "noise": {
            "t1_us": 100.0,
            "t2_us": 80.0,
        },
    }


def test_validate_program_accepts_optional_noise_channels():
    program = _base_program()
    program["noise"]["channels"] = [
        {"type": 1, "qubit": 0, "p": 0.05},
        {"type": 2, "qubit": 1, "gamma": 0.1},
        {"type": 3, "qubit": 1, "gamma": 0.2},
    ]

    validate_program(program)


def test_validate_program_rejects_invalid_noise_channel_shape():
    program = _base_program()
    program["noise"]["channels"] = [
        {"type": 4, "qubit": 0, "p": 0.05},
    ]

    with pytest.raises(ValidationError):
        validate_program(program)


def test_validate_program_rejects_out_of_range_noise_channel_values():
    program = _base_program()
    program["noise"]["channels"] = [
        {"type": 2, "qubit": 0, "gamma": 1.5},
    ]

    with pytest.raises(ValidationError):
        validate_program(program)
