import pytest
from qasm3_reference_parser import parse
from afana.emit import emit_qasm3
from afana.trotter import trotterize
from spec.valid import load_ehrenfest_program

@pytest.mark.parametrize("program", load_ehrenfest_program())
def test_qasm3_validation(program):
    ast = trotterize(program, TrotterOrder.First)
    qasm3 = emit_qasm3(&ast).unwrap()
    parse(qasm3)  # Will raise SyntaxError if invalid