# Heisenberg Model on 2D Grid (12 qubits)

## Model Description

This example implements the Heisenberg model on a 2D 3×4 grid lattice with 12 qubits. The Hamiltonian is:

```
H = J * Σ_<i,j> (X_i X_j + Y_i Y_j + Z_i Z_j)
```

Where `<i,j>` denotes nearest-neighbor pairs on the grid, and we use J=1.0 as the coupling constant.

The qubits are arranged on a grid as follows:
```
 0  1  2  3
 4  5  6  7
 8  9 10 11
```

## Parameters

- Number of qubits: 12
- Coupling constant (J): 1.0
- Evolution time: 100.0 μs
- Trotter steps: 20
- Time step (dt): 5.0 μs
- Required T1: 200.0 μs
- Required T2: 100.0 μs

## Nearest-neighbor Pairs

The model includes interactions between these nearest-neighbor pairs:
- Horizontal: (0,1), (1,2), (2,3), (4,5), (5,6), (6,7), (8,9), (9,10), (10,11)
- Vertical: (0,4), (1,5), (2,6), (3,7), (4,8), (5,9), (6,10), (7,11)

## Expected Results

This model can be used to simulate antiferromagnetic behavior on a 2D lattice. The energy observable measures the total expectation value of the Hamiltonian. The system exhibits quantum spin correlations that can be studied through the evolution.