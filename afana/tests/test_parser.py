"""Tests for afana.parser — Ehrenfest .ef text parser."""

import pytest
from afana.parser import (
    EhrenfestAST, Gate, Measure, Expect, VariationalLoop,
    ParseError, parse, parse_file,
)


# ── Helpers ───────────────────────────────────────────────────────────────────

def _src(*lines: str) -> str:
    return "\n".join(lines)


# ── Basic structure ───────────────────────────────────────────────────────────

def test_parse_minimal():
    src = _src('program "minimal"', "qubits 1")
    ast = parse(src)
    assert ast.name == "minimal"
    assert ast.n_qubits == 1
    assert ast.prepare is None
    assert ast.gates == []
    assert ast.measures == []
    assert ast.expects == []
    assert ast.variational_loops == []


def test_parse_bell():
    """Acceptance criterion: parse examples/bell.ef into a valid AST."""
    src = _src(
        '// Bell state',
        'program "bell"',
        "qubits 2",
        "prepare basis |00>",
        "h q0",
        "cnot q0 q1",
        "measure q0 -> c0",
        "measure q1 -> c1",
        'expect state "(|00> + |11>) / sqrt(2)"',
        'expect counts "00,11"',
    )
    ast = parse(src)
    assert isinstance(ast, EhrenfestAST)
    assert ast.name == "bell"
    assert ast.n_qubits == 2
    assert ast.prepare == "|00>"
    assert ast.gates == [
        Gate(name="h", qubits=[0]),
        Gate(name="cx", qubits=[0, 1]),  # cnot normalised to cx
    ]
    assert ast.measures == [Measure(qubit=0, cbit=0), Measure(qubit=1, cbit=1)]
    assert ast.expects == [
        Expect(kind="state", value="(|00> + |11>) / sqrt(2)"),
        Expect(kind="counts", value="00,11"),
    ]


def test_parse_ghz():
    src = _src(
        'program "ghz"',
        "qubits 3",
        "prepare basis |000>",
        "h q0",
        "cnot q0 q1",
        "cnot q0 q2",
        "measure q0 -> c0",
        "measure q1 -> c1",
        "measure q2 -> c2",
    )
    ast = parse(src)
    assert ast.n_qubits == 3
    assert len(ast.gates) == 3
    assert ast.gates[0] == Gate(name="h", qubits=[0])
    assert ast.gates[1] == Gate(name="cx", qubits=[0, 1])
    assert ast.gates[2] == Gate(name="cx", qubits=[0, 2])
    assert len(ast.measures) == 3


# ── Gate coverage ─────────────────────────────────────────────────────────────

def test_single_qubit_gates():
    src = _src('program "p"', "qubits 2", "x q0", "y q1", "z q0", "s q1", "t q0")
    ast = parse(src)
    names = [g.name for g in ast.gates]
    assert names == ["x", "y", "z", "s", "t"]


def test_two_qubit_gates():
    src = _src('program "p"', "qubits 2", "cx q0 q1", "cz q0 q1", "swap q0 q1")
    ast = parse(src)
    assert [g.name for g in ast.gates] == ["cx", "cz", "swap"]


def test_toffoli_alias():
    src = _src('program "p"', "qubits 3", "ccx q0 q1 q2", "toffoli q2 q1 q0")
    ast = parse(src)
    assert all(g.name == "ccx" for g in ast.gates)
    assert ast.gates[0].qubits == [0, 1, 2]
    assert ast.gates[1].qubits == [2, 1, 0]


def test_rotation_gate_parses_angle():
    src = _src('program "p"', "qubits 1", "rx 1.5708 q0")
    ast = parse(src)
    assert ast.gates[0].name == "rx"
    assert abs(ast.gates[0].params[0] - 1.5708) < 1e-9
    assert ast.gates[0].qubits == [0]


def test_sdg_tdg_gates():
    src = _src('program "p"', "qubits 1", "sdg q0", "tdg q0")
    ast = parse(src)
    assert [g.name for g in ast.gates] == ["sdg", "tdg"]


# ── Comments ──────────────────────────────────────────────────────────────────

