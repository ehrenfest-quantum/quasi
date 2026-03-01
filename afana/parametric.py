"""Parametric Ehrenfest program compiler (QUASI-014).

Compiles a v0.2 Ehrenfest program dict (with ParameterRef coefficients)
to an OpenQASM 3.0 circuit that uses ``input float[64]`` declarations for
unbound variational parameters.

Supports first-order Trotterization for single- and two-qubit Pauli terms.

No external dependencies — stdlib only.
"""
from __future__ import annotations

from typing import Any, Dict, List, Optional, Tuple, Union


# ── Type aliases ─────────────────────────────────────────────────────────────

#: A coefficient is either a literal float or a ParameterRef dict.
Coefficient = Union[float, Dict[str, str]]


# ── Public API ───────────────────────────────────────────────────────────────

class ParametricCompileError(ValueError):
    """Raised when a parametric program cannot be compiled."""


def compile_parametric(
    program: Dict[str, Any],
    bindings: Optional[Dict[str, float]] = None,
) -> str:
    """Compile an Ehrenfest program dict to OpenQASM 3.0.

    Parameters
    ----------
    program:
        Parsed Ehrenfest program dict (v0.1 or v0.2).  May contain
        ``ParameterRef`` coefficients (``{"param": "theta_0"}``).
    bindings:
        Optional map of parameter name → concrete float value.  Parameters
        present in *bindings* are substituted before code generation;
        remaining parameters are emitted as ``input float[64]`` declarations.

    Returns
    -------
    str
        OpenQASM 3.0 source code.
    """
    bindings = bindings or {}
    _validate_program(program, bindings)

    n_qubits: int = program["system"]["n_qubits"]
    evolution: Dict[str, Any] = program["evolution"]
    dt_us: float = evolution["dt_us"]
    steps: int = int(evolution["steps"])

    # Collect all parameter names referenced in the Hamiltonian.
    param_names = _collect_params(program["hamiltonian"]["terms"])

    # Separate unbound params (→ input float) from bound params.
    unbound = [p for p in param_names if p not in bindings]

    lines: List[str] = [
        "OPENQASM 3.0;",
        'include "stdgates.inc";',
        "",
    ]
    if unbound:
        lines.append("// Variational parameters — bind via --param or classical optimizer")
        for p in unbound:
            lines.append(f"input float[64] {p};")
        lines.append("")

    lines.append(f"qubit[{n_qubits}] q;")
    lines.append(f"bit[{n_qubits}] c;")
    lines.append("")

    if steps > 1:
        lines.append(f"// First-order Trotter decomposition: {steps} steps × dt={dt_us} µs")
        lines.append(f"for uint i in [0:{steps - 1}] {{")
        indent = "  "
    else:
        indent = ""
        if dt_us != evolution["total_us"]:
            lines.append(f"// Single Trotter step: dt={dt_us} µs")

    # Emit one Trotter step for each Pauli term.
    for term in program["hamiltonian"]["terms"]:
        coeff = term["coefficient"]
        paulis: List[Dict[str, Any]] = term.get("paulis", [])
        gate_lines = _emit_trotter_step(coeff, paulis, dt_us, bindings)
        for gl in gate_lines:
            lines.append(indent + gl)

    if steps > 1:
        lines.append("}")
        lines.append("")

    # Measure observables.
    for obs in program.get("observables", []):
        obs_type = obs.get("type", "")
        if obs_type in ("SZ", "SX"):
            q = obs["qubit"]
            if obs_type == "SX":
                lines.append(f"h q[{q}];   // basis rotation for SX")
            lines.append(f"c[{q}] = measure q[{q}];")

    return "\n".join(lines)


