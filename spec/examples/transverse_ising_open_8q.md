# Transverse-Field Ising Model (Open Boundary Conditions, 8 qubits)

## Physics

The transverse-field Ising model describes a chain of spins with nearest-neighbour ZZ interactions and a global transverse magnetic field (X terms). Open boundary conditions mean the chain ends do not interact.

Hamiltonian:
$$
H = J \sum_{i=0}^{6} Z_i Z_{i+1} + h \sum_{i=0}^{7} X_i
$$

Parameters:
- Coupling strength: $J = -1.0$ (ferromagnetic)
- Transverse field: $h = 1.0$
- Number of qubits: $8$
- Evolution time: $1.0\ \mu$s, divided into 10 Trotter steps of $0.1\ \mu$s each

## Observables

Energy ($\langle H \rangle$) – the expectation value of the full Hamiltonian.

## Noise Constraints

- T1: $1000\ \mu$s
- T2: $500\ \mu$s

## Expected Results

For these parameters, the ground state is paramagnetic (all spins aligned with the transverse field). The energy expectation after evolution from an initial product state will oscillate. The emitted QASM3 should contain exactly 7 nearest-neighbour ZZ interaction terms and 8 transverse-field X terms in the Trotterized gate sequence.

## Usage

```bash
afana spec/examples/transverse_ising_open_8q.cbor.hex --qasm v3
```

This produces OpenQASM 3.0 code with only standard gates (H, X, Y, Z, S, T, Cx, Cz, Rx, Ry, Rz, Swap, Ccx) and correct 8-qubit chain connectivity.
