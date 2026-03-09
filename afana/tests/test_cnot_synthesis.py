import pytest
from afana.src.gate_synthesis import decompose_zx_to_cnot


def test_zx_spider_to_cnot_emits_correct_qasm():
    """Test that Z-spider to X-spider pairs emit correct CNOT QASM3 with proper qubit ordering."""
    # Test case 1: Simple Z-X spider connection
    result = decompose_zx_to_cnot(0, 1)
    assert result == "cx q[0], q[1];"
    
    # Test case 2: Different qubit indices
    result = decompose_zx_to_cnot(2, 3)
    assert result == "cx q[2], q[3];"
    
    # Test case 3: Same qubit (edge case)
    result = decompose_zx_to_cnot(5, 5)
    assert result == "cx q[5], q[5];"
