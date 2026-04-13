// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Statevector simulator for circuit execution.
//!
//! This is the "Huoma statevector equivalent" — a pure-Rust simulator
//! that executes parsed QASM circuits and returns measurement samples.
//! For small circuits (≤ 20 qubits) it runs exact statevector simulation.
//!
//! The simulator is invoked from the C FFI layer when CUDA-Q sends a
//! circuit via the CircuitSimulator API.

use crate::qasm::{Circuit, GateOp};

/// Measurement result: bitstring → count.
#[derive(Debug, Clone)]
pub struct SampleResult {
    pub counts: Vec<(String, u32)>,
    pub fidelity_estimate: f64,
    pub wall_time_ms: f64,
}

/// Simulate a circuit and return measurement samples.
///
/// The circuit is executed as a statevector simulation. Measurement
/// probabilities are computed from |ψ|², then sampled `shots` times.
pub fn simulate(circuit: &Circuit, measure_qubits: &[usize], shots: u32) -> SampleResult {
    let start = std::time::Instant::now();
    let n = circuit.n_qubits;
    assert!(n <= 20, "statevector simulator limited to 20 qubits, got {n}");

    let dim = 1usize << n;

    // Statevector: complex amplitudes as (re, im) pairs
    let mut state_re = vec![0.0f64; dim];
    let mut state_im = vec![0.0f64; dim];
    state_re[0] = 1.0; // |00...0⟩

    // Apply gates
    for op in &circuit.ops {
        match op {
            GateOp::H(q) => apply_h(&mut state_re, &mut state_im, *q, n),
            GateOp::X(q) => apply_x(&mut state_re, &mut state_im, *q, n),
            GateOp::Y(q) => apply_y(&mut state_re, &mut state_im, *q, n),
            GateOp::Z(q) => apply_z(&mut state_re, &mut state_im, *q, n),
            GateOp::S(q) => apply_phase(&mut state_re, &mut state_im, *q, n, 0.0, 1.0),
            GateOp::T(q) => {
                let c = std::f64::consts::FRAC_PI_4.cos();
                let s = std::f64::consts::FRAC_PI_4.sin();
                apply_phase(&mut state_re, &mut state_im, *q, n, c, s);
            }
            GateOp::Rx(angle, q) => apply_rx(&mut state_re, &mut state_im, *q, n, *angle),
            GateOp::Ry(angle, q) => apply_ry(&mut state_re, &mut state_im, *q, n, *angle),
            GateOp::Rz(angle, q) => apply_rz(&mut state_re, &mut state_im, *q, n, *angle),
            GateOp::Cx(ctrl, tgt) => apply_cx(&mut state_re, &mut state_im, *ctrl, *tgt, n),
            GateOp::Cz(ctrl, tgt) => apply_cz(&mut state_re, &mut state_im, *ctrl, *tgt, n),
            GateOp::Ccx(c1, c2, tgt) => {
                apply_ccx(&mut state_re, &mut state_im, *c1, *c2, *tgt, n)
            }
            GateOp::Measure(_, _) => {
                // Measurements are deferred — we sample from the final state
            }
        }
    }

    // Compute probabilities
    let probs: Vec<f64> = state_re
        .iter()
        .zip(&state_im)
        .map(|(&re, &im)| re * re + im * im)
        .collect();

    // Sample measurement outcomes
    let counts = sample_measurements(&probs, measure_qubits, n, shots);

    let elapsed = start.elapsed();
    SampleResult {
        counts,
        fidelity_estimate: 1.0, // exact simulation
        wall_time_ms: elapsed.as_secs_f64() * 1000.0,
    }
}

/// Compute expectation value ⟨ψ|H|ψ⟩ for a Pauli Hamiltonian.
///
/// Used by the Level 2 Hamiltonian interceptor path.
pub fn expectation_value(pauli_terms: &[(f64, String)], n_qubits: usize) -> f64 {
    quasi_bridge::postprocess::ground_state_energy(pauli_terms, n_qubits).unwrap_or(f64::NAN)
}

// ── Gate implementations ────────────────────────────────────────────────────

/// Apply Hadamard gate on qubit `q`.
fn apply_h(re: &mut [f64], im: &mut [f64], q: usize, n: usize) {
    let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
    let dim = 1usize << n;
    let mask = 1usize << q;
    for i in 0..dim {
        if i & mask == 0 {
            let j = i | mask;
            let a_re = re[i];
            let a_im = im[i];
            let b_re = re[j];
            let b_im = im[j];
            re[i] = inv_sqrt2 * (a_re + b_re);
            im[i] = inv_sqrt2 * (a_im + b_im);
            re[j] = inv_sqrt2 * (a_re - b_re);
            im[j] = inv_sqrt2 * (a_im - b_im);
        }
    }
}

