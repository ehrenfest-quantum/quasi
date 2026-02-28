from __future__ import annotations

from .parser import EhrenfestAST, Gate, parse_file


_SIMPLE_GATES = {"h", "x", "y", "z", "s", "t", "sdg", "tdg"}
_TWO_QUBIT = {"cx", "cz", "swap"}


def _format_param(value: float | str) -> str:
    if isinstance(value, str):
        return value
    text = f"{value:.12g}"
    return text if "." in text or "e" in text.lower() else f"{text}.0"


def _emit_gate(gate: Gate) -> list[str]:
    if gate.name in _SIMPLE_GATES:
        return [f"{gate.name} q[{gate.qubits[0]}];"]
    if gate.name in ("rx", "ry", "rz"):
        return [f"{gate.name}({_format_param(gate.params[0])}) q[{gate.qubits[0]}];"]
    if gate.name in _TWO_QUBIT:
        return [f"{gate.name} q[{gate.qubits[0]}], q[{gate.qubits[1]}];"]
    if gate.name == "ccx":
        return [f"ccx q[{gate.qubits[0]}], q[{gate.qubits[1]}], q[{gate.qubits[2]}];"]
    raise ValueError(f"unsupported gate for variational emitter: {gate.name}")


def emit_qasm3(ast: EhrenfestAST, backend: str = "ibm_torino") -> str:
    lines = ["OPENQASM 3.0;", f"// backend: {backend}", f"qubit[{ast.n_qubits}] q;"]

    for gate in ast.gates:
        lines.extend(_emit_gate(gate))

    for loop in ast.variational_loops:
        lines.append(f"float {loop.parameter} = {_format_param(loop.start)};")
        lines.append(f"while ({loop.parameter} <= {_format_param(loop.stop)}) {{")
        for gate in loop.gates:
            for emitted in _emit_gate(gate):
                lines.append(f"  {emitted}")
        lines.append(
            f"  {loop.parameter} = {loop.parameter} + {_format_param(loop.step)};"
        )
        lines.append("}")

    return "\n".join(lines) + "\n"


def compile_variational_file(path: str, backend: str = "ibm_torino") -> dict:
    ast = parse_file(path)
    qasm = emit_qasm3(ast, backend=backend)
    gate_count = 0
    for raw in qasm.splitlines():
        line = raw.strip()
        if not line or line.startswith(("OPENQASM", "//", "qubit[", "float ", "while ", "}")):
            continue
        gate_count += 1
    return {
        "qasm": qasm,
        "stats": {
            "gate_count_before": gate_count,
            "gate_count_after": gate_count,
            "optimized": False,
        },
    }