def test_inline_comments_stripped():
    src = _src(
        'program "p"  // the name',
        "qubits 2     // two qubits",
        "h q0         // Hadamard",
    )
    ast = parse(src)
    assert ast.name == "p"
    assert ast.n_qubits == 2
    assert ast.gates == [Gate(name="h", qubits=[0])]


def test_full_line_comment_ignored():
    src = _src(
        "// This is a comment",
        'program "p"',
        "// Another comment",
        "qubits 1",
    )
    ast = parse(src)
    assert ast.name == "p"


# ── Error cases ───────────────────────────────────────────────────────────────

def test_missing_program_header():
    with pytest.raises(ParseError, match="expected 'program'"):
        parse("qubits 2")


def test_missing_qubits():
    with pytest.raises(ParseError, match="qubits"):
        parse('program "p"')


def test_zero_qubits_rejected():
    with pytest.raises(ParseError, match="qubit count must be"):
        parse(_src('program "p"', "qubits 0"))


def test_qubit_out_of_range():
    with pytest.raises(ParseError, match="out of range"):
        parse(_src('program "p"', "qubits 2", "h q5"))


def test_measure_out_of_range():
    with pytest.raises(ParseError, match="out of range"):
        parse(_src('program "p"', "qubits 2", "measure q9 -> c0"))


def test_invalid_directive():
    with pytest.raises(ParseError, match="unknown directive"):
        parse(_src('program "p"', "qubits 1", "foo q0"))


def test_measure_missing_arrow():
    with pytest.raises(ParseError, match="measure.*syntax"):
        parse(_src('program "p"', "qubits 1", "measure q0 c0"))


def test_gate_missing_qubit():
    with pytest.raises(ParseError, match="requires at least one qubit"):
        parse(_src('program "p"', "qubits 1", "h"))


def test_expect_invalid_kind():
    with pytest.raises(ParseError, match="kind must be"):
        parse(_src('program "p"', "qubits 1", 'expect probability "0.5"'))


def test_expect_relation():
    src = _src(
        'program "teleport"',
        "qubits 3",
        'expect relation "state(q2) == initial_state(q0)"',
    )
    ast = parse(src)
    assert ast.expects[0].kind == "relation"


def test_conditional_gate():
    src = _src(
        'program "teleport"',
        "qubits 3",
        "measure q0 -> c0",
        "if c0 == 1: x q2",
    )
    ast = parse(src)
    assert len(ast.conditionals) == 1
    cg = ast.conditionals[0]
    assert cg.cbit == 0
    assert cg.cbit_value == 1
    assert cg.gate.name == "x"
    assert cg.gate.qubits == [2]


def test_variational_loop_parses_into_ast():
    src = _src(
        'program "vqe"',
        "qubits 2",
        "vary theta_0 from 0.0 to 3.14 step 0.1",
        "rz theta_0 q0",
        "cx q0 q1",
        "endvary",
    )
    ast = parse(src)
    assert ast.variational_loops == [
        VariationalLoop(
            parameter="theta_0",
            start=0.0,
            stop=3.14,
            step=0.1,
            gates=[
                Gate(name="rz", qubits=[0], params=["theta_0"]),
                Gate(name="cx", qubits=[0, 1]),
            ],
        )
    ]


def test_empty_source():
    with pytest.raises(ParseError, match="unexpected end of file"):
        parse("")


# ── File loading ──────────────────────────────────────────────────────────────

def test_parse_file_bell(tmp_path):
    p = tmp_path / "bell.ef"
    p.write_text(
        'program "bell"\nqubits 2\nh q0\ncnot q0 q1\n',
        encoding="utf-8",
    )
    ast = parse_file(p)
    assert ast.name == "bell"
    assert ast.n_qubits == 2
    assert len(ast.gates) == 2


def test_parse_file_examples(tmp_path):
    """Parse all bundled .ef examples without error."""
    import pathlib
    examples_dir = pathlib.Path(__file__).parent.parent.parent / "examples"
    ef_files = list(examples_dir.glob("*.ef"))
    assert ef_files, "No .ef example files found"
    for ef in ef_files:
        ast = parse_file(ef)
        assert ast.n_qubits >= 1
        assert ast.name  # non-empty
