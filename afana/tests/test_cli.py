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
