# Ehrenfest Language Specification

## Ehrenfest Language Overview

Ehrenfest is the QUASI quantum programming language, designed from first principles for AI agents as primary contributors. It has two complementary representations that serve different roles in the compilation pipeline.

**Physics-level (canonical form):** The authoritative representation is a CBOR-encoded binary format, specified in [`spec/ehrenfest-v0.1.cddl`](ehrenfest-v0.1.cddl). Programs express *physics* — Hamiltonians, observables, evolution time, and noise constraints — not gate sequences. The Afana compiler is responsible for deriving gate sequences via Trotterization and ZX-calculus optimization. This separation is deliberate: quantum hardware changes rapidly, but the physics of a problem does not. An Ehrenfest program targeting a 2-qubit transverse-Ising Hamiltonian will compile correctly to any backend Afana supports.

**Circuit-level (text `.ef` format):** For problems best expressed as explicit gate sequences — Bell-state preparation, Grover search, quantum teleportation — Ehrenfest provides a human-readable text format parsed by `afana.parser`. The format declares qubit count, optional initial state, gate operations (H, CNOT, CZ, Rx/Ry/Rz, etc.), measurements, and classical feed-forward conditionals. See [`examples/bell.ef`](../examples/bell.ef) for a minimal example:

```
program "bell"
qubits 2
prepare basis |00>
h q0
cnot q0 q1
measure q0 -> c0
measure q1 -> c1
expect state "(|00> + |11>) / sqrt(2)"
```

The `.ef` parser (`afana.parse` / `afana.parse_file`) converts this source to a typed `EhrenfestAST` with structured `Gate`, `Measure`, `ConditionalGate`, and `Expect` nodes. The AST feeds into Afana's OpenQASM emission stage and subsequently the ZX-calculus optimization pass (PyZX `full_reduce`), which reduces gate count by 30–50% on typical circuits.

**ZX-IR:** Between the gate-level and the hardware backend, Afana converts circuits to a ZX-calculus graph (ZX-IR). Spiders, phase gadgets, and Hadamard edges in ZX-IR correspond to measurement patterns in the measurement-based quantum computing model. The `full_reduce` simplification rewrites this graph using local rewrite rules (spider fusion, identity removal, colour change, pivot) before re-extracting an optimized circuit for hardware transpilation.

**Relation to CBOR:** The physics-level CBOR schema (`ehrenfest-v0.1.cddl`) encodes Hamiltonians as lists of Pauli terms with real coefficients in GHz·rad units. The `EvolutionTime` field specifies Trotter decomposition parameters; `NoiseConstraint` fields (`t1_us`, `t2_us`, `gate_fidelity_min`) become a type-level constraint enforced at compile time — a program that cannot execute within its noise budget is a type error, not a runtime failure.



Ehrenfest is the QUASI quantum programming language designed for AI agents to express quantum physics problems in a hardware-agnostic format. It uses CBOR binary encoding and focuses on Hamiltonians and observables rather than gate sequences.

## Syntax

Ehrenfest programs are defined using a CDDL schema with the following key components:

- `version`: Must be 1 for v0.1
- `system`: Physical context including number of qubits
- `hamiltonian`: Energy operator of the system
- `evolution`: Evolution time
- `observables`: List of measurable quantities
- `noise`: Noise constraints

## Semantics

Ehrenfest expresses quantum programs in terms of physics rather than circuits:

- Programs describe expectation values and Hamiltonians
- The compiler (Afana) derives gate sequences
- Uses natural units (GHz·rad) for energy
- Supports Pauli operators (I, X, Y, Z)

## Type System

Ehrenfest uses a strong static type system with these core types:

- `uint`: Natural numbers
- `float`: Real numbers
- `tstr`: Text strings
- `PauliAxis`: Enum {I: 0, X: 1, Y: 2, Z: 3}
- `Observable`: Union of SigmaZ, SigmaX, Energy, Density, Fidelity

## Example Programs

### 1. Simple Rabi Oscillation

```cbor
{ "version": 1, "system": { "n_qubits": 1 }, "hamiltonian": { "terms": [ { "coefficient": 1.0, "paulis": [ { "qubit": 0, "axis": 1 } ] } ], "constant_offset": 0.0 }, "evolution": 3.14159, "observables": [ { "type": "SX", "qubit": 0 } ], "noise": { "type": "none" } }
```

### 2. Two-Qubit Transverse Ising Model

```cbor
{ "version": 1, "system": { "n_qubits": 2 }, "hamiltonian": { "terms": [ { "coefficient": 1.0, "paulis": [ { "qubit": 0, "axis": 1 }, { "qubit": 1, "axis": 1 } ] } ], "constant_offset": 0.0 }, "evolution": 1.5708, "observables": [ { "type": "SZ", "qubit": 0 } ], "noise": { "type": "none" } }
```

### 3. Single Qubit with Noise

```cbor
{ "version": 1, "system": { "n_qubits": 1 }, "hamiltonian": { "terms": [ { "coefficient": 1.0, "paulis": [ { "qubit": 0, "axis": 3 } ] } ], "constant_offset": 0.0 }, "evolution": 1.0, "observables": [ { "type": "SZ", "qubit": 0 } ], "noise": { "type": "depolarizing", "rate": 0.01 } }
```