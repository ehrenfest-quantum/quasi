// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gaussian-type orbital integral engine (McMurchie-Davidson scheme).
//!
//! Computes one- and two-electron integrals for s and p shells:
//! - Overlap (S)
//! - Kinetic energy (T)
//! - Nuclear attraction (V)
//! - Electron repulsion integrals (ERI)
//!
//! Reference: McMurchie & Davidson, JCP 1978; Helgaker, Jorgensen & Olsen,
//! "Molecular Electronic-Structure Theory", Ch. 9.

use crate::basis::BasisFunction;

// ── Boys function ──────────────────────────────────────────────────────────

/// Boys function F_n(x) = ∫₀¹ t^{2n} exp(-x t²) dt.
///
/// Uses series expansion for x < 25, asymptotic formula for x ≥ 25.
pub fn boys(n: usize, x: f64) -> f64 {
    if x < 1e-14 {
        return 1.0 / (2 * n + 1) as f64;
    }
    if x < 25.0 {
        // Series: F_n(x) = Σ_{k=0}^∞ (-x)^k / (k! (2n+2k+1))
        // term accumulates (-x)^k / k!, contrib adds the (2n+2k+1) denominator
        let mut term = 1.0; // (-x)^0 / 0! = 1
        let mut sum = 1.0 / (2 * n + 1) as f64;
        for k in 1..=60 {
            term *= -x / k as f64;
            let contrib = term / (2 * n + 2 * k + 1) as f64;
            sum += contrib;
            if contrib.abs() < 1e-15 * sum.abs() {
                break;
            }
        }
        sum
    } else {
        // Asymptotic: F_n(x) ≈ (2n-1)!! / 2^{n+1} * sqrt(π / x^{2n+1})
        // Start with F_0(x) ≈ ½ sqrt(π/x), then downward recursion.
        let max_n = n + 6; // start high for stability
        // F_n(x) ≈ (2n-1)!! √π / (2^{n+1} x^{n+1/2})
        let mut f = double_fact_ratio(max_n) * (std::f64::consts::PI / x).sqrt()
            / (2.0 * (2.0 * x).powi(max_n as i32));
        let exp_neg_x = (-x).exp();
        // Downward recursion: F_{n-1}(x) = (2x F_n(x) + exp(-x)) / (2n-1)
        for m in (1..=max_n).rev() {
            f = (2.0 * x * f + exp_neg_x) / (2 * m - 1) as f64;
            if m - 1 == n {
                return f;
            }
        }
        f
    }
}

/// (2n-1)!! as f64. Returns 1.0 for n=0.
fn double_fact_ratio(n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let mut r = 1.0;
    for k in (1..=(2 * n as i64 - 1)).rev().step_by(2) {
        r *= k as f64;
    }
    r
}

// ── Hermite expansion coefficients ─────────────────────────────────────────

/// McMurchie-Davidson Hermite expansion coefficient E_t^{ij}.
///
/// Recursive definition:
///   E_0^{00} = 1
///   E_t^{i+1,j} = (1/2p) E_{t-1}^{ij} + XPA E_t^{ij} + (t+1) E_{t+1}^{ij}
///   E_t^{i,j+1} = (1/2p) E_{t-1}^{ij} + XPB E_t^{ij} + (t+1) E_{t+1}^{ij}
fn hermite_e(i: i32, j: i32, t: i32, xpa: f64, xpb: f64, inv2p: f64) -> f64 {
    if t < 0 || t > i + j {
        return 0.0;
    }
    if i == 0 && j == 0 {
        return if t == 0 { 1.0 } else { 0.0 };
    }
    if i > 0 {
        inv2p * hermite_e(i - 1, j, t - 1, xpa, xpb, inv2p)
            + xpa * hermite_e(i - 1, j, t, xpa, xpb, inv2p)
            + (t + 1) as f64 * hermite_e(i - 1, j, t + 1, xpa, xpb, inv2p)
    } else {
        inv2p * hermite_e(i, j - 1, t - 1, xpa, xpb, inv2p)
            + xpb * hermite_e(i, j - 1, t, xpa, xpb, inv2p)
            + (t + 1) as f64 * hermite_e(i, j - 1, t + 1, xpa, xpb, inv2p)
    }
}

