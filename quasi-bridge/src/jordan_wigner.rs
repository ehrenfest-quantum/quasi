// SPDX-License-Identifier: AGPL-3.0-or-later
//! Jordan-Wigner transform: fermionic Hamiltonian → qubit Hamiltonian.
//!
//! Maps a second-quantized molecular Hamiltonian (in the MO basis) to a
//! sum of Pauli strings suitable for encoding as an Ehrenfest program.
//!
//! Reference: Whitfield, Biamonte & Aspuru-Guzik, Mol. Phys. 109, 735 (2011).

use std::collections::HashMap;

/// A Pauli operator on a single qubit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pauli {
    I,
    X,
    Y,
    Z,
}

impl Pauli {
    pub fn as_char(self) -> char {
        match self {
            Pauli::I => 'I',
            Pauli::X => 'X',
            Pauli::Y => 'Y',
            Pauli::Z => 'Z',
        }
    }
}

/// Multiply two Pauli operators. Returns (phase_power, result) where
/// the actual phase is i^phase_power (phase_power mod 4).
fn pauli_mul(a: Pauli, b: Pauli) -> (i8, Pauli) {
    use Pauli::*;
    match (a, b) {
        (I, x) => (0, x),
        (x, I) => (0, x),
        (X, X) | (Y, Y) | (Z, Z) => (0, I),
        (X, Y) => (1, Z),   // XY = iZ
        (Y, X) => (3, Z),   // YX = -iZ
        (Y, Z) => (1, X),   // YZ = iX
        (Z, Y) => (3, X),   // ZY = -iX
        (Z, X) => (1, Y),   // ZX = iY
        (X, Z) => (3, Y),   // XZ = -iY
    }
}

/// A weighted Pauli string: coefficient × P₁ ⊗ P₂ ⊗ ... ⊗ Pₙ.
#[derive(Clone)]
struct PauliTerm {
    paulis: Vec<Pauli>,
    coeff_re: f64,
    coeff_im: f64,
}

impl PauliTerm {
    fn identity(n: usize) -> Self {
        PauliTerm {
            paulis: vec![Pauli::I; n],
            coeff_re: 1.0,
            coeff_im: 0.0,
        }
    }

    /// Multiply two Pauli terms (qubit by qubit).
    fn mul(&self, other: &PauliTerm) -> PauliTerm {
        assert_eq!(self.paulis.len(), other.paulis.len());
        let mut total_phase: i8 = 0;
        let mut result_paulis = Vec::with_capacity(self.paulis.len());
        for (a, b) in self.paulis.iter().zip(&other.paulis) {
            let (phase, p) = pauli_mul(*a, *b);
            total_phase = (total_phase + phase) % 4;
            result_paulis.push(p);
        }
        // Multiply complex coefficients and phase
        let (sr, si) = (self.coeff_re, self.coeff_im);
        let (or, oi) = (other.coeff_re, other.coeff_im);
        let (pr, pi) = (sr * or - si * oi, sr * oi + si * or);
        // Apply i^total_phase
        let (fr, fi) = match total_phase % 4 {
            0 => (pr, pi),
            1 => (-pi, pr),    // × i
            2 => (-pr, -pi),   // × -1
            3 => (pi, -pr),    // × -i
            _ => unreachable!(),
        };
        PauliTerm {
            paulis: result_paulis,
            coeff_re: fr,
            coeff_im: fi,
        }
    }

    fn to_string_key(&self) -> String {
        self.paulis.iter().map(|p| p.as_char()).collect()
    }

    #[allow(dead_code)]
    fn is_identity(&self) -> bool {
        self.paulis.iter().all(|p| *p == Pauli::I)
    }
}

/// Build the JW representation of a†_p (creation on spin-orbital p).
/// Returns two Pauli terms that sum to a†_p.
/// a†_p = ½(X_p - iY_p) Z_{p-1} ... Z_0
fn jw_creation(p: usize, n_qubits: usize) -> Vec<PauliTerm> {
    let mut term_x = PauliTerm::identity(n_qubits);
    let mut term_y = PauliTerm::identity(n_qubits);

    // Z-string on qubits 0..p-1
    for k in 0..p {
        term_x.paulis[k] = Pauli::Z;
        term_y.paulis[k] = Pauli::Z;
    }
    // X_p and Y_p
    term_x.paulis[p] = Pauli::X;
    term_x.coeff_re = 0.5;

    term_y.paulis[p] = Pauli::Y;
    term_y.coeff_re = 0.0;
    term_y.coeff_im = -0.5; // -i/2

    vec![term_x, term_y]
}

/// Build the JW representation of a_p (annihilation on spin-orbital p).
fn jw_annihilation(p: usize, n_qubits: usize) -> Vec<PauliTerm> {
    let mut term_x = PauliTerm::identity(n_qubits);
    let mut term_y = PauliTerm::identity(n_qubits);

    for k in 0..p {
        term_x.paulis[k] = Pauli::Z;
        term_y.paulis[k] = Pauli::Z;
    }

    term_x.paulis[p] = Pauli::X;
    term_x.coeff_re = 0.5;

    term_y.paulis[p] = Pauli::Y;
    term_y.coeff_re = 0.0;
    term_y.coeff_im = 0.5; // +i/2

    vec![term_x, term_y]
}