/// Apply Pauli-X gate on qubit `q`.
fn apply_x(re: &mut [f64], im: &mut [f64], q: usize, n: usize) {
    let dim = 1usize << n;
    let mask = 1usize << q;
    for i in 0..dim {
        if i & mask == 0 {
            let j = i | mask;
            re.swap(i, j);
            im.swap(i, j);
        }
    }
}

/// Apply Pauli-Y gate on qubit `q`.
/// Y = [[0, -i], [i, 0]]
fn apply_y(re: &mut [f64], im: &mut [f64], q: usize, n: usize) {
    let dim = 1usize << n;
    let mask = 1usize << q;
    for i in 0..dim {
        if i & mask == 0 {
            let j = i | mask;
            let a_re = re[i];
            let a_im = im[i];
            let b_re = re[j];
            let b_im = im[j];
            // |0⟩ → i|1⟩:  new[j] = i * old[i] = (-a_im, a_re)
            // |1⟩ → -i|0⟩: new[i] = -i * old[j] = (b_im, -b_re)
            re[i] = b_im;
            im[i] = -b_re;
            re[j] = -a_im;
            im[j] = a_re;
        }
    }
}

/// Apply Pauli-Z gate on qubit `q`.
fn apply_z(re: &mut [f64], im: &mut [f64], q: usize, n: usize) {
    let dim = 1usize << n;
    let mask = 1usize << q;
    for i in 0..dim {
        if i & mask != 0 {
            re[i] = -re[i];
            im[i] = -im[i];
        }
    }
}

/// Apply a phase gate diag(1, e^{iθ}) where phase = (c, s).
fn apply_phase(re: &mut [f64], im: &mut [f64], q: usize, n: usize, c: f64, s: f64) {
    let dim = 1usize << n;
    let mask = 1usize << q;
    for i in 0..dim {
        if i & mask != 0 {
            let old_re = re[i];
            let old_im = im[i];
            re[i] = c * old_re - s * old_im;
            im[i] = s * old_re + c * old_im;
        }
    }
}

/// Apply Rx(θ) = exp(-iθX/2) = [[cos(θ/2), -i·sin(θ/2)], [-i·sin(θ/2), cos(θ/2)]]
fn apply_rx(re: &mut [f64], im: &mut [f64], q: usize, n: usize, angle: f64) {
    let half = angle / 2.0;
    let c = half.cos();
    let s = half.sin();
    let dim = 1usize << n;
    let mask = 1usize << q;
    for i in 0..dim {
        if i & mask == 0 {
            let j = i | mask;
            let a_re = re[i];
            let a_im = im[i];
            let b_re = re[j];
            let b_im = im[j];
            // new_a = cos(θ/2)·a - i·sin(θ/2)·b
            re[i] = c * a_re + s * b_im;
            im[i] = c * a_im - s * b_re;
            // new_b = -i·sin(θ/2)·a + cos(θ/2)·b
            re[j] = s * a_im + c * b_re;
            im[j] = -s * a_re + c * b_im;
        }
    }
}

/// Apply Ry(θ) = exp(-iθY/2) = [[cos(θ/2), -sin(θ/2)], [sin(θ/2), cos(θ/2)]]
fn apply_ry(re: &mut [f64], im: &mut [f64], q: usize, n: usize, angle: f64) {
    let half = angle / 2.0;
    let c = half.cos();
    let s = half.sin();
    let dim = 1usize << n;
    let mask = 1usize << q;
    for i in 0..dim {
        if i & mask == 0 {
            let j = i | mask;
            let a_re = re[i];
            let a_im = im[i];
            let b_re = re[j];
            let b_im = im[j];
            // new_a = cos(θ/2)·a - sin(θ/2)·b
            re[i] = c * a_re - s * b_re;
            im[i] = c * a_im - s * b_im;
            // new_b = sin(θ/2)·a + cos(θ/2)·b
            re[j] = s * a_re + c * b_re;
            im[j] = s * a_im + c * b_im;
        }
    }
}

