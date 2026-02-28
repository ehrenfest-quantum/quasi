"""Tests for afana.parser — Ehrenfest .ef text parser."""

import pytest
from afana.parser import (
    EhrenfestAST, Gate, Measure, Expect, TypeDecl,
    VariationalGate, VariationalLoop,
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


# ── Type declarations ─────────────────────────────────────────────────────────

def test_type_decl_simple():
    """A single type alias before the program header is collected."""
    src = _src(
        "type AngleRad = Float",
        'program "p"',
        "qubits 1",
    )
    ast = parse(src)
    assert len(ast.type_decls) == 1
    td = ast.type_decls[0]
    assert td.name == "AngleRad"
    assert td.definition == "Float"


def test_type_decl_tuple():
    """Tuple type expressions are preserved verbatim."""
    src = _src(
        "type QubitPair = ( Qubit Qubit )",
        'program "p"',
        "qubits 2",
    )
    ast = parse(src)
    assert len(ast.type_decls) == 1
    assert ast.type_decls[0].name == "QubitPair"
    assert ast.type_decls[0].definition == "( Qubit Qubit )"


def test_type_decl_multiple():
    """Three or more type declarations are all collected (acceptance criterion)."""
    src = _src(
        "type QubitPair = ( Qubit Qubit )",
        "type QubitTriple = ( Qubit Qubit Qubit )",
        "type AngleRad = Float",
        'program "types-demo"',
        "qubits 3",
        "h q0",
        "cnot q0 q1",
        "cnot q0 q2",
    )
    ast = parse(src)
    assert isinstance(ast, EhrenfestAST)
    assert len(ast.type_decls) == 3
    names = [td.name for td in ast.type_decls]
    assert names == ["QubitPair", "QubitTriple", "AngleRad"]


def test_type_decl_cbor_tag():
    """TypeDecl.to_dict() returns a CBOR-compatible dict with the schema v0.3+ tag."""
    td = TypeDecl(name="QubitPair", definition="( Qubit Qubit )")
    d = td.to_dict()
    assert d["_tag"] == "quasi.org/ast/type-alias"
    assert d["name"] == "QubitPair"
    assert d["definition"] == "( Qubit Qubit )"


def test_type_decl_no_decls_gives_empty_list():
    """Programs without type declarations have an empty type_decls list."""
    src = _src('program "p"', "qubits 1")
    ast = parse(src)
    assert ast.type_decls == []


def test_type_decl_in_body():
    """Type declarations inside the program body (after qubits) are also parsed."""
    src = _src(
        'program "p"',
        "qubits 1",
        "type Angle = Float",
        "h q0",
    )
    ast = parse(src)
    assert len(ast.type_decls) == 1
    assert ast.type_decls[0].name == "Angle"
    assert len(ast.gates) == 1


def test_type_decl_missing_equals():
    """Missing '=' in type declaration raises ParseError."""
    with pytest.raises(ParseError, match="'type' declaration expects '='"):
        # 4 tokens but wrong separator: "type Foo Bar Baz" -> toks[2]=="Bar" != "="
        parse(_src("type Foo Bar Baz", 'program "p"', "qubits 1"))


def test_type_decl_missing_expression():
    """Type declaration without a type expression raises ParseError."""
    with pytest.raises(ParseError, match="'type' declaration requires a type expression"):
        parse(_src("type Foo =", 'program "p"', "qubits 1"))


def test_parse_types_ef_example():
    """examples/types.ef parses to a valid TypedAST with 3 type declarations."""
    import pathlib
    types_ef = pathlib.Path(__file__).parent.parent.parent / "examples" / "types.ef"
    ast = parse_file(types_ef)
    assert isinstance(ast, EhrenfestAST)
    assert len(ast.type_decls) >= 3
    names = [td.name for td in ast.type_decls]
    assert "QubitPair" in names
    assert "QubitTriple" in names
    assert "AngleRad" in names


# ── Variational loops ─────────────────────────────────────────────────────────

def test_variational_loop_basic():
    """A simple variational block is parsed into a VariationalLoop AST node."""
    src = _src(
        'program "vqe"',
        "qubits 2",
        "variational params theta phi max_iter 50",
        "  ry theta q0",
        "  ry phi q1",
        "  cnot q0 q1",
        "end",
    )
    ast = parse(src)
    assert isinstance(ast, EhrenfestAST)
    assert len(ast.variational_loops) == 1
    vl = ast.variational_loops[0]
    assert isinstance(vl, VariationalLoop)
    assert vl.params == ["theta", "phi"]
    assert vl.max_iter == 50
    assert len(vl.body) == 3


def test_variational_loop_body_gates():
    """Body gates have correct names, qubits, and param_refs."""
    src = _src(
        'program "vqe"',
        "qubits 2",
        "variational params theta phi max_iter 100",
        "  ry theta q0",
        "  ry phi q1",
        "  cnot q0 q1",
        "end",
    )
    ast = parse(src)
    vl = ast.variational_loops[0]
    ry0, ry1, cnot = vl.body
    assert isinstance(ry0, VariationalGate)
    assert ry0.name == "ry"
    assert ry0.qubits == [0]
    assert ry0.param_refs == ["theta"]
    assert ry1.name == "ry"
    assert ry1.qubits == [1]
    assert ry1.param_refs == ["phi"]
    assert cnot.name == "cx"   # normalised
    assert cnot.qubits == [0, 1]
    assert cnot.param_refs == []


def test_variational_loop_default_max_iter():
    """max_iter defaults to 100 when not specified."""
    src = _src(
        'program "p"',
        "qubits 1",
        "variational params alpha",
        "  rx alpha q0",
        "end",
    )
    ast = parse(src)
    assert ast.variational_loops[0].max_iter == 100


def test_variational_loop_to_qasm3():
    """VariationalLoop.to_qasm3() emits valid QASM3 with input float declarations."""
    vl = VariationalLoop(
        params=["theta", "phi"],
        max_iter=100,
        body=[
            VariationalGate(name="ry", qubits=[0], param_refs=["theta"]),
            VariationalGate(name="ry", qubits=[1], param_refs=["phi"]),
            VariationalGate(name="cx", qubits=[0, 1], param_refs=[]),
        ],
    )
    qasm3 = vl.to_qasm3(n_qubits=2)
    assert "OPENQASM 3.0;" in qasm3
    assert "input float[64] theta;" in qasm3
    assert "input float[64] phi;" in qasm3
    assert "ry(theta) q[0];" in qasm3
    assert "ry(phi) q[1];" in qasm3
    assert "cx q[0], q[1];" in qasm3


def test_variational_loop_no_loops_gives_empty_list():
    """Programs without variational blocks have an empty variational_loops list."""
    src = _src('program "p"', "qubits 1", "h q0")
    ast = parse(src)
    assert ast.variational_loops == []


def test_variational_loop_and_regular_gates_coexist():
    """Variational blocks and regular gates coexist in the same program."""
    src = _src(
        'program "hybrid"',
        "qubits 2",
        "h q0",
        "variational params theta max_iter 10",
        "  rx theta q1",
        "end",
        "measure q0 -> c0",
    )
    ast = parse(src)
    assert len(ast.gates) == 1
    assert ast.gates[0].name == "h"
    assert len(ast.variational_loops) == 1
    assert len(ast.measures) == 1


def test_variational_loop_missing_end():
    """Unclosed variational block raises ParseError."""
    with pytest.raises(ParseError, match="never closed with 'end'"):
        parse(_src(
            'program "p"',
            "qubits 1",
            "variational params theta",
            "  rx theta q0",
        ))


def test_variational_loop_missing_params_keyword():
    """variational without 'params' keyword raises ParseError."""
    with pytest.raises(ParseError, match="'variational' syntax"):
        parse(_src(
            'program "p"',
            "qubits 1",
            "variational theta phi",
            "end",
        ))


def test_variational_loop_empty_params():
    """variational params with no parameter names raises ParseError."""
    with pytest.raises(ParseError, match="requires at least one parameter"):
        parse(_src(
            'program "p"',
            "qubits 1",
            "variational params",
            "  h q0",
            "end",
        ))


def test_parse_vqe_example():
    """examples/vqe.ef parses to a valid AST with one variational loop."""
    import pathlib
    vqe_ef = pathlib.Path(__file__).parent.parent.parent / "examples" / "vqe.ef"
    ast = parse_file(vqe_ef)
    assert isinstance(ast, EhrenfestAST)
    assert len(ast.variational_loops) == 1
    vl = ast.variational_loops[0]
    assert "theta" in vl.params
    assert "phi" in vl.params
