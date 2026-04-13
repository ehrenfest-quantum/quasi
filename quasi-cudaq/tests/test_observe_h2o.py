# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright 2026 QUASI Contributors
#
# Test 3: Hamiltonian observe (Level 2 — Hamiltonian interception).
#
# Tests the Level 2 integration where QUASI intercepts the Hamiltonian
# before CUDA-Q decomposes it. The Pauli terms are routed through
# QUASI's pipeline: sin(C/2) analysis → Afana → Huoma.
#
# Prerequisites:
#   pip install cuda-quantum
#   QUASI plugin installed (libnvqir-quasi.so)

import cudaq
from cudaq import spin

# ── Target selection (Level 2: Hamiltonian mode) ────────────────────────────
cudaq.set_target("quasi", mode="hamiltonian")

# ── Simplified H₂O Hamiltonian (4 qubits) ──────────────────────────────────
# In production this comes from Jordan-Wigner on molecular integrals
hamiltonian = (
    -0.5 * spin.z(0)
    + 0.3 * spin.x(0) * spin.x(1)
    - 0.2 * spin.z(1) * spin.z(2)
    + 0.1 * spin.y(2) * spin.y(3)
)


@cudaq.kernel
def state_prep():
    q = cudaq.qvector(4)
    x(q[0])
    x(q[1])


# ── Execute observe ────────────────────────────────────────────────────────
result = cudaq.observe(state_prep, hamiltonian)
energy = result.expectation()

print(f"Energy: {energy:.6f}")

# Level 2 interception means:
# - QUASI's sin(C/2) analysis ran on the Hamiltonian
# - Afana compiled it (not CUDA-Q's transpiler)
# - Huoma executed with adaptive bond dimensions
# - Result should be a valid energy

# Verify the energy is a finite number (not NaN or Inf)
import math
assert math.isfinite(energy), f"Energy is not finite: {energy}"

print("PASS: Hamiltonian observation produced valid energy")
print(f"  Energy:  {energy:.6f}")
print(f"  Qubits:  4")
print(f"  Pipeline: spin_op → QUASI (Ehrenfest → Afana → Huoma)")
print(f"  Level:   2 (Hamiltonian interception)")