/// Apply Rz(θ) = exp(-iθZ/2) = [[e^{-iθ/2}, 0], [0, e^{iθ/2}]]
fn apply_rz(re: &mut [f64], im: &mut [f64], q: usize, n: usize, angle: f64) {
    let half = angle / 2.0;
    let c = half.cos();
    let s = half.sin();
    let dim = 1usize << n;
    let mask = 1usize << q;
    for i in 0..dim {
        let old_re = re[i];
        let old_im = im[i];
        if i & mask == 0 {
            // e^{-iθ/2}
            re[i] = c * old_re + s * old_im;
            im[i] = -s * old_re + c * old_im;
        } else {
            // e^{+iθ/2}
            re[i] = c * old_re - s * old_im;
            im[i] = s * old_re + c * old_im;
        }
    }
}

/// Apply CNOT: flip target qubit when control is |1⟩.
fn apply_cx(re: &mut [f64], im: &mut [f64], ctrl: usize, tgt: usize, n: usize) {
    let dim = 1usize << n;
    let ctrl_mask = 1usize << ctrl;
    let tgt_mask = 1usize << tgt;
    for i in 0..dim {
        // Only act when control is |1⟩ and target is |0⟩ (swap with target |1⟩)
        if (i & ctrl_mask != 0) && (i & tgt_mask == 0) {
            let j = i | tgt_mask;
            re.swap(i, j);
            im.swap(i, j);
        }
    }
}

/// Apply CZ: phase-flip when both qubits are |1⟩.
fn apply_cz(re: &mut [f64], im: &mut [f64], q1: usize, q2: usize, n: usize) {
    let dim = 1usize << n;
    let mask1 = 1usize << q1;
    let mask2 = 1usize << q2;
    for i in 0..dim {
        if (i & mask1 != 0) && (i & mask2 != 0) {
            re[i] = -re[i];
            im[i] = -im[i];
        }
    }
}

/// Apply Toffoli (CCX): flip target when both controls are |1⟩.
fn apply_ccx(re: &mut [f64], im: &mut [f64], c1: usize, c2: usize, tgt: usize, n: usize) {
    let dim = 1usize << n;
    let c1_mask = 1usize << c1;
    let c2_mask = 1usize << c2;
    let tgt_mask = 1usize << tgt;
    for i in 0..dim {
        if (i & c1_mask != 0) && (i & c2_mask != 0) && (i & tgt_mask == 0) {
            let j = i | tgt_mask;
            re.swap(i, j);
            im.swap(i, j);
        }
    }
}

// ── Measurement sampling ────────────────────────────────────────────────────