def bind_parameters(
    program: Dict[str, Any],
    bindings: Dict[str, float],
) -> Dict[str, Any]:
    """Return a copy of *program* with ParameterRef coefficients replaced by
    their bound float values.

    Useful for producing a fully-concrete v0.1-compatible program from a v0.2
    parametric program.
    """
    import copy
    prog = copy.deepcopy(program)
    for term in prog["hamiltonian"]["terms"]:
        coeff = term["coefficient"]
        if isinstance(coeff, dict) and "param" in coeff:
            name = coeff["param"]
            if name in bindings:
                term["coefficient"] = bindings[name]
            else:
                raise ParametricCompileError(
                    f"Parameter {name!r} is not bound and has no default value"
                )
    if "parameters" not in prog:
        prog["parameters"] = {}
    prog["parameters"].update(bindings)
    return prog


# ── Internal helpers ─────────────────────────────────────────────────────────

def _validate_program(program: Dict[str, Any], bindings: Dict[str, float]) -> None:
    """Check structural validity and parameter consistency."""
    if "system" not in program or "n_qubits" not in program["system"]:
        raise ParametricCompileError("program must have system.n_qubits")
    if "hamiltonian" not in program or "terms" not in program["hamiltonian"]:
        raise ParametricCompileError("program must have hamiltonian.terms")
    if "evolution" not in program:
        raise ParametricCompileError("program must have evolution")

    ev = program["evolution"]
    total = ev.get("total_us", 0.0)
    steps = ev.get("steps", 1)
    dt = ev.get("dt_us", 0.0)
    if steps < 1:
        raise ParametricCompileError("evolution.steps must be >= 1")
    if abs(dt * steps - total) > 1e-9:
        raise ParametricCompileError(
            f"evolution: dt_us * steps ({dt} * {steps} = {dt * steps}) "
            f"must equal total_us ({total})"
        )

    # Check that all ParameterRef names are declared in program.parameters
    # (unless provided via bindings).
    declared = set(program.get("parameters", {}).keys())
    bound = set(bindings.keys())
    for term in program["hamiltonian"]["terms"]:
        coeff = term["coefficient"]
        if isinstance(coeff, dict) and "param" in coeff:
            name = coeff["param"]
            if name not in declared and name not in bound:
                raise ParametricCompileError(
                    f"Parameter {name!r} used in Hamiltonian is not declared "
                    "in program.parameters and is not in bindings"
                )


def _collect_params(terms: List[Dict[str, Any]]) -> List[str]:
    """Return ordered list of distinct parameter names in *terms*."""
    seen: Dict[str, int] = {}
    for term in terms:
        coeff = term["coefficient"]
        if isinstance(coeff, dict) and "param" in coeff:
            name = coeff["param"]
            if name not in seen:
                seen[name] = len(seen)
    return sorted(seen, key=lambda n: seen[n])


def _resolve_coeff(coeff: Coefficient, dt_us: float, bindings: Dict[str, float]) -> Tuple[str, bool]:
    """Return (angle_expression, is_literal) for a Trotter angle 2·coeff·dt.

    For a literal coefficient *c*, returns (str(2*c*dt), True).
    For a bound ParameterRef, returns (str(2*bound_value*dt), True).
    For an unbound ParameterRef "theta", returns ("2.0*theta*<dt>", False).
    """
    if isinstance(coeff, (int, float)):
        angle = 2.0 * float(coeff) * dt_us
        return str(angle), True
    name = coeff["param"]
    if name in bindings:
        angle = 2.0 * bindings[name] * dt_us
        return str(angle), True
    # Unbound: emit symbolic expression
    return f"2.0*{name}*{dt_us}", False


def _emit_trotter_step(
    coeff: Coefficient,
    paulis: List[Dict[str, Any]],
    dt_us: float,
    bindings: Dict[str, float],
) -> List[str]:
    """Emit QASM3 lines for e^{-i·coeff·P·dt} using standard decompositions."""
    if not paulis:
        return []  # Identity term — global phase only, no gate

    angle_expr, _ = _resolve_coeff(coeff, dt_us, bindings)

    # Single-qubit terms
    if len(paulis) == 1:
        q = paulis[0]["qubit"]
        axis = paulis[0]["axis"]
        return _single_qubit_rotation(axis, q, angle_expr)

    # Two-qubit terms — general Pauli product via CNOT-Rz-CNOT
    if len(paulis) == 2:
        return _two_qubit_rotation(paulis, angle_expr)

    # Multi-qubit: parity reduction via CNOT chain
    return _multi_qubit_rotation(paulis, angle_expr)


