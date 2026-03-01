"""Ehrenfest (.ef) text parser — converts .ef source to an AST.

Grammar summary (v0.2):
  program  ::= type_decl* header stmt*
  type_decl ::= 'type' NAME '=' type_expr
  type_expr ::= NAME | '(' NAME (',' NAME)* ')'
  header   ::= 'program' STRING 'qubits' INT ('prepare' 'basis' STATE)?
  stmt     ::= gate_stmt | measure_stmt | expect_stmt | comment
  gate_stmt    ::= GATE qubit_list
  measure_stmt ::= 'measure' QUBIT '->' CBIT
  expect_stmt  ::= 'expect' ('state' | 'counts') STRING
  comment  ::= '//' REST_OF_LINE

Supported gates (case-insensitive): h, x, y, z, s, t, sdg, tdg,
  cx / cnot, cz, swap, ccx / toffoli, rx, ry, rz.

Type declarations use schema tag ``quasi.org/ast/type-alias`` (CBOR schema v0.3+).

No external dependencies — stdlib only.
"""

from __future__ import annotations

import re
import shlex
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Optional, Tuple


# ── AST nodes ─────────────────────────────────────────────────────────────────

@dataclass
class Gate:
    """A single gate application, e.g. ``h q0`` or ``cnot q0 q1``."""
    name: str          # lower-cased gate name, e.g. "h", "cnot", "rx"
    qubits: List[int]  # qubit indices, e.g. [0] or [0, 1]
    params: List[float] = field(default_factory=list)  # rotation angles


@dataclass
class Measure:
    """A measurement directive: ``measure qN -> cN``."""
    qubit: int
    cbit: int


@dataclass
class ConditionalGate:
    """A classically-conditioned gate: ``if cN == M: gate qN``."""
    cbit: int
    cbit_value: int
    gate: "Gate"


@dataclass
class Expect:
    """An assertion hint (non-executable): ``expect state "..."``."""
    kind: str   # "state", "counts", or "relation"
    value: str  # raw string, e.g. "(|00> + |11>) / sqrt(2)"


@dataclass
class TypeDecl:
    """A user-defined type alias: ``type QubitPair = (Qubit, Qubit)``.

    Serialises to CBOR schema v0.3+ using the ``quasi.org/ast/type-alias`` tag.
    The ``definition`` field holds the raw type expression exactly as written.
    """

    #: CBOR schema tag for type alias nodes (schema v0.3+).
    CBOR_TAG = "quasi.org/ast/type-alias"

    name: str            # alias name, e.g. "QubitPair"
    definition: str      # raw type expression, e.g. "(Qubit, Qubit)"

    def to_dict(self) -> dict:
        """Return a CBOR-compatible dict representation (schema v0.3+)."""
        return {"_tag": self.CBOR_TAG, "name": self.name, "definition": self.definition}


@dataclass
class EhrenfestAST:
    """Root AST node for a parsed .ef program."""
    name: str
    n_qubits: int
    prepare: Optional[str]        # basis state string, e.g. "|00>"
    gates: List[Gate]
    measures: List[Measure]
    conditionals: List[ConditionalGate]
    expects: List[Expect]
    type_decls: List["TypeDecl"] = field(default_factory=list)


# ── Tokenisation helpers ───────────────────────────────────────────────────────

_GATE_RE = re.compile(
    r"^(h|x|y|z|s|t|sdg|tdg|cx|cnot|cz|swap|ccx|toffoli|rx|ry|rz)$",
    re.IGNORECASE,
)
_QUBIT_RE = re.compile(r"^q(\d+)$", re.IGNORECASE)
_CBIT_RE = re.compile(r"^c(\d+)$", re.IGNORECASE)
_FLOAT_RE = re.compile(r"^-?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?(pi)?$", re.IGNORECASE)


def _parse_qubit(token: str, lineno: int) -> int:
    m = _QUBIT_RE.match(token)
    if not m:
        raise ParseError(f"line {lineno}: expected qubit (e.g. q0), got {token!r}")
    return int(m.group(1))


def _parse_cbit(token: str, lineno: int) -> int:
    m = _CBIT_RE.match(token)
    if not m:
        raise ParseError(f"line {lineno}: expected cbit (e.g. c0), got {token!r}")
    return int(m.group(1))


def _parse_float_param(token: str, lineno: int) -> float:
    import math
    t = token.lower()
    if t.endswith("pi"):
        prefix = t[:-2]
        factor = float(prefix) if prefix and prefix not in ("", "-", "+") else (
            -math.pi if prefix == "-" else math.pi
        )
        return factor * math.pi if prefix not in ("", "-", "+") else factor
    try:
        return float(token)
    except ValueError:
        raise ParseError(f"line {lineno}: invalid float parameter {token!r}")


