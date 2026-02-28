from afana.cli import cmd_benchmark, cmd_compile


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


def test_cmd_compile_supports_variational_source(tmp_path, monkeypatch, capsys):
    src = tmp_path / "vqe.ef"
    src.write_text('program "vqe"\nqubits 2\n', encoding="utf-8")

    monkeypatch.setattr(
        "afana.cli.compile_variational_file",
        lambda path, backend: {
            "qasm": (
                "OPENQASM 3.0;\n"
                "float theta_0 = 0.0;\n"
                "while (theta_0 <= 1.0) {\n"
                "  rz(theta_0) q[0];\n"
                "  theta_0 = theta_0 + 0.2;\n"
                "}\n"
            ),
            "stats": {"gate_count_before": 2, "gate_count_after": 2},
        },
    )

    rc = cmd_compile(str(src), optimize=False, output=None, backend="ibm_torino")
    assert rc == 0
    printed = capsys.readouterr().out
    assert "before=2 after=2" in printed
    assert "while (theta_0 <= 1.0)" in printed
