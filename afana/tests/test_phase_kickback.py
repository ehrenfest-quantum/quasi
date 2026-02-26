from afana.phase_kickback import phase_kickback


def test_phase_kickback_builds_core_sequence():
    def oracle(circuit, target):
        circuit.add("CX", [0, target], stage="oracle")

    c = phase_kickback(target_qubit=1, oracle=oracle, n_qubits=2)
    gates = [op.gate for op in c.operations]
    assert gates[0:2] == ["X", "H"]
    assert "CX" in gates


def test_phase_kickback_keeps_target_qubit():
    def oracle(circuit, target):
        circuit.add("Z", [target], stage="oracle")

    c = phase_kickback(target_qubit=0, oracle=oracle, n_qubits=1)
    assert c.operations[0].qubits == [0]
    assert c.operations[1].qubits == [0]