// ── R auxiliary integrals (Coulomb) ────────────────────────────────────────

/// Coulomb auxiliary integral R_{tuv}^n used for nuclear attraction and ERI.
///
/// Recursion:
///   R_{000}^n = (-2p)^n F_n(p |RPC|²)
///   R_{t+1,u,v}^n = t R_{t-1,u,v}^{n+1} + XPC R_{t,u,v}^{n+1}
fn r_aux(t: i32, u: i32, v: i32, n: usize, p: f64, rpc: [f64; 3]) -> f64 {
    if t < 0 || u < 0 || v < 0 {
        return 0.0;
    }
    if t == 0 && u == 0 && v == 0 {
        let rpc_sq = rpc[0] * rpc[0] + rpc[1] * rpc[1] + rpc[2] * rpc[2];
        return (-2.0 * p).powi(n as i32) * boys(n, p * rpc_sq);
    }
    if t > 0 {
        (t - 1) as f64 * r_aux(t - 2, u, v, n + 1, p, rpc)
            + rpc[0] * r_aux(t - 1, u, v, n + 1, p, rpc)
    } else if u > 0 {
        (u - 1) as f64 * r_aux(t, u - 2, v, n + 1, p, rpc)
            + rpc[1] * r_aux(t, u - 1, v, n + 1, p, rpc)
    } else {
        (v - 1) as f64 * r_aux(t, u, v - 2, n + 1, p, rpc)
            + rpc[2] * r_aux(t, u, v - 1, n + 1, p, rpc)
    }
}

// ── Primitive integrals ────────────────────────────────────────────────────

/// Gaussian product centre and prefactor for primitives (alpha, A) and (beta, B).
struct GaussianProduct {
    p: f64,         // alpha + beta
    inv2p: f64,     // 1 / (2p)
    #[allow(dead_code)]
    mu: f64,        // alpha * beta / p
    center: [f64; 3], // P = (alpha*A + beta*B) / p
    pa: [f64; 3],   // P - A
    pb: [f64; 3],   // P - B
    k: f64,         // exp(-mu * |A-B|^2)
}

