import pytest
from afana.emit import emit_qasm, QasmVersion
from afana.ast import EhrenfestAst, VariationalLoop, VariationalGate, GateName

def test_unbound_parameter_rejected():
    ast = EhrenfestAst(
        name="bad_vqe",
        n_qubits=1,
        gates=Vec::new(),
        measures=Vec::new(),
        conditionals=Vec::new(),
        expects=Vec::new(),
        type_decls=Vec::new(),
        variational_loops=[VariationalLoop(
            params=["theta"],
            max_iter=50,
            body=[VariationalGate(
                name=GateName::Ry,
                qubits=[0],
                param_refs=["gamma"]  # Unbound parameter
            )]
        )]
    )
    with pytest.raises(Exception):
        emit_qasm(ast, QasmVersion::V3)