def _strip_comment(line: str) -> str:
    """Remove inline // comment and return stripped content."""
    idx = line.find("//")
    return line[:idx].strip() if idx != -1 else line.strip()


# ── Parser ─────────────────────────────────────────────────────────────────────

class ParseError(ValueError):
    """Raised when .ef source cannot be parsed."""


def parse(source: str) -> EhrenfestAST:
    """Parse *source* (contents of a .ef file) into an :class:`EhrenfestAST`.

    Raises :class:`ParseError` on any syntax error.
    """
    lines = source.splitlines()
    tokens_by_line: List[Tuple[int, List[str]]] = []
    for lineno, raw in enumerate(lines, start=1):
        clean = _strip_comment(raw)
        if not clean:
            continue
        # Use shlex to handle quoted strings correctly
        try:
            toks = shlex.split(clean)
        except ValueError as exc:
            raise ParseError(f"line {lineno}: {exc}") from exc
        if toks:
            tokens_by_line.append((lineno, toks))

    it = iter(tokens_by_line)

    def _next_line(expect_what: str) -> Tuple[int, List[str]]:
        try:
            return next(it)
        except StopIteration:
            raise ParseError(f"unexpected end of file while expecting {expect_what}")

    def _parse_type_decl(lineno: int, toks: List[str]) -> TypeDecl:
        """Parse ``type NAME = TYPE_EXPR`` and return a :class:`TypeDecl`."""
        # toks[0] == "type"; toks[1] == NAME; toks[2] == "="; toks[3:] == type expr
        if len(toks) < 2:
            raise ParseError(
                f"line {lineno}: 'type' syntax: type NAME = TYPE_EXPR"
            )
        type_name = toks[1]
        if not type_name.isidentifier():
            raise ParseError(
                f"line {lineno}: 'type' name must be a valid identifier, got {type_name!r}"
            )
        if len(toks) < 3 or toks[2] != "=":
            got = toks[2] if len(toks) >= 3 else "<end of line>"
            raise ParseError(
                f"line {lineno}: 'type' declaration expects '=', got {got!r}"
            )
        # Reconstruct the definition from remaining tokens
        definition = " ".join(toks[3:])
        if not definition:
            raise ParseError(f"line {lineno}: 'type' declaration requires a type expression")
        return TypeDecl(name=type_name, definition=definition)

    # ── Pre-header: collect type declarations ─────────────────────────────────

    type_decls: List[TypeDecl] = []
    peeked: Optional[Tuple[int, List[str]]] = None

    for lineno, toks in it:
        kw = toks[0].lower()
        if kw == "type":
            type_decls.append(_parse_type_decl(lineno, toks))
        else:
            peeked = (lineno, toks)
            break

    # ── Header ────────────────────────────────────────────────────────────────

    if peeked is not None:
        lineno, toks = peeked
    else:
        lineno, toks = _next_line("'program' declaration")

    if toks[0].lower() != "program":
        raise ParseError(f"line {lineno}: expected 'program', got {toks[0]!r}")
    if len(toks) < 2:
        raise ParseError(f"line {lineno}: 'program' requires a name")
    program_name = toks[1]

    lineno, toks = _next_line("'qubits' declaration")
    if toks[0].lower() != "qubits":
        raise ParseError(f"line {lineno}: expected 'qubits', got {toks[0]!r}")
    if len(toks) < 2 or not toks[1].isdigit():
        raise ParseError(f"line {lineno}: 'qubits' requires a positive integer")
    n_qubits = int(toks[1])
    if n_qubits < 1:
        raise ParseError(f"line {lineno}: qubit count must be ≥ 1, got {n_qubits}")

    # ── Body ──────────────────────────────────────────────────────────────────

    prepare: Optional[str] = None
    gates: List[Gate] = []
    measures: List[Measure] = []
    conditionals: List[ConditionalGate] = []
    expects: List[Expect] = []

    for lineno, toks in it:
        kw = toks[0].lower()

        if kw == "prepare":
            # prepare basis |00>
            if len(toks) < 3 or toks[1].lower() != "basis":
                raise ParseError(f"line {lineno}: 'prepare' syntax: prepare basis |STATE>")
            prepare = toks[2]

        elif kw == "measure":
            # measure q0 -> c0
            if len(toks) < 4 or toks[2] != "->":
                raise ParseError(f"line {lineno}: 'measure' syntax: measure qN -> cN")
            qubit = _parse_qubit(toks[1], lineno)
            cbit = _parse_cbit(toks[3], lineno)
            if qubit >= n_qubits:
                raise ParseError(
                    f"line {lineno}: qubit q{qubit} out of range (n_qubits={n_qubits})"
                )
            measures.append(Measure(qubit=qubit, cbit=cbit))

        elif kw == "expect":
            # expect state|counts|relation "..."
            if len(toks) < 3:
                raise ParseError(
                    f"line {lineno}: 'expect' syntax: expect state|counts|relation \"...\""
                )
            kind = toks[1].lower()
            if kind not in ("state", "counts", "relation"):
                raise ParseError(
                    f"line {lineno}: 'expect' kind must be 'state', 'counts', or 'relation',"
                    f" got {toks[1]!r}"
                )
            expects.append(Expect(kind=kind, value=toks[2]))

        elif kw == "if":
            # if cN == M: gate qN ...
            # Tokens after stripping inline comment and shlex split:
            #   ["if", "c1", "==", "1:", "x", "q2"]   (colon attached to value)
            #   or ["if", "c1", "==", "1", "x", "q2"] (colon stripped by comment remover)
            if len(toks) < 5:
                raise ParseError(
                    f"line {lineno}: 'if' syntax: if cN == M: gate qN"
                )
            cbit = _parse_cbit(toks[1], lineno)
            if toks[2] != "==":
                raise ParseError(
                    f"line {lineno}: 'if' expects '==', got {toks[2]!r}"
                )
            # Strip trailing colon from the value token if present
            val_tok = toks[3].rstrip(":")
            if not val_tok.isdigit():
                raise ParseError(
                    f"line {lineno}: 'if' condition value must be an integer, got {toks[3]!r}"
                )
            cbit_value = int(val_tok)
            # The rest is the gate
            gate_toks = toks[4:]
            if not gate_toks or not _GATE_RE.match(gate_toks[0]):
                raise ParseError(
                    f"line {lineno}: 'if' body must be a gate, got {gate_toks}"
                )
            g_name = gate_toks[0].lower()
            if g_name == "cnot":
                g_name = "cx"
            if g_name == "toffoli":
                g_name = "ccx"
            g_qubits = [_parse_qubit(qt, lineno) for qt in gate_toks[1:]]
            if not g_qubits:
                raise ParseError(
                    f"line {lineno}: gate in 'if' body requires at least one qubit"
                )
            cond_gate = Gate(name=g_name, qubits=g_qubits)
            conditionals.append(ConditionalGate(cbit=cbit, cbit_value=cbit_value, gate=cond_gate))

        elif _GATE_RE.match(kw):
            gate_name = kw
            # Normalise aliases
            if gate_name == "cnot":
                gate_name = "cx"
            if gate_name == "toffoli":
                gate_name = "ccx"

            # Collect optional float params (for rx/ry/rz), then qubits
            params: List[float] = []
            qubit_tokens: List[str] = toks[1:]

            # Rotation gates carry one float param before qubits
            if gate_name in ("rx", "ry", "rz"):
                if not qubit_tokens:
                    raise ParseError(f"line {lineno}: {gate_name} requires an angle parameter")
                params.append(_parse_float_param(qubit_tokens[0], lineno))
                qubit_tokens = qubit_tokens[1:]

            qubit_indices: List[int] = []
            for qt in qubit_tokens:
                idx = _parse_qubit(qt, lineno)
                if idx >= n_qubits:
                    raise ParseError(
                        f"line {lineno}: qubit q{idx} out of range (n_qubits={n_qubits})"
                    )
                qubit_indices.append(idx)

            if not qubit_indices:
                raise ParseError(f"line {lineno}: gate {toks[0]!r} requires at least one qubit")

            gates.append(Gate(name=gate_name, qubits=qubit_indices, params=params))

        elif kw == "type":
            # type declarations may also appear inside the program body
            type_decls.append(_parse_type_decl(lineno, toks))

        else:
            raise ParseError(f"line {lineno}: unknown directive {toks[0]!r}")

    return EhrenfestAST(
        name=program_name,
        n_qubits=n_qubits,
        prepare=prepare,
        gates=gates,
        measures=measures,
        conditionals=conditionals,
        expects=expects,
        type_decls=type_decls,
    )


def parse_file(path: str | Path) -> EhrenfestAST:
    """Read *path* and parse its contents."""
    source = Path(path).read_text(encoding="utf-8")
    return parse(source)
