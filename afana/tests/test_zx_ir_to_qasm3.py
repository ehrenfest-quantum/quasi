import pytest
from afana import ast, emit

def test_identity_gate_synthesis():
    # Create an Identity gate representation in ZX-IR equivalent
    gate = ast.Gate(name=ast.GateName.X, qubits=[0], params=[])
    # Apply X gate twice to get Identity (X^2 = I)
    gates = [gate, gate]
    
    ast_program = ast.EhrenfestAst(
        name="identity_test",
        n_qubits=1,
        prepare=None,
        gates=gates,
        measures=[],
        conditionals=[],
        expects=[],
        type_decls=[],
        variational_loops=[]
    )
    
    qasm_output = emit.emit_qasm(ast_program, emit.QasmVersion.V3)
    # Identity should be optimized away
    assert "x q[0];" not in qasm_output
    assert "OPENQASM 3.0;" in qasm_output

def test_hadamard_gate_synthesis():
    # Create a Hadamard gate representation
    h_gate = ast.Gate(name=ast.GateName.H, qubits=[0], params=[])
    
    ast_program = ast.EhrenfestAst(
        name="hadamard_test",
        n_qubits=1,
        prepare=None,
        gates=[h_gate],
        measures=[],
        conditionals=[],
        expects=[],
        type_decls=[],
        variational_loops=[]
    )
    
    qasm_output = emit.emit_qasm(ast_program, emit.QasmVersion.V3)
    # Verify Hadamard gate is correctly emitted
    assert "h q[0];" in qasm_output
    assert "OPENQASM 3.0;" in qasm_output