def _single_qubit_rotation(axis: int, qubit: int, angle: str) -> List[str]:
    q = f"q[{qubit}]"
    if axis == 0:      # I — identity
        return []
    elif axis == 3:    # Z
        return [f"rz({angle}) {q};"]
    elif axis == 1:    # X = H Z H
        return [f"h {q};", f"rz({angle}) {q};", f"h {q};"]
    elif axis == 2:    # Y = Sdg H Z H S
        return [f"sdg {q};", f"h {q};", f"rz({angle}) {q};", f"h {q};", f"s {q};"]
    return []


def _two_qubit_rotation(paulis: List[Dict[str, Any]], angle: str) -> List[str]:
    """Emit e^{-i·angle·P1⊗P2} using CNOT + single-qubit basis changes."""
    q0, a0 = paulis[0]["qubit"], paulis[0]["axis"]
    q1, a1 = paulis[1]["qubit"], paulis[1]["axis"]
    sq0, sq1 = f"q[{q0}]", f"q[{q1}]"

    lines: List[str] = []

    # Basis change into Z⊗Z
    def _to_z_basis(axis: int, qubit_str: str) -> List[str]:
        if axis == 0:
            return []
        elif axis == 1:   # X → H
            return [f"h {qubit_str};"]
        elif axis == 2:   # Y → Sdg H
            return [f"sdg {qubit_str};", f"h {qubit_str};"]
        return []  # Z — already in Z basis

    def _from_z_basis(axis: int, qubit_str: str) -> List[str]:
        if axis == 0:
            return []
        elif axis == 1:   # X → H
            return [f"h {qubit_str};"]
        elif axis == 2:   # Y → H S
            return [f"h {qubit_str};", f"s {qubit_str};"]
        return []

    lines.extend(_to_z_basis(a0, sq0))
    lines.extend(_to_z_basis(a1, sq1))
    lines.append(f"cx {sq0}, {sq1};")
    lines.append(f"rz({angle}) {sq1};")
    lines.append(f"cx {sq0}, {sq1};")
    lines.extend(_from_z_basis(a0, sq0))
    lines.extend(_from_z_basis(a1, sq1))
    return lines


def _multi_qubit_rotation(paulis: List[Dict[str, Any]], angle: str) -> List[str]:
    """Parity-ladder decomposition for N-qubit Pauli tensor products."""
    qubits = [p["qubit"] for p in paulis]
    axes = [p["axis"] for p in paulis]
    lines: List[str] = []

    # Basis change to Z
    for q, a in zip(qubits, axes):
        sq = f"q[{q}]"
        if a == 1:
            lines.append(f"h {sq};")
        elif a == 2:
            lines.append(f"sdg {sq};")
            lines.append(f"h {sq};")

    # CNOT chain: cascade parity into last qubit
    for i in range(len(qubits) - 1):
        lines.append(f"cx q[{qubits[i]}], q[{qubits[i + 1]}];")

    # Rotation on last qubit
    last = qubits[-1]
    lines.append(f"rz({angle}) q[{last}];")

    # Undo CNOT chain
    for i in reversed(range(len(qubits) - 1)):
        lines.append(f"cx q[{qubits[i]}], q[{qubits[i + 1]}];")

    # Undo basis change
    for q, a in zip(qubits, axes):
        sq = f"q[{q}]"
        if a == 1:
            lines.append(f"h {sq};")
        elif a == 2:
            lines.append(f"h {sq};")
            lines.append(f"s {sq};")

    return lines
