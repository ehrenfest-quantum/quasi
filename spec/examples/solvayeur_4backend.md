# Example: Solvayeur 4-Backend Scheduling Hamiltonian

**File:** `solvayeur_4backend.cbor.hex`
**Size:** 404 bytes
**Schema:** `EhrenfestProgram` v0.1

## Physical Description

The Solvayeur scheduling kernel: an Ehrenfest program that encodes a quantum
annealing-inspired scheduling decision across 4 quantum backends.

```
H = J*Z_0*Z_1 + h_0*Z_0 + h_1*Z_1 + Gamma*X_0 + Gamma*X_1
```

With:
- J = 0.1 (contention coupling between backend pairs)
- h_0 = 0.3 (bias from previous scheduling round for backend pair 0)
- h_1 = 0.2 (bias from previous scheduling round for backend pair 1)
- Gamma = 0.5 (exploration strength -- quantum tunneling between states)

**Measure:** sigma_z on qubit 0 and qubit 1 (dispatching decision)
**Evolution:** 1.0 us total, 5 Trotter steps (dt = 0.2 us) -- shallow circuit
**Noise floor:** T1 >= 200 us, T2 >= 150 us

## Physical Meaning -- The OS Kernel

This is the Solvayeur kernel: a quantum operating system scheduler that uses
Ehrenfest physics programs to make dispatching decisions. Two qubits encode
four backends via binary encoding:

| Qubit 0 | Qubit 1 | Backend        |
|---------|---------|----------------|
| |0>     | |0>     | Backend A      |
| |0>     | |1>     | Backend B      |
| |1>     | |0>     | Backend C      |
| |1>     | |1>     | Backend D      |

The Hamiltonian terms encode scheduling physics:

- **ZZ coupling (J=0.1):** Contention -- backends that share resources incur
  an energy penalty when both are selected. Low J means weak contention.
- **Z biases (h_0, h_1):** Historical performance -- backends that performed
  well in previous rounds get a lower energy (higher selection probability).
  h_0=0.3 means backend pair 0 has stronger historical preference.
- **X terms (Gamma=0.5):** Exploration -- quantum tunneling allows the
  scheduler to explore non-obvious backend assignments. Without this term
  the scheduler would be a classical greedy algorithm.

The sigma_z measurements on both qubits after evolution produce the dispatching
decision. The ground state of H encodes the optimal backend assignment given
the current contention and historical data.

This proves that Ehrenfest programs can express not just physics simulations
but also combinatorial optimization problems -- including the QUASI operating
system's own scheduling kernel.

## Parameters

| Parameter              | Value   | Unit     |
|------------------------|---------|----------|
| Qubits                 | 2       |          |
| Backends encoded       | 4       |          |
| Contention J           | 0.1     | GHz*rad  |
| Bias h_0               | 0.3     | GHz*rad  |
| Bias h_1               | 0.2     | GHz*rad  |
| Exploration Gamma      | 0.5     | GHz*rad  |
| Total time             | 1.0     | us       |
| Trotter steps          | 5       |          |
| T1 minimum             | 200.0   | us       |
| T2 minimum             | 150.0   | us       |

## Expected Results

- Ground state: determined by competition between bias and exploration
- <Z_0> and <Z_1> give the dispatching decision
- With these parameters, the scheduler should prefer backends with lower
  energy (stronger historical bias), modulated by exploration

## Python Reconstruction

```python
import cbor2

with open("solvayeur_4backend.cbor.hex") as f:
    raw = bytes.fromhex(f.read().strip())

program = cbor2.loads(raw)
for term in program["hamiltonian"]["terms"]:
    axes = {0: "I", 1: "X", 2: "Y", 3: "Z"}
    ops = "".join(f"{axes[p['axis']]}{p['qubit']}" for p in term["paulis"])
    print(f"  {term['coefficient']:+.1f} * {ops}")
# +0.1 * Z0Z1, +0.3 * Z0, +0.2 * Z1, +0.5 * X0, +0.5 * X1
```