fn gaussian_product(alpha: f64, a: [f64; 3], beta: f64, b: [f64; 3]) -> GaussianProduct {
    let p = alpha + beta;
    let mu = alpha * beta / p;
    let ab_sq = (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let center = [
        (alpha * a[0] + beta * b[0]) / p,
        (alpha * a[1] + beta * b[1]) / p,
        (alpha * a[2] + beta * b[2]) / p,
    ];
    GaussianProduct {
        p,
        inv2p: 0.5 / p,
        mu,
        center,
        pa: [center[0] - a[0], center[1] - a[1], center[2] - a[2]],
        pb: [center[0] - b[0], center[1] - b[1], center[2] - b[2]],
        k: (-mu * ab_sq).exp(),
    }
}

/// One primitive Gaussian shell: exponent, centre, and angular momentum.
struct PrimShell {
    alpha: f64,
    center: [f64; 3],
    ang: [i32; 3],
}

/// Overlap integral between two unnormalised primitive Gaussians.
fn overlap_prim(
    alpha: f64, a: [f64; 3], la: [i32; 3],
    beta: f64, b: [f64; 3], lb: [i32; 3],
) -> f64 {
    let gp = gaussian_product(alpha, a, beta, b);
    let pre = gp.k * (std::f64::consts::PI / gp.p).powf(1.5);
    let ex = hermite_e(la[0], lb[0], 0, gp.pa[0], gp.pb[0], gp.inv2p);
    let ey = hermite_e(la[1], lb[1], 0, gp.pa[1], gp.pb[1], gp.inv2p);
    let ez = hermite_e(la[2], lb[2], 0, gp.pa[2], gp.pb[2], gp.inv2p);
    pre * ex * ey * ez
}

/// Kinetic energy integral between two unnormalised primitive Gaussians.
///
/// T(a,b) = -½ ∫ g_a ∇² g_b dr
///        = -½ Σ_d [l_d(l_d-1) S(a,b_{d-2}) - 2β(2l_d+1) S(a,b) + 4β² S(a,b_{d+2})]
fn kinetic_prim(
    alpha: f64, a: [f64; 3], la: [i32; 3],
    beta: f64, b: [f64; 3], lb: [i32; 3],
) -> f64 {
    let s_ab = overlap_prim(alpha, a, la, beta, b, lb);
    let mut raw = 0.0;
    for d in 0..3usize {
        let ld = lb[d];
        // First term: l_d(l_d-1) S(a, b_{d-2})
        if ld >= 2 {
            let mut lb_m = lb;
            lb_m[d] -= 2;
            raw += (ld * (ld - 1)) as f64 * overlap_prim(alpha, a, la, beta, b, lb_m);
        }
        // Middle term: -2β(2l_d+1) S(a,b)
        raw -= 2.0 * beta * (2 * ld + 1) as f64 * s_ab;
        // Last term: 4β² S(a, b_{d+2})
        let mut lb_p = lb;
        lb_p[d] += 2;
        raw += 4.0 * beta * beta * overlap_prim(alpha, a, la, beta, b, lb_p);
    }
    -0.5 * raw
}

/// Nuclear attraction integral for one nucleus at C with charge Z.
///
/// V(a,b|C) = -Z K (2π/p) Σ_{tuv} E_t E_u E_v R_{tuv}^0
fn nuclear_prim(a: PrimShell, b: PrimShell, c: [f64; 3], z_charge: f64) -> f64 {
    let gp = gaussian_product(a.alpha, a.center, b.alpha, b.center);
    let rpc = [
        gp.center[0] - c[0],
        gp.center[1] - c[1],
        gp.center[2] - c[2],
    ];
    let pre = -z_charge * gp.k * 2.0 * std::f64::consts::PI / gp.p;
    let mut val = 0.0;
    for t in 0..=(a.ang[0] + b.ang[0]) {
        let et = hermite_e(a.ang[0], b.ang[0], t, gp.pa[0], gp.pb[0], gp.inv2p);
        if et.abs() < 1e-16 { continue; }
        for u in 0..=(a.ang[1] + b.ang[1]) {
            let eu = hermite_e(a.ang[1], b.ang[1], u, gp.pa[1], gp.pb[1], gp.inv2p);
            if eu.abs() < 1e-16 { continue; }
            for v in 0..=(a.ang[2] + b.ang[2]) {
                let ev = hermite_e(a.ang[2], b.ang[2], v, gp.pa[2], gp.pb[2], gp.inv2p);
                if ev.abs() < 1e-16 { continue; }
                val += et * eu * ev * r_aux(t, u, v, 0, gp.p, rpc);
            }
        }
    }
    pre * val
}

/// Electron repulsion integral (ab|cd) between four unnormalised primitives.
///
/// (ab|cd) = K_AB K_CD (2π^{5/2})/(pq√(p+q))
///           × Σ_{tuv,τυφ} E^{ab}_t E^{ab}_u E^{ab}_v
///                          × E^{cd}_τ E^{cd}_υ E^{cd}_φ
///                          × (-1)^{τ+υ+φ} R_{t+τ, u+υ, v+φ}(α, P-Q)
fn eri_prim(a: PrimShell, b: PrimShell, c: PrimShell, d: PrimShell) -> f64 {
    let gp1 = gaussian_product(a.alpha, a.center, b.alpha, b.center);
    let gp2 = gaussian_product(c.alpha, c.center, d.alpha, d.center);
    let p = gp1.p;
    let q = gp2.p;
    let alpha_g = p * q / (p + q);
    let rpq = [
        gp1.center[0] - gp2.center[0],
        gp1.center[1] - gp2.center[1],
        gp1.center[2] - gp2.center[2],
    ];
    let pre = gp1.k * gp2.k * 2.0 * std::f64::consts::PI.powf(2.5)
        / (p * q * (p + q).sqrt());

    let mut val = 0.0;
    for t in 0..=(a.ang[0] + b.ang[0]) {
        let et = hermite_e(a.ang[0], b.ang[0], t, gp1.pa[0], gp1.pb[0], gp1.inv2p);
        if et.abs() < 1e-16 { continue; }
        for u in 0..=(a.ang[1] + b.ang[1]) {
            let eu = hermite_e(a.ang[1], b.ang[1], u, gp1.pa[1], gp1.pb[1], gp1.inv2p);
            if eu.abs() < 1e-16 { continue; }
            for v in 0..=(a.ang[2] + b.ang[2]) {
                let ev = hermite_e(a.ang[2], b.ang[2], v, gp1.pa[2], gp1.pb[2], gp1.inv2p);
                if ev.abs() < 1e-16 { continue; }
                for tau in 0..=(c.ang[0] + d.ang[0]) {
                    let e_tau = hermite_e(c.ang[0], d.ang[0], tau, gp2.pa[0], gp2.pb[0], gp2.inv2p);
                    if e_tau.abs() < 1e-16 { continue; }
                    for nu in 0..=(c.ang[1] + d.ang[1]) {
                        let e_nu = hermite_e(c.ang[1], d.ang[1], nu, gp2.pa[1], gp2.pb[1], gp2.inv2p);
                        if e_nu.abs() < 1e-16 { continue; }
                        for phi in 0..=(c.ang[2] + d.ang[2]) {
                            let e_phi = hermite_e(c.ang[2], d.ang[2], phi, gp2.pa[2], gp2.pb[2], gp2.inv2p);
                            if e_phi.abs() < 1e-16 { continue; }
                            let sign = if (tau + nu + phi) % 2 == 0 { 1.0 } else { -1.0 };
                            val += et * eu * ev * e_tau * e_nu * e_phi * sign
                                * r_aux(t + tau, u + nu, v + phi, 0, alpha_g, rpq);
                        }
                    }
                }
            }
        }
    }
    pre * val
}

// ── Contracted integrals ───────────────────────────────────────────────────

fn angular_i32(bf: &BasisFunction) -> [i32; 3] {
    [bf.angular[0] as i32, bf.angular[1] as i32, bf.angular[2] as i32]
}

/// Contracted overlap integral S_μν.
pub fn overlap(a: &BasisFunction, b: &BasisFunction) -> f64 {
    let la = angular_i32(a);
    let lb = angular_i32(b);
    let mut val = 0.0;
    for (k, &alpha) in a.exponents.iter().enumerate() {
        for (l, &beta) in b.exponents.iter().enumerate() {
            val += a.coefficients[k] * b.coefficients[l]
                * overlap_prim(alpha, a.center, la, beta, b.center, lb);
        }
    }
    val
}

/// Contracted kinetic energy integral T_μν.
pub fn kinetic(a: &BasisFunction, b: &BasisFunction) -> f64 {
    let la = angular_i32(a);
    let lb = angular_i32(b);
    let mut val = 0.0;
    for (k, &alpha) in a.exponents.iter().enumerate() {
        for (l, &beta) in b.exponents.iter().enumerate() {
            val += a.coefficients[k] * b.coefficients[l]
                * kinetic_prim(alpha, a.center, la, beta, b.center, lb);
        }
    }
    val
}

/// Contracted nuclear attraction integral V_μν = Σ_C V_μν^C.
pub fn nuclear(a: &BasisFunction, b: &BasisFunction, nuclei: &[(u8, [f64; 3])]) -> f64 {
    let la = angular_i32(a);
    let lb = angular_i32(b);
    let mut val = 0.0;
    for &(z, c) in nuclei {
        let z_f = z as f64;
        for (k, &alpha) in a.exponents.iter().enumerate() {
            for (l, &beta) in b.exponents.iter().enumerate() {
                val += a.coefficients[k] * b.coefficients[l]
                    * nuclear_prim(
                        PrimShell { alpha, center: a.center, ang: la },
                        PrimShell { alpha: beta, center: b.center, ang: lb },
                        c,
                        z_f,
                    );
            }
        }
    }
    val
}

/// Contracted electron repulsion integral (μν|λσ).
pub fn eri(a: &BasisFunction, b: &BasisFunction, c: &BasisFunction, d: &BasisFunction) -> f64 {
    let la = angular_i32(a);
    let lb = angular_i32(b);
    let lc = angular_i32(c);
    let ld = angular_i32(d);
    let mut val = 0.0;
    for (i, &ai) in a.exponents.iter().enumerate() {
        for (j, &bj) in b.exponents.iter().enumerate() {
            for (k, &ck) in c.exponents.iter().enumerate() {
                for (l, &dl) in d.exponents.iter().enumerate() {
                    val += a.coefficients[i]
                        * b.coefficients[j]
                        * c.coefficients[k]
                        * d.coefficients[l]
                        * eri_prim(
                            PrimShell { alpha: ai, center: a.center, ang: la },
                            PrimShell { alpha: bj, center: b.center, ang: lb },
                            PrimShell { alpha: ck, center: c.center, ang: lc },
                            PrimShell { alpha: dl, center: d.center, ang: ld },
                        );
                }
            }
        }
    }
    val
}

// ── Full molecular integrals ───────────────────────────────────────────────

/// All molecular integrals needed for RHF.
pub struct MolecularIntegrals {
    pub n_basis: usize,
    /// Overlap matrix S (n×n).
    pub overlap: Vec<Vec<f64>>,
    /// Kinetic energy matrix T (n×n).
    pub kinetic: Vec<Vec<f64>>,
    /// Nuclear attraction matrix V (n×n).
    pub nuclear: Vec<Vec<f64>>,
    /// Two-electron integrals (μν|λσ) stored as 4D array.
    pub eri: Vec<f64>,
    /// Nuclear repulsion energy.
    pub nuclear_repulsion: f64,
}

impl MolecularIntegrals {
    /// Access ERI element (μν|λσ).
    pub fn eri(&self, mu: usize, nu: usize, lam: usize, sig: usize) -> f64 {
        let n = self.n_basis;
        self.eri[mu * n * n * n + nu * n * n + lam * n + sig]
    }
}

/// Compute all molecular integrals.
pub fn compute_integrals(
    atoms: &[(u8, [f64; 3])],
    basis: &[BasisFunction],
) -> MolecularIntegrals {
    let n = basis.len();

    // One-electron integrals
    let mut s_mat = vec![vec![0.0; n]; n];
    let mut t_mat = vec![vec![0.0; n]; n];
    let mut v_mat = vec![vec![0.0; n]; n];

    for mu in 0..n {
        for nu in mu..n {
            let s = overlap(&basis[mu], &basis[nu]);
            let t = kinetic(&basis[mu], &basis[nu]);
            let v = nuclear(&basis[mu], &basis[nu], atoms);
            s_mat[mu][nu] = s;
            s_mat[nu][mu] = s;
            t_mat[mu][nu] = t;
            t_mat[nu][mu] = t;
            v_mat[mu][nu] = v;
            v_mat[nu][mu] = v;
        }
    }

    // Two-electron integrals with 8-fold symmetry
    let mut eri_arr = vec![0.0; n * n * n * n];
    for mu in 0..n {
        for nu in 0..n {
            for lam in 0..n {
                for sig in 0..n {
                    // Use 4-fold permutation symmetry: (μν|λσ) = (νμ|λσ) = (μν|σλ) = (λσ|μν)
                    let idx = mu * n * n * n + nu * n * n + lam * n + sig;
                    if eri_arr[idx] != 0.0 {
                        continue;
                    }
                    let val = eri(&basis[mu], &basis[nu], &basis[lam], &basis[sig]);
                    // Fill symmetry-related elements
                    let perms = [
                        (mu, nu, lam, sig),
                        (nu, mu, lam, sig),
                        (mu, nu, sig, lam),
                        (nu, mu, sig, lam),
                        (lam, sig, mu, nu),
                        (sig, lam, mu, nu),
                        (lam, sig, nu, mu),
                        (sig, lam, nu, mu),
                    ];
                    for (a, b, c, d) in perms {
                        eri_arr[a * n * n * n + b * n * n + c * n + d] = val;
                    }
                }
            }
        }
    }

    // Nuclear repulsion
    let mut e_nuc = 0.0;
    for i in 0..atoms.len() {
        for j in (i + 1)..atoms.len() {
            let (zi, ri) = atoms[i];
            let (zj, rj) = atoms[j];
            let r = ((ri[0] - rj[0]).powi(2) + (ri[1] - rj[1]).powi(2) + (ri[2] - rj[2]).powi(2))
                .sqrt();
            e_nuc += (zi as f64) * (zj as f64) / r;
        }
    }

    MolecularIntegrals {
        n_basis: n,
        overlap: s_mat,
        kinetic: t_mat,
        nuclear: v_mat,
        eri: eri_arr,
        nuclear_repulsion: e_nuc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boys_f0_zero() {
        assert!((boys(0, 0.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn boys_f0_known() {
        // F_0(1) = √π/2 erf(1) ≈ 0.7468
        let val = boys(0, 1.0);
        assert!((val - 0.746824133).abs() < 1e-6, "F_0(1) = {val}");
    }

    #[test]
    fn boys_large_x() {
        // F_0(100) ≈ ½ √(π/100) ≈ 0.08862
        let val = boys(0, 100.0);
        let expected = 0.5 * (std::f64::consts::PI / 100.0).sqrt();
        assert!((val - expected).abs() < 1e-6, "F_0(100) = {val}, expected {expected}");
    }

    #[test]
    fn hermite_e00_is_one() {
        assert!((hermite_e(0, 0, 0, 0.5, -0.3, 0.25) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn h2_overlap_diagonal() {
        // Self-overlap of a normalised contracted s-function should be ~1.
        let atoms = vec![(1u8, [0.0, 0.0, 0.0]), (1, [0.0, 0.0, 1.4])];
        let basis = crate::basis::build_basis(&atoms, "sto-3g").unwrap();
        let s00 = overlap(&basis[0], &basis[0]);
        assert!(
            (s00 - 1.0).abs() < 0.01,
            "S(0,0) should be ~1.0, got {s00}"
        );
    }

    #[test]
    fn water_overlap_diagonal() {
        // All basis function self-overlaps should be 1.0
        let atoms = crate::molecule::from_smiles("O").unwrap();
        let basis = crate::basis::build_basis(&atoms, "sto-3g").unwrap();
        for (i, bf) in basis.iter().enumerate() {
            let s = overlap(bf, bf);
            assert!(
                (s - 1.0).abs() < 0.02,
                "S({i},{i}) = {s:.6}, angular={:?}, expected ~1.0",
                bf.angular
            );
        }
    }

    #[test]
    fn boys_higher_order() {
        // Verify F_1 and F_2 using the recurrence: F_{n+1} = [(2n+1)F_n - exp(-x)]/(2x)
        let x = 2.5;
        let f0 = boys(0, x);
        let f1 = boys(1, x);
        let f2 = boys(2, x);
        let f1_from_f0 = (1.0 * f0 - (-x).exp()) / (2.0 * x);
        let f2_from_f1 = (3.0 * f1 - (-x).exp()) / (2.0 * x);
        assert!(
            (f1 - f1_from_f0).abs() < 1e-10,
            "F_1 recurrence: {f1} vs {f1_from_f0}"
        );
        assert!(
            (f2 - f2_from_f1).abs() < 1e-10,
            "F_2 recurrence: {f2} vs {f2_from_f1}"
        );
    }

    #[test]
    fn water_kinetic_reference() {
        // Kinetic energy T(O 1s, O 1s) for STO-3G oxygen should be ~29.0 Ha
        let atoms = crate::molecule::from_smiles("O").unwrap();
        let basis = crate::basis::build_basis(&atoms, "sto-3g").unwrap();
        let t00 = kinetic(&basis[0], &basis[0]);
        // Reference: Szabo & Ostlund, the 1s kinetic for O/STO-3G is ~29.0
        eprintln!("T(O 1s, O 1s) = {t00:.6}");
        assert!(t00 > 20.0 && t00 < 40.0, "T(O 1s) = {t00}");

        // Check p-orbital kinetic (should be positive)
        let t22 = kinetic(&basis[2], &basis[2]); // O 2px
        eprintln!("T(O 2px, O 2px) = {t22:.6}");
        assert!(t22 > 0.0, "T(O 2px) should be positive, got {t22}");

        // Nuclear attraction (should be very negative)
        let v00 = nuclear(&basis[0], &basis[0], &atoms);
        eprintln!("V(O 1s, O 1s) = {v00:.6}");
        assert!(v00 < -50.0, "V(O 1s) should be very negative, got {v00}");

        let v22 = nuclear(&basis[2], &basis[2], &atoms);
        eprintln!("V(O 2px, O 2px) = {v22:.6}");
    }

    #[test]
    fn h2_integrals_szabo_ostlund() {
        // Reference: Szabo & Ostlund, Table 3.11 (H2/STO-3G at R=1.4 bohr)
        let atoms = vec![(1u8, [0.0, 0.0, 0.0]), (1, [0.0, 0.0, 1.4])];
        let basis = crate::basis::build_basis(&atoms, "sto-3g").unwrap();
        let ints = compute_integrals(&atoms, &basis);

        // Core Hamiltonian: H = T + V
        let h11 = ints.kinetic[0][0] + ints.nuclear[0][0];
        let h12 = ints.kinetic[0][1] + ints.nuclear[0][1];
        eprintln!("H(1,1) = {h11:.4}, expected -1.1204");
        eprintln!("H(1,2) = {h12:.4}, expected -0.9584");
        assert!((h11 - (-1.1204)).abs() < 0.01, "H(1,1) = {h11}");
        assert!((h12 - (-0.9584)).abs() < 0.01, "H(1,2) = {h12}");

        // Overlap
        let s12 = ints.overlap[0][1];
        eprintln!("S(1,2) = {s12:.4}, expected 0.6593");
        assert!((s12 - 0.6593).abs() < 0.01, "S(1,2) = {s12}");

        // Two-electron integrals
        let eri_1111 = ints.eri(0, 0, 0, 0);
        let eri_1122 = ints.eri(0, 0, 1, 1);
        let eri_1112 = ints.eri(0, 0, 0, 1);
        let eri_1212 = ints.eri(0, 1, 0, 1);
        eprintln!("(11|11) = {eri_1111:.4}, expected 0.7746");
        eprintln!("(11|22) = {eri_1122:.4}, expected 0.5697");
        eprintln!("(11|12) = {eri_1112:.4}, expected 0.4441");
        eprintln!("(12|12) = {eri_1212:.4}, expected 0.2970");
        assert!((eri_1111 - 0.7746).abs() < 0.01, "(11|11) = {eri_1111}");
        assert!((eri_1122 - 0.5697).abs() < 0.01, "(11|22) = {eri_1122}");
    }

    #[test]
    fn h2_nuclear_repulsion() {
        let atoms = vec![(1u8, [0.0, 0.0, 0.0]), (1, [0.0, 0.0, 1.4])];
        let basis = crate::basis::build_basis(&atoms, "sto-3g").unwrap();
        let ints = compute_integrals(&atoms, &basis);
        // E_nuc = 1/1.4 ≈ 0.7143
        assert!(
            (ints.nuclear_repulsion - 1.0 / 1.4).abs() < 1e-10,
            "E_nuc = {}",
            ints.nuclear_repulsion
        );
    }
}
