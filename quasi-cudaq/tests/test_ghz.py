# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright 2026 QUASI Contributors
#
# Test 1: GHZ state sampling via CUDA-Q → QUASI.
#
# This test creates a 5-qubit GHZ state and verifies that sampling
# produces only |00000⟩ and |11111⟩ with approximately equal probability.
#
# Prerequisites:
#   pip install cuda-quantum
#   QUASI plugin installed (libnvqir-quasi.so)
#
# Usage:
#   python test_ghz.py
#   # or with cuQuantum for comparison:
#   CUDAQ_TARGET=nvidia python test_ghz.py

import cudaq

# ── Target selection ─────────────────────────────────────────────────────────
# Change this one line to switch backends. Everything else stays the same.
cudaq.set_target("quasi")


@cudaq.kernel
def ghz():
    qubits = cudaq.qvector(5)
    h(qubits[0])
    for i in range(4):
        cx(qubits[i], qubits[i + 1])
    mz(qubits)


# ── Execute and verify ──────────────────────────────────────────────────────

result = cudaq.sample(ghz, shots_count=10000)
print(result)

# Verify: only |00000⟩ and |11111⟩ should appear
counts = dict(result)
assert len(counts) == 2, f"Expected 2 outcomes, got {len(counts)}: {counts}"
assert "00000" in counts, f"Missing |00000⟩: {counts}"
assert "11111" in counts, f"Missing |11111⟩: {counts}"

# Each should be approximately 50% (within statistical error)
total = sum(counts.values())
ratio_00000 = counts["00000"] / total
ratio_11111 = counts["11111"] / total
assert 0.45 < ratio_00000 < 0.55, f"|00000⟩ ratio {ratio_00000:.3f} outside [0.45, 0.55]"
assert 0.45 < ratio_11111 < 0.55, f"|11111⟩ ratio {ratio_11111:.3f} outside [0.45, 0.55]"

print("PASS: GHZ state sampling matches expected distribution")
print(f"  |00000⟩: {counts['00000']} ({ratio_00000:.1%})")
print(f"  |11111⟩: {counts['11111']} ({ratio_11111:.1%})")
print(f"  Backend: Huoma (commensurability-guided, adaptive χ)")
