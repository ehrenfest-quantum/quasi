from afana.backend_selector import BackendCapabilities
from afana.cli import cmd_benchmark, cmd_compile, main


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


def test_main_select_backend_prints_ranked_backends(tmp_path, monkeypatch, capsys):
    src = tmp_path / "in.qasm"
    src.write_text(QASM_IN, encoding="utf-8")

    monkeypatch.setattr(
        "afana.cli.simulator_capabilities",
        lambda n_qubits=32: BackendCapabilities(
            name="simulator",
            t1_us=float("inf"),
            t2_us=float("inf"),
            gate_fidelity=1.0,
            n_qubits=n_qubits,
        ),
    )
    monkeypatch.setattr(
        "afana.cli.ibm_backend_capabilities",
        lambda name="ibm_torino": BackendCapabilities(
            name=name,
            t1_us=210.0,
            t2_us=260.0,
            gate_fidelity=0.991,
            n_qubits=127,
        ),
    )
    monkeypatch.setattr("afana.cli.select_backends", lambda backends, req: list(backends))

    seen = {}

    def _fake_cmd_compile(path, optimize, output, backend=None):
        seen["backend"] = backend
        return 0

    monkeypatch.setattr("afana.cli.cmd_compile", _fake_cmd_compile)
    monkeypatch.setattr(
        "sys.argv",
        [
            "afana",
            "compile",
            str(src),
            "--select-backend",
            "--noise-t1-us",
            "150",
            "--noise-t2-us",
            "200",
            "--gate-fidelity-min",
            "0.98",
            "--n-qubits",
            "12",
        ],
    )

    rc = main()
    assert rc == 0
    assert seen["backend"] == "simulator"
    printed = capsys.readouterr().out
    assert "backend ranking:" in printed
    assert "* simulator:" in printed
    assert "- ibm_torino:" in printed
