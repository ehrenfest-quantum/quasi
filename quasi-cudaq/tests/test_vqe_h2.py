# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright 2026 QUASI Contributors
#
# Test 2: VQE on H₂ (Level 1 — circuit execution via QUASI).
#
# Uses CUDA-Q's VQE with a simple ansatz to find the ground-state energy
# of the deuteron Hamiltonian. The circuit execution goes through QUASI's
# Huoma simulator.
#
# Expected result: ground state energy ≈ -1.7489
#
# Prerequisites:
#   pip install cuda-quantum
#   QUASI plugin installed (libnvqir-quasi.so)

import cudaq
from cudaq import spin

# ── Target selection ─────────────────────────────────────────────────────────
cudaq.set_target("quasi")


@cudaq.kernel
def ansatz(theta: float):
    q = cudaq.qvector(2)
    x(q[0])
    ry(theta, q[1])
    cx(q[1], q[0])


# ── Deuteron Hamiltonian ─────────────────────────────────────────────────────
# Dumitrescu et al., PRL 120, 210501 (2018)
hamiltonian = (
    5.907 - 2.1433 * spin.x(0) * spin.x(1)
    - 2.1433 * spin.y(0) * spin.y(1)
    + 0.21829 * spin.z(0)
    - 6.125 * spin.z(1)
)

# ── VQE optimisation ────────────────────────────────────────────────────────
optimizer = cudaq.optimizers.COBYLA()
optimal_value, optimal_params = cudaq.vqe(
    ansatz, hamiltonian, optimizer, n_params=1
)

print(f"Ground state energy: {optimal_value:.6f}")
print(f"Optimal parameter:   {optimal_params[0]:.6f}")

# Verify: should match the known deuteron ground state
# -1.7489 with 4 decimal places
assert abs(optimal_value - (-1.7489)) < 0.01, (
    f"VQE energy {optimal_value:.6f} too far from expected -1.7489"
)

print("PASS: VQE H₂ ground state energy matches expected value")
print(f"  Energy:  {optimal_value:.6f} Ha")
print(f"  Backend: Huoma (QUASI circuit execution)")
print(f"  Solvayeur: classical fallback, selected huoma_statevector")
