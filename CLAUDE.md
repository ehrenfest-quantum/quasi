# CLAUDE.md — Instructions for the Coding Agent

## Autonomous work loop

When asked to work through open issues:

1. `git pull origin main` before each issue
2. Check if already done — close with comment if so
3. Implement on a focused branch, write tests, lint, push, open PR
4. Keep going until no open issues remain or explicitly stopped

Priority order:
1. `compiler` + `core` labelled issues
2. `compiler` labelled issues
3. `specification` labelled issues
4. Other open issues

---

## Architectural invariants — check before implementing anything

Read these before touching any file. If an issue or PR violates these, **close
it with an explanation rather than implementing it**.

### 1. Afana is a compiler — it never imports vendor SDKs

```
Ehrenfest → Afana → OpenQASM → HAL Contract → HAL driver → hardware
```

Afana's output is standard OpenQASM. It stops there.

**Allowed in `afana/`:** stdlib, `cbor2`, `pyzx`
**Never allowed in `afana/`:** `qiskit`, `cirq`, `pennylane`, `pytket`,
`braket`, `pyquil`, or any HTTP client targeting hardware APIs.

Hardware-native gate decomposition, topology routing, and noise-aware
qubit mapping belong in `hal-drivers/<vendor>/` — behind the HAL Contract
API. The compiler does not know about hardware.

CI enforces this via the "Compiler boundary" check. If you see a failing
`compiler-boundary` job, it is a hard architectural violation — do not
suppress it.

### 2. HAL Contract is the hardware abstraction layer

`ts-halcontract` (TypeScript) and `quasi-mcp` expose the HAL Contract API.
Hardware submission always goes through `POST /hal/jobs`. Nothing in the
compiler or quasi-board layers talks directly to IBM Quantum, IQM, AWS
Braket, etc.

### 3. quasi-board / quasi-agent own the task lifecycle

ActivityPub activities (`quasi:Propose`, `quasi:Claim`, `quasi:Complete`)
are the only way work enters and exits the system. Do not invent parallel
task submission paths.

### 4. Ehrenfest programs are CBOR — no canonical text form

The `.ef` format is binary. The parser (`afana/parser.py`) accepts `.ef`
files and produces a typed AST. There is no text serialization of Ehrenfest
programs. Do not add one.

---

## Red flags — ask before implementing if you see these

- An issue that names a vendor SDK (`qiskit`, `cirq`, …) as the implementation
- A PR that adds `import qiskit` anywhere in `afana/`
- Code that calls a quantum hardware API directly from `afana/` or `quasi-board/`
- A new "backend" in `afana/backends/` that does more than package QASM for HAL

---

## Testing conventions

- quasi-board tests: `pytest quasi-board/tests/` — use `pytest-anyio`, mock
  `_load_proposals` / `_save_proposals` with `patch()`
- afana tests: `pytest afana/tests/` — mock optional deps with `monkeypatch`
  or `pytest.importorskip()`
- Python 3.9 compat: use `Optional[X]` not `X | None`
- Lint: `flake8 --max-line-length=120`