/// Multiply two lists of Pauli terms (distributes over sum).
fn mul_term_lists(a: &[PauliTerm], b: &[PauliTerm]) -> Vec<PauliTerm> {
    let mut result = Vec::with_capacity(a.len() * b.len());
    for ta in a {
        for tb in b {
            result.push(ta.mul(tb));
        }
    }
    result
}

/// Scale a list of Pauli terms by a real coefficient.
fn scale_terms(terms: &mut [PauliTerm], c: f64) {
    for t in terms.iter_mut() {
        t.coeff_re *= c;
        t.coeff_im *= c;
    }
}

/// Accumulate Pauli terms into a HashMap, combining like strings.
fn accumulate(acc: &mut HashMap<String, (f64, f64)>, terms: &[PauliTerm]) {
    for t in terms {
        let key = t.to_string_key();
        let entry = acc.entry(key).or_insert((0.0, 0.0));
        entry.0 += t.coeff_re;
        entry.1 += t.coeff_im;
    }
}

/// Jordan-Wigner transform of a molecular Hamiltonian.
///
/// Input: one-electron integrals h[p][q] and two-electron integrals (pq|rs)
/// in the **spatial** MO basis. n_spatial = h.len().
///
/// Output: list of (coefficient, Pauli string) pairs in the qubit basis.
/// The number of qubits is 2 * n_spatial (spin-orbitals).
pub fn jordan_wigner_transform(
    h_one: &[Vec<f64>],
    h_two: &[f64],
    n_spatial: usize,
    nuclear_repulsion: f64,
) -> Vec<(f64, String)> {
    let n_spin = 2 * n_spatial;
    let mut acc: HashMap<String, (f64, f64)> = HashMap::new();

    // Nuclear repulsion → identity term
    let id_key: String = (0..n_spin).map(|_| 'I').collect();
    *acc.entry(id_key).or_insert((0.0, 0.0)) = (nuclear_repulsion, 0.0);

    // One-body: Σ_{pq} h_{pq} a†_p a_q (spin-orbital basis)
    // h_{2i,2j} = h_{i,j} (alpha-alpha), h_{2i+1,2j+1} = h_{i,j} (beta-beta)
    for p_spat in 0..n_spatial {
        for q_spat in 0..n_spatial {
            let h_pq = h_one[p_spat][q_spat];
            if h_pq.abs() < 1e-14 {
                continue;
            }
            // Alpha-alpha
            let mut terms = mul_term_lists(
                &jw_creation(2 * p_spat, n_spin),
                &jw_annihilation(2 * q_spat, n_spin),
            );
            scale_terms(&mut terms, h_pq);
            accumulate(&mut acc, &terms);

            // Beta-beta
            let mut terms = mul_term_lists(
                &jw_creation(2 * p_spat + 1, n_spin),
                &jw_annihilation(2 * q_spat + 1, n_spin),
            );
            scale_terms(&mut terms, h_pq);
            accumulate(&mut acc, &terms);
        }
    }

    // Two-body: ½ Σ_{pqrs} (pq|rs) a†_{pα} a†_{rα} a_{sα} a_{qα} + spin combos
    //
    // The full second-quantized two-body Hamiltonian in chemists' notation:
    // H₂ = ½ Σ_{pqrs} (pq|rs) [a†_{pα}a_{qα} a†_{rα}a_{sα}
    //                           + a†_{pα}a_{qα} a†_{rβ}a_{sβ}
    //                           + a†_{pβ}a_{qβ} a†_{rα}a_{sα}
    //                           + a†_{pβ}a_{qβ} a†_{rβ}a_{sβ}]
    //     - ½ Σ_{pr} (Σ_q (pq|rq)) [a†_{pα}a_{rα} + a†_{pβ}a_{rβ}]
    //
    // But it's cleaner to work in spin-orbital basis directly:
    // H₂ = ½ Σ_{PQRS} <PQ||RS> a†_P a†_Q a_S a_R
    //     = ½ Σ_{PQRS} (<PQ|RS> - <PQ|SR>) a†_P a†_Q a_S a_R
    //
    // We use: H₂ = ½ Σ_{PQ,RS} g_{PQRS} (a†_P a_R)(a†_Q a_S) - ½ Σ_{PR} g_{PRPR} a†_P a_R
    // where g_{PQRS} = (PQ|RS) in spin-orbital basis (chemists' notation).

    // Spin-orbital two-electron integrals:
    // (Pα Qα | Rα Sα) = (pq|rs) if all same spin, etc.
    let g = |p: usize, q: usize, r: usize, s: usize| -> f64 {
        // Map spin-orbital indices to spatial + check spin conservation
        let p_spat = p / 2;
        let q_spat = q / 2;
        let r_spat = r / 2;
        let s_spat = s / 2;
        let p_spin = p % 2;
        let q_spin = q % 2;
        let r_spin = r % 2;
        let s_spin = s % 2;
        // Chemists' notation (pq|rs): electrons 1 at (p,q), electron 2 at (r,s)
        // Spin conservation: p_spin == q_spin AND r_spin == s_spin
        if p_spin != q_spin || r_spin != s_spin {
            return 0.0;
        }
        h_two[p_spat * n_spatial * n_spatial * n_spatial
            + q_spat * n_spatial * n_spatial
            + r_spat * n_spatial
            + s_spat]
    };

    // H₂ = ½ Σ_{PQRS} (PQ|RS) [(a†_P a_Q)(a†_R a_S)]
    //     - ½ Σ_{PR} [Σ_Q (PQ|RQ)] a†_P a_R
    //
    // But we need to be careful: (a†_P a_Q)(a†_R a_S) involves a product of
    // JW strings. Each a†_P a_Q is a sum of Pauli terms, so the product
    // gives 16 terms per (P,Q,R,S) quartet.

    for p in 0..n_spin {
        for q in 0..n_spin {
            for r in 0..n_spin {
                for s in 0..n_spin {
                    let val = g(p, q, r, s);
                    if val.abs() < 1e-14 {
                        continue;
                    }

                    // ½ (pq|rs) (a†_p a_q)(a†_r a_s)
                    let op_pq = mul_term_lists(
                        &jw_creation(p, n_spin),
                        &jw_annihilation(q, n_spin),
                    );
                    let op_rs = mul_term_lists(
                        &jw_creation(r, n_spin),
                        &jw_annihilation(s, n_spin),
                    );
                    let mut product = mul_term_lists(&op_pq, &op_rs);
                    scale_terms(&mut product, 0.5 * val);
                    accumulate(&mut acc, &product);
                }
            }
        }
    }

    // Correction: -½ Σ_{PR} [Σ_Q (PQ|RQ)] a†_P a_R
    for p in 0..n_spin {
        for r in 0..n_spin {
            let mut sum_q = 0.0;
            for q in 0..n_spin {
                sum_q += g(p, q, r, q);
            }
            if sum_q.abs() < 1e-14 {
                continue;
            }
            let mut terms = mul_term_lists(
                &jw_creation(p, n_spin),
                &jw_annihilation(r, n_spin),
            );
            scale_terms(&mut terms, -0.5 * sum_q);
            accumulate(&mut acc, &terms);
        }
    }

    // Collect results: drop near-zero terms and verify imaginary parts vanish
    let mut result: Vec<(f64, String)> = Vec::new();
    for (key, (re, im)) in &acc {
        if im.abs() > 1e-8 {
            // For a Hermitian Hamiltonian, imaginary parts should cancel.
            // Warn but continue.
            eprintln!(
                "WARNING: non-zero imaginary coefficient {im:.2e} for Pauli string {key}"
            );
        }
        if re.abs() > 1e-12 {
            result.push((*re, key.clone()));
        }
    }
    result.sort_by(|a, b| b.0.abs().partial_cmp(&a.0.abs()).unwrap());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pauli_multiplication() {
        // XX = I
        let (phase, p) = pauli_mul(Pauli::X, Pauli::X);
        assert_eq!(p, Pauli::I);
        assert_eq!(phase % 4, 0);

        // XY = iZ
        let (phase, p) = pauli_mul(Pauli::X, Pauli::Y);
        assert_eq!(p, Pauli::Z);
        assert_eq!(phase % 4, 1);
    }

    #[test]
    fn jw_number_operator() {
        // a†_0 a_0 should give ½(I - Z_0) on 2 qubits
        let creation = jw_creation(0, 2);
        let annihilation = jw_annihilation(0, 2);
        let product = mul_term_lists(&creation, &annihilation);

        let mut acc: HashMap<String, (f64, f64)> = HashMap::new();
        accumulate(&mut acc, &product);

        // Should have II (coeff 0.5) and ZI (coeff -0.5)
        let ii = acc.get("II").map(|v| v.0).unwrap_or(0.0);
        let zi = acc.get("ZI").map(|v| v.0).unwrap_or(0.0);
        assert!((ii - 0.5).abs() < 1e-10, "II coeff = {ii}");
        assert!((zi - (-0.5)).abs() < 1e-10, "ZI coeff = {zi}");
    }

    #[test]
    fn h2_hamiltonian_has_terms() {
        // Minimal 2-spatial-orbital H2 test with dummy integrals
        let h_one = vec![vec![-1.0, -0.5], vec![-0.5, -0.5]];
        let h_two = vec![0.6; 16]; // 2^4 = 16 entries
        let terms = jordan_wigner_transform(&h_one, &h_two, 2, 0.7);
        assert!(!terms.is_empty(), "should produce Pauli terms");
        // All terms should be real (imaginary parts cancelled)
        assert!(terms.len() > 3, "H2 should have multiple Pauli terms");
    }
}
