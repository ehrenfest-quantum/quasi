import pytest
from afana.ast import Gate, GateName, EhrenfestAst
from afana.emit import emit_qasm, QasmVersion

def test_y_gate_synthesis():
    # Create an AST with a Y gate on qubit 0
    ast = EhrenfestAst(
        name="y_gate_test",
        n_qubits=1,
        prepare=None,
        gates=[
            Gate(
                name=GateName.Y,
                qubits=[0],
                params=[]
            )
        ],
        measures=[],
        conditionals=[],
        expects=[],
        type_decls=[],
        variational_loops=[]
    )
    
    # Emit QASM3
    qasm3 = emit_qasm(&ast, QasmVersion.V3)
    
    # Check that the output contains 'y q[0];'
    assert 'y q[0];' in qasm3, "Y gate not synthesized correctly in QASM3 output"
    