import json

from afana.cli import cmd_benchmark, cmd_compile, cmd_select_backend


QASM_IN = "\n".join(
    [
        "OPENQASM 2.0;",
        'include "qelib1.inc";',
        "qreg q[2];",
        "h q[0];",
        "cx q[0],q[1];",
    ]
)


def test_cmd_compile_writes_output(tmp_path, monkeypatch, capsys):
    src = tmp_path / "in.qasm"
    out = tmp_path / "out.qasm"
    src.write_text(QASM_IN, encoding="utf-8")

    monkeypatch.setattr(
        "afana.cli.compile_qasm",
        lambda qasm, optimize: {
            "qasm": qasm + "\n// optimized\n",
            "stats": {"gate_count_before": 2, "gate_count_after": 1},
        },
    )

    rc = cmd_compile(str(src), optimize=True, output=str(out))
    assert rc == 0
    assert "// optimized" in out.read_text(encoding="utf-8")
    printed = capsys.readouterr().out
    assert "before=2 after=1" in printed


def test_cmd_benchmark_prints_json(tmp_path, monkeypatch, capsys):
    src = tmp_path / "in.qasm"
    src.write_text(QASM_IN, encoding="utf-8")

    monkeypatch.setattr(
        "afana.cli.compile_qasm",
        lambda qasm, optimize: {
            "qasm": qasm,
            "stats": {"gate_count_before": 2, "gate_count_after": 2},
        },
    )

    rc = cmd_benchmark([str(src)], optimize=True)
    assert rc == 0
    printed = capsys.readouterr().out
    assert "before=2 after=2" in printed
    assert "\"results\"" in printed


# ── cmd_select_backend tests ──────────────────────────────────────────────────

def test_select_backend_simulator_always_passes(capsys):
    """Simulator (infinite coherence) always satisfies any noise requirement."""
    rc = cmd_select_backend(t1_min=999.0, t2_min=999.0, n_qubits_min=100,
                            fidelity_min=0.9999, backends_json=None)
    assert rc == 0
    out = capsys.readouterr().out
    assert "simulator" in out
    assert "[SELECTED]" in out


def test_select_backend_prints_requirements(capsys):
    """Output shows the noise requirements that were used."""
    cmd_select_backend(t1_min=50.0, t2_min=30.0, n_qubits_min=2,
                       fidelity_min=None, backends_json=None)
    out = capsys.readouterr().out
    assert "T1≥50.0" in out
    assert "T2≥30.0" in out


def test_select_backend_extra_from_json(tmp_path, capsys):
    """Extra backends loaded from JSON are ranked alongside the simulator."""
    backends_data = [
        {"name": "ibm_test", "t1_us": 200.0, "t2_us": 150.0, "gate_fidelity": 0.99, "n_qubits": 20},
    ]
    bf = tmp_path / "backends.json"
    bf.write_text(json.dumps(backends_data), encoding="utf-8")

    rc = cmd_select_backend(t1_min=100.0, t2_min=80.0, n_qubits_min=5,
                            fidelity_min=0.98, backends_json=str(bf))
    assert rc == 0
    out = capsys.readouterr().out
    assert "ibm_test" in out
    assert "✓" in out


def test_select_backend_fails_when_backend_below_req(tmp_path, capsys):
    """Backend that doesn't meet requirements is shown with ✗ and reason."""
    backends_data = [
        {"name": "weak_backend", "t1_us": 10.0, "t2_us": 8.0, "gate_fidelity": 0.95, "n_qubits": 5},
    ]
    bf = tmp_path / "backends.json"
    bf.write_text(json.dumps(backends_data), encoding="utf-8")

    # Require more than the weak backend can provide (simulator still passes)
    rc = cmd_select_backend(t1_min=50.0, t2_min=30.0, n_qubits_min=5,
                            fidelity_min=None, backends_json=str(bf))
    assert rc == 0   # simulator passes
    out = capsys.readouterr().out
    assert "✗ weak_backend" in out
    assert "T1=" in out
