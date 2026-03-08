import pytest
from afana.emit import emit_qasm, QasmVersion
from afana.ast import EhrenfestAst, Gate, GateName

def test_swap_emission():
    ast = EhrenfestAst(
        name='swap_test',
        n_qubits=2,
        prepare=None,
        gates=[
            Gate(name=GateName.Cx, qubits=[0, 1], params=[]),
            Gate(name=GateName.Cx, qubits=[1, 0], params=[]),
            Gate(name=GateName.Cx, qubits=[0, 1], params=[]),
        ],
        measures=[],
        conditionals=[],
        expects=[],
        type_decls=[],
        variational_loops=[],
    )
    qasm = emit_qasm(&ast, QasmVersion.V3).unwrap();
    assert 'swap q[0], q[1];' in qasm.replace(' ', '').replace('\n', '');