/// Sample measurement outcomes from the probability distribution.
///
/// Uses a simple deterministic sampling strategy: for each basis state,
/// compute the expected number of counts, then distribute shots
/// proportionally. This avoids requiring a PRNG dependency.
fn sample_measurements(
    probs: &[f64],
    measure_qubits: &[usize],
    n_qubits: usize,
    shots: u32,
) -> Vec<(String, u32)> {
    // If no specific qubits to measure, measure all
    let qubits: Vec<usize> = if measure_qubits.is_empty() {
        (0..n_qubits).collect()
    } else {
        measure_qubits.to_vec()
    };

    let n_measure = qubits.len();
    let n_outcomes = 1usize << n_measure;

    // Marginalise: sum probabilities for each measurement outcome
    let mut marginal = vec![0.0f64; n_outcomes];
    for (state, &p) in probs.iter().enumerate() {
        if p < 1e-16 {
            continue;
        }
        // Extract measured bits from the full state
        let mut outcome = 0usize;
        for (bit_pos, &qubit) in qubits.iter().enumerate() {
            if (state >> qubit) & 1 == 1 {
                outcome |= 1 << bit_pos;
            }
        }
        marginal[outcome] += p;
    }

    // Distribute shots proportionally (deterministic)
    let mut counts = vec![0u32; n_outcomes];
    let mut remaining_shots = shots;

    // First pass: floor allocation
    for (i, &p) in marginal.iter().enumerate() {
        let expected = p * shots as f64;
        let floored = expected.floor() as u32;
        counts[i] = floored;
        remaining_shots -= floored;
    }

    // Second pass: distribute remaining shots to highest-remainder outcomes
    if remaining_shots > 0 {
        let mut remainders: Vec<(usize, f64)> = marginal
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let expected = p * shots as f64;
                (i, expected - expected.floor())
            })
            .collect();
        remainders.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for &(i, _) in remainders.iter().take(remaining_shots as usize) {
            counts[i] += 1;
        }
    }

    // Convert to bitstring → count pairs (skip zero counts)
    let mut result = Vec::new();
    for (outcome, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let mut bitstring = String::with_capacity(n_measure);
        // MSB first (standard quantum convention)
        for bit_pos in (0..n_measure).rev() {
            if (outcome >> bit_pos) & 1 == 1 {
                bitstring.push('1');
            } else {
                bitstring.push('0');
            }
        }
        result.push((bitstring, count));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qasm;

    #[test]
    fn bell_state_produces_00_and_11() {
        let qasm_str = r#"
OPENQASM 2.0;
include "qelib1.inc";
qreg q[2];
creg c[2];
h q[0];
cx q[0],q[1];
"#;
        let circuit = qasm::parse_qasm(qasm_str).unwrap();
        let result = simulate(&circuit, &[0, 1], 1000);

        // Bell state: should only have |00⟩ and |11⟩
        assert_eq!(result.counts.len(), 2);
        for (bits, count) in &result.counts {
            assert!(
                bits == "00" || bits == "11",
                "unexpected bitstring: {bits}"
            );
            assert!(*count > 0);
        }
        let total: u32 = result.counts.iter().map(|(_, c)| c).sum();
        assert_eq!(total, 1000);
    }

    #[test]
    fn x_gate_flips_to_one() {
        let qasm_str = r#"
OPENQASM 2.0;
qreg q[1];
x q[0];
"#;
        let circuit = qasm::parse_qasm(qasm_str).unwrap();
        let result = simulate(&circuit, &[0], 100);

        assert_eq!(result.counts.len(), 1);
        assert_eq!(result.counts[0].0, "1");
        assert_eq!(result.counts[0].1, 100);
    }

    #[test]
    fn identity_circuit_all_zeros() {
        let qasm_str = r#"
OPENQASM 2.0;
qreg q[3];
"#;
        let circuit = qasm::parse_qasm(qasm_str).unwrap();
        let result = simulate(&circuit, &[0, 1, 2], 50);

        assert_eq!(result.counts.len(), 1);
        assert_eq!(result.counts[0].0, "000");
        assert_eq!(result.counts[0].1, 50);
    }

    #[test]
    fn ghz_5_only_00000_and_11111() {
        let qasm_str = r#"
OPENQASM 2.0;
qreg q[5];
h q[0];
cx q[0],q[1];
cx q[1],q[2];
cx q[2],q[3];
cx q[3],q[4];
"#;
        let circuit = qasm::parse_qasm(qasm_str).unwrap();
        let result = simulate(&circuit, &[0, 1, 2, 3, 4], 10000);

        assert_eq!(result.counts.len(), 2);
        for (bits, count) in &result.counts {
            assert!(
                bits == "00000" || bits == "11111",
                "unexpected: {bits}"
            );
            assert!(*count > 0);
        }
    }

    #[test]
    fn ry_pi_half_gives_equal_superposition() {
        let qasm_str = r#"
OPENQASM 2.0;
qreg q[1];
ry(1.5707963267948966) q[0];
"#;
        let circuit = qasm::parse_qasm(qasm_str).unwrap();
        let result = simulate(&circuit, &[0], 1000);

        assert_eq!(result.counts.len(), 2);
        let total: u32 = result.counts.iter().map(|(_, c)| c).sum();
        assert_eq!(total, 1000);
        for (_, count) in &result.counts {
            assert!(*count == 500, "expected 500, got {count}");
        }
    }

    #[test]
    fn cz_gate_phase_flip() {
        // |+⟩|1⟩ → CZ → |−⟩|1⟩ → H → |1⟩|1⟩
        let qasm_str = r#"
OPENQASM 2.0;
qreg q[2];
h q[0];
x q[1];
cz q[0],q[1];
h q[0];
"#;
        let circuit = qasm::parse_qasm(qasm_str).unwrap();
        let result = simulate(&circuit, &[0, 1], 100);

        assert_eq!(result.counts.len(), 1);
        assert_eq!(result.counts[0].0, "11");
        assert_eq!(result.counts[0].1, 100);
    }

    #[test]
    fn ccx_toffoli() {
        // |11⟩|0⟩ → CCX → |11⟩|1⟩ = |111⟩
        let qasm_str = r#"
OPENQASM 2.0;
qreg q[3];
x q[0];
x q[1];
ccx q[0],q[1],q[2];
"#;
        let circuit = qasm::parse_qasm(qasm_str).unwrap();
        let result = simulate(&circuit, &[0, 1, 2], 100);

        assert_eq!(result.counts.len(), 1);
        assert_eq!(result.counts[0].0, "111");
    }
}
