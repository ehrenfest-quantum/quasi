# Ehrenfest Language Semantics

**QUASI-001** | Ehrenfest v0.1

## Core Principle

Ehrenfest programs express **physics**, not gate sequences.

A program describes:
- A quantum system (qubit count, optional hardware hints)
- A Hamiltonian (the energy operator)
- An evolution time (how long the system evolves)
- Observables (what to measure)
- Noise constraints (hardware requirements)

The compiler (Afana) derives all gates. The program never specifies circuits,
rotations, or measurement bases. This is the fundamental design invariant:
**programs describe physics; the compiler chooses gates**.

## Hamiltonian Semantics

The Hamiltonian `H` is a Hermitian operator expressed as a sum of Pauli terms:

```
H = Σᵢ cᵢ × ⊗ⱼ σⱼ  +  constant_offset
```

Where:
- `cᵢ` is a real coefficient in **GHz·rad** (natural units for superconducting QPUs)
- `σⱼ` is a Pauli operator (I, X, Y, Z) on qubit `j`
- `constant_offset` is a scalar energy shift in GHz·rad

Qubits not listed in a term's `paulis` array are implicitly Identity.
An empty `paulis` array represents the identity on all qubits (scalar term).

### Units

All energies are in **GHz·rad**. All times are in **microseconds (μs)**.
These are the natural units for superconducting quantum hardware where
qubit frequencies are typically 4–8 GHz.

## Observable Semantics

Observables define what the program measures after time evolution.
Each variant has specific physical meaning:

| Observable | Expression | Range | Meaning |
|---|---|---|---|
| SZ(q) | ⟨ψ\|Z_q\|ψ⟩ | [-1, +1] | Z-basis expectation on qubit q |
| SX(q) | ⟨ψ\|X_q\|ψ⟩ | [-1, +1] | X-basis expectation on qubit q |
| E | ⟨ψ\|H\|ψ⟩ | ℝ | Total energy in GHz·rad |
| rho(qs) | Tr_rest(\|ψ⟩⟨ψ\|) | matrix | Reduced density matrix on qubits qs |
| F(target) | ⟨ψ\|ρ_target\|ψ⟩ | [0, 1] | Fidelity against reference state |

Observables are **not** bit-string readouts. They express physics quantities.
Afana translates them to measurement circuits + classical post-processing.

## Evolution Semantics

The evolution block specifies time propagation under the Hamiltonian:

```
|ψ(t)⟩ = e^{-iHt} |ψ(0)⟩
```

- `total_us`: total evolution time `t` in microseconds
- `steps`: number of Trotter steps `n` for discretization
- `dt_us`: timestep `dt = t/n` in microseconds (must equal `total_us / steps`)

The initial state is always |0...0⟩. Custom state preparation is not part
of v0.1 (it belongs to the variational extension in v0.2).

### Trotterization

Trotterization is the **compiler's job**, not the program's concern.
The program specifies `steps` (how finely to discretize) but not the
decomposition order. Afana chooses the Trotter order based on accuracy
requirements and available hardware.

The Trotter-Suzuki decomposition approximates:

```
e^{-iHdt} ≈ e^{-iH₁dt} · e^{-iH₂dt} · ...     (first order)
e^{-iHdt} ≈ e^{-iH₁dt/2} · ... · e^{-iHₙdt/2}
           · e^{-iHₙdt/2} · ... · e^{-iH₁dt/2}  (second order)
```

Each Pauli term `e^{-iθP}` decomposes into basis-change gates, CNOT
ladders, and Rz rotations. This decomposition is deterministic and
architecture-independent.

## Noise Constraint Semantics

Noise constraints are **compile-time type assertions**, not runtime parameters.

```
noise: {
  t1_us: 100.0,    // "I require T1 ≥ 100 μs"
  t2_us: 80.0,     // "I require T2 ≥ 80 μs"
}
```

A program that cannot be executed on any backend satisfying its noise
constraints is a **type error** — detected by Afana at compile time,
not at runtime on the QPU.

Physical constraint: `t2_us ≤ 2 × t1_us` (enforced by Afana).

Optional fidelity bounds (`gate_fidelity_min`, `readout_fidelity_min`)
further constrain the target hardware. Values are in [0, 1].

## System Definition

The system block describes the physical setup:

- `n_qubits`: number of qubits (required, ≥ 1)
- `backend_hint`: non-binding hardware suggestion (e.g., `"ibm_torino"`)
- `cooling_profile`: optional cryogenic requirements for Alsvid integration

`backend_hint` is advisory. Afana MAY ignore it. It exists so that AI agents
can express hardware preferences without forcing a specific backend.

## What Ehrenfest Is Not

- **Not a gate language.** Programs do not contain `H`, `CNOT`, `Rz`, or
  any gate instructions. Those are Afana's output, not Ehrenfest's input.
- **Not a text format.** There is no human-readable serialization. CBOR
  binary is the only form. The human never sees an Ehrenfest program;
  AI agents generate them.
- **Not a circuit description.** Circuit topology, qubit routing, and
  gate decomposition are compiler concerns, not program concerns.
