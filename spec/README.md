# Ehrenfest Language Specification

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