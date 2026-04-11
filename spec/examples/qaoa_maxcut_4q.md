# Example: QAOA MaxCut, 4-Node Cycle Graph

**File:** `qaoa_maxcut_4q.cbor.hex`
**Size:** 366 bytes
**Schema:** `EhrenfestProgram` v0.1

## Physical Description

QAOA cost Hamiltonian for the MaxCut problem on a 4-node cycle graph (0-1-2-3-0):

```
H_C = (1/2)(I - Z_0 Z_1) + (1/2)(I - Z_1 Z_2) + (1/2)(I - Z_2 Z_3) + (1/2)(I - Z_3 Z_0)
    = 2*I - (1/2)(Z_0 Z_1 + Z_1 Z_2 + Z_2 Z_3 + Z_3 Z_0)
```

**Measure:** Energy (E) -- expectation value of H_C gives the cut value
**Evolution:** 1.0 us total, 10 Trotter steps (dt = 0.1 us)
**Noise floor:** T1 >= 200 us, T2 >= 150 us (superconducting)

## Physical Meaning

MaxCut partitions graph vertices into two sets to maximize the number of edges
crossing the partition. For the 4-node cycle, the optimal cut has value 4 (all
edges cut), achieved by alternating partition assignments (0,1,0,1).

The QAOA cost Hamiltonian encodes this: its ground state energy equals the
negative of the maximum cut value. The constant offset of 2.0 shifts the
spectrum so that the ground state energy corresponds to the cut value directly.

This is the standard benchmark for QAOA on near-term quantum hardware.

## Parameters

| Parameter        | Value   | Unit     |
|------------------|---------|----------|
| Qubits           | 4       |          |
| ZZ terms         | 4       |          |
| ZZ coefficient   | -0.5    | GHz*rad  |
| Constant offset  | 2.0     | GHz*rad  |
| Total time       | 1.0     | us       |
| Trotter steps    | 10      |          |
| T1 minimum       | 200.0   | us       |
| T2 minimum       | 150.0   | us       |

## Expected Results

- Maximum cut value: 4 (all edges of the cycle are cut)
- Ground state energy of H_C: 0.0 (with offset, minimum eigenvalue = 0)
- Optimal bitstring: |0101> or |1010> (alternating partition)

## Python Reconstruction

```python
import cbor2

with open("qaoa_maxcut_4q.cbor.hex") as f:
    raw = bytes.fromhex(f.read().strip())

program = cbor2.loads(raw)
for term in program["hamiltonian"]["terms"]:
    qubits = [p["qubit"] for p in term["paulis"]]
    print(f"  coeff={term['coefficient']}, qubits={qubits}")
# Each term: coeff=-0.5, ZZ on pairs (0,1), (1,2), (2,3), (3,0)
```
