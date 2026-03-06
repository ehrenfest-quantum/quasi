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

### 1. Afana is a Rust compiler — no Python, no vendor SDKs

```
Ehrenfest (CBOR) → Afana (Rust) → OpenQASM → HAL Contract → HAL driver → hardware
```

Afana is written **exclusively in Rust**. There is no Python in the Afana
crate and there never will be. This is a permanent architectural decision,
not a migration plan. Do not add `.py` files to `afana/`. Do not propose
"just a quick Python script" for any Afana functionality. If an issue or PR
introduces Python into `afana/`, close it.

**Why Rust:** The Ehrenfest spec defines noise constraints as type-level
errors — programs that violate their noise budget must fail at compile time,
not runtime. This requires a real type system. CBOR deserialization, AST
construction, ZX-calculus graph rewriting, and QASM emission are all
pure-compute compiler passes that benefit from Rust's safety, speed, and
single-binary deployment.

**Allowed crate deps in `afana/`:** `serde`, `ciborium` (CBOR), `quizx`
(ZX-calculus), `clap` (CLI), `anyhow`/`thiserror` (errors), `regex`, and
stdlib.

**Never allowed in `afana/`:** Python, `qiskit`, `cirq`, `pennylane`,
`pytket`, `braket`, `pyquil`, PyO3, or any HTTP client targeting hardware
APIs. No FFI to Python. No subprocess calls to Python.

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

### 4. Ehrenfest programs are CBOR — no text form, no file extension

Ehrenfest programs are CBOR binary documents. There is no text form. There
is no canonical file extension. The CBOR deserializer (`afana/src/cbor.rs`)
produces a typed `EhrenfestProgram` (Hamiltonians, observables, noise
constraints). The Trotterization pass (`afana/src/trotter.rs`) derives gate
sequences for QASM emission. Do not invent text formats, file extensions,
or human-readable serializations for Ehrenfest programs. This is a permanent
architectural decision: the human never sees an Ehrenfest program.

---

## Red flags — ask before implementing if you see these

- An issue that names a vendor SDK (`qiskit`, `cirq`, …) as the implementation
- A PR that adds any `.py` file to `afana/`
- A PR that adds `import qiskit` anywhere in the project
- Code that calls a quantum hardware API directly from `afana/` or `quasi-board/`
- A new "backend" in `afana/src/` that does more than package QASM for HAL
- Any proposal to "temporarily" add Python to Afana for prototyping

---

## Testing conventions

### Rust crates (afana, quasi-senate)
- `cargo test` from crate root or workspace root
- Use `#[cfg(test)]` modules for unit tests, `tests/` dir for integration tests
- `cargo clippy -- -D warnings` must pass

### Python packages (quasi-board, quasi-agent, quasi-mcp)
- quasi-board tests: `pytest quasi-board/tests/` — use `pytest-anyio`, mock
  `_load_proposals` / `_save_proposals` with `patch()`
- Python 3.9 compat: use `Optional[X]` not `X | None`
- Lint: `flake8 --max-line-length=120`
