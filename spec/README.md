# Ehrenfest Language Specification

## Overview

Ehrenfest is the QUASI quantum programming language, designed from first principles for AI agents as primary contributors. Programs are CBOR binary documents. There is no text form. There is no canonical file extension.

Programs express *physics* — Hamiltonians, observables, evolution time, and noise constraints — not gate sequences. The Afana compiler is responsible for deriving gate sequences via Trotterization and ZX-calculus optimization. This separation is deliberate: quantum hardware changes rapidly, but the physics of a problem does not. An Ehrenfest program targeting a 2-qubit transverse-Ising Hamiltonian will compile correctly to any backend Afana supports.

The human never sees an Ehrenfest program. The full loop:

```
human intent → AI → Ehrenfest (CBOR) → Afana → QPU → result → AI → human
```

## Schema

The authoritative specification is a CDDL schema:

- **v0.1:** [`ehrenfest-v0.1.cddl`](ehrenfest-v0.1.cddl) — Hamiltonians, observables, noise constraints
- **v0.2:** [`ehrenfest-v0.2.cddl`](ehrenfest-v0.2.cddl) — Adds parametric support (ParameterRef, ParameterMap)

Validate: `gem install cddl && cddl spec/ehrenfest-v0.1.cddl validate <program.cbor>`

## Compilation Pipeline

```
Ehrenfest (CBOR binary)
    → deserialize (afana/src/cbor.rs)
    → EhrenfestProgram (Hamiltonians, observables, noise constraints)
    → trotterize (afana/src/trotter.rs)
    → EhrenfestAst (gate sequences)
    → optimize: T-gate reduction + ZX-calculus (QuiZX)
    → emit OpenQASM 2.0 / 3.0
```

## Syntax

Ehrenfest programs are defined using a CDDL schema with the following key components:

- `version`: Must be 1 for v0.1, 2 for v0.2
- `system`: Physical context including number of qubits
- `hamiltonian`: Energy operator of the system
- `evolution`: Evolution time and Trotter decomposition parameters
- `observables`: List of measurable quantities
- `noise`: Noise constraints (T1, T2, gate fidelity)
- `parameters` (v0.2): Named variational parameters

## Semantics

- Programs describe expectation values and Hamiltonians
- The compiler (Afana) derives gate sequences via Trotterization
- Uses natural units (GHz·rad) for energy
- Supports Pauli operators (I, X, Y, Z)

## Type System

Ehrenfest uses a strong static type system with these core types:

- `uint`: Natural numbers
- `float`: Real numbers
- `tstr`: Text strings
- `PauliAxis`: Enum {I: 0, X: 1, Y: 2, Z: 3}
- `Observable`: Union of SigmaZ, SigmaX, Energy, Density, Fidelity
- `NoiseConstraint`: T1, T2, gate fidelity minimums — compile-time type errors

## Noise as Type System

A program that cannot execute within its noise budget is a type error, not a runtime failure. `NoiseConstraint` fields (`t1_us`, `t2_us`, `gate_fidelity_min`) are enforced at compile time.

## Examples

Examples are CBOR binary with companion documentation in [`examples/`](examples/):

| Example | Qubits | Physics |
|---------|--------|---------|
| [Rabi oscillation](examples/rabi_oscillation_1q.md) | 1 | Single-qubit σ_x drive |
| [Transverse Ising](examples/transverse_ising_2q.md) | 2 | Transverse-field Ising model |
| [Heisenberg chain](examples/heisenberg_4q.md) | 4 | XXX Heisenberg spin chain |
| [VQE H₂](examples/vqe_h2_parametric.cbor.hex) | 2 | Variational H₂ ground state (v0.2) |
