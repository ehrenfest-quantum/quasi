from __future__ import annotations

from pathlib import Path
from typing import Mapping


AXIS_TO_GATE = {
    1: "rx",
    2: "ry",
    3: "rz",
}


def _load_cbor2():
    try:
        import cbor2
    except ImportError as exc:  # pragma: no cover - dependency issue
        raise RuntimeError("cbor2 is required for parametric Ehrenfest compilation") from exc
    return cbor2


def load_program(path: str) -> dict:
    cbor2 = _load_cbor2()
    raw = bytes.fromhex(Path(path).read_text(encoding="utf-8").strip())
    program = cbor2.loads(raw)
    if not isinstance(program, dict):
        raise ValueError("Ehrenfest payload must decode to a map")
    return program


def parse_param_bindings(items: list[str] | None) -> dict[str, float]:
    bindings: dict[str, float] = {}
    for item in items or []:
        if "=" not in item:
            raise ValueError(f"invalid --param {item!r}; expected name=value")
        name, raw_value = item.split("=", 1)
        name = name.strip()
        if not name:
            raise ValueError(f"invalid --param {item!r}; parameter name is empty")
        try:
            bindings[name] = float(raw_value)
        except ValueError as exc:
            raise ValueError(f"invalid --param {item!r}; value must be a float") from exc
    return bindings


def _format_float(value: float) -> str:
    text = f"{value:.12g}"
    return text if "." in text or "e" in text.lower() else f"{text}.0"


def _coefficient_expr(coefficient: object, bindings: Mapping[str, float]) -> tuple[str, str | None]:
    if isinstance(coefficient, (int, float)):
        return _format_float(float(coefficient)), None
    if isinstance(coefficient, dict) and isinstance(coefficient.get("param"), str):
        name = coefficient["param"]
        if name in bindings:
            return _format_float(bindings[name]), None
        return name, name
    raise ValueError("coefficient must be a float or {'param': <name>}")


def emit_openqasm3(program: dict, bindings: Mapping[str, float] | None = None) -> str:
    bindings = dict(bindings or {})
    n_qubits = int(program["system"]["n_qubits"])
    terms = program["hamiltonian"]["terms"]
    declared = program.get("parameters", {})
    unresolved: list[str] = []
    lines = ["OPENQASM 3.0;"]

    for name in declared:
        if name not in bindings:
            unresolved.append(name)
    for name in unresolved:
        lines.append(f"input float {name};")

    lines.append(f"qubit[{n_qubits}] q;")

    for term in terms:
        paulis = term.get("paulis", [])
        if not paulis:
            continue
        angle, unresolved_name = _coefficient_expr(term["coefficient"], bindings)
        if unresolved_name and unresolved_name not in unresolved:
            lines.insert(1, f"input float {unresolved_name};")
            unresolved.append(unresolved_name)
        for op in paulis:
            gate = AXIS_TO_GATE.get(op["axis"])
            if gate is None:
                continue
            lines.append(f"{gate}({angle}) q[{op['qubit']}];")
        if len(paulis) > 1:
            for left, right in zip(paulis, paulis[1:]):
                lines.append(f"cx q[{left['qubit']}], q[{right['qubit']}];")

    return "\n".join(lines) + "\n"


def compile_ehrenfest_hex(path: str, bindings: Mapping[str, float] | None = None) -> dict:
    program = load_program(path)
    qasm = emit_openqasm3(program, bindings=bindings)
    gate_count = 0
    for raw in qasm.splitlines():
        line = raw.strip()
        if not line or line.startswith(("OPENQASM", "input", "qubit")):
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
