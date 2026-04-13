// SPDX-License-Identifier: AGPL-3.0-or-later
//! STO-3G basis set data (Hehre, Stewart, Pople, JCP 1969).
//!
//! Each atomic shell is a contraction of 3 primitive Gaussians.
//! Exponents and contraction coefficients are public-domain reference data.

use crate::error::BridgeError;

/// Angular momentum type for a basis shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    /// s-type (angular momentum 0)
    S,
    /// p-type (angular momentum 1): expands to px, py, pz
    P,
}

/// A contracted Gaussian shell.
#[derive(Debug, Clone)]
pub struct Shell {
    pub shell_type: ShellType,
    /// Primitive Gaussian exponents.
    pub exponents: Vec<f64>,
    /// Contraction coefficients (for normalised primitives).
    pub coefficients: Vec<f64>,
}

/// Basis set for a single atom.
#[derive(Debug, Clone)]
pub struct AtomBasis {
    pub atomic_number: u8,
    pub shells: Vec<Shell>,
}

/// A single basis function positioned at a centre.
#[derive(Debug, Clone)]
pub struct BasisFunction {
    /// Centre (bohr).
    pub center: [f64; 3],
    /// Cartesian angular momentum (l, m, n).
    pub angular: [usize; 3],
    /// Primitive exponents.
    pub exponents: Vec<f64>,
    /// Combined coefficients: contraction_coeff * normalisation.
    pub coefficients: Vec<f64>,
}

/// Normalization constant for a primitive Cartesian Gaussian
/// (x-Ax)^l (y-Ay)^m (z-Az)^n exp(-alpha |r-A|^2).
pub fn norm_primitive(alpha: f64, l: usize, m: usize, n: usize) -> f64 {
    let l_total = l + m + n;
    let num = (2.0 * alpha / std::f64::consts::PI).powf(0.75)
        * (4.0 * alpha).powi(l_total as i32).sqrt();
    let denom = (double_factorial(l) * double_factorial(m) * double_factorial(n))
        as f64;
    num / denom.sqrt()
}

/// (2n - 1)!! for non-negative n. Returns 1 for n=0.
fn double_factorial(n: usize) -> u64 {
    if n == 0 {
        return 1;
    }
    let val = (2 * n) as i64 - 1;
    if val <= 0 {
        return 1;
    }
    let mut result = 1u64;
    let mut k = val;
    while k > 0 {
        result *= k as u64;
        k -= 2;
    }
    result
}

/// Build the full list of basis functions for a molecule.
///
/// Each s-shell produces 1 function; each p-shell produces 3 (px, py, pz).
pub fn build_basis(atoms: &[(u8, [f64; 3])], basis_name: &str) -> Result<Vec<BasisFunction>, BridgeError> {
    if basis_name != "sto-3g" {
        return Err(BridgeError::UnsupportedBasis(basis_name.into()));
    }
    let mut functions = Vec::new();
    for &(z, center) in atoms {
        let atom_basis = sto3g_basis(z)?;
        for shell in &atom_basis.shells {
            match shell.shell_type {
                ShellType::S => {
                    let coeffs: Vec<f64> = shell
                        .exponents
                        .iter()
                        .zip(&shell.coefficients)
                        .map(|(&alpha, &d)| d * norm_primitive(alpha, 0, 0, 0))
                        .collect();
                    functions.push(BasisFunction {
                        center,
                        angular: [0, 0, 0],
                        exponents: shell.exponents.clone(),
                        coefficients: coeffs,
                    });
                }
                ShellType::P => {
                    for (li, mi, ni) in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
                        let coeffs: Vec<f64> = shell
                            .exponents
                            .iter()
                            .zip(&shell.coefficients)
                            .map(|(&alpha, &d)| d * norm_primitive(alpha, li, mi, ni))
                            .collect();
                        functions.push(BasisFunction {
                            center,
                            angular: [li, mi, ni],
                            exponents: shell.exponents.clone(),
                            coefficients: coeffs,
                        });
                    }
                }
            }
        }
    }
    Ok(functions)
}

// ── STO-3G basis data ──────────────────────────────────────────────────────

/// STO-3G contraction coefficients shared by all first-row 1s shells.
const STO3G_1S_COEFFS: [f64; 3] = [0.15432897, 0.53532814, 0.44463454];

/// STO-3G contraction coefficients for second-row 2s (SP shell, s-part).
const STO3G_2S_COEFFS: [f64; 3] = [-0.09996723, 0.39951283, 0.70011547];

/// STO-3G contraction coefficients for second-row 2p (SP shell, p-part).
const STO3G_2P_COEFFS: [f64; 3] = [0.15591627, 0.60768372, 0.39195739];

/// Get STO-3G basis for an element.
fn sto3g_basis(z: u8) -> Result<AtomBasis, BridgeError> {
    match z {
        1 => Ok(AtomBasis {
            atomic_number: 1,
            shells: vec![Shell {
                shell_type: ShellType::S,
                exponents: vec![3.42525091, 0.62353064, 0.16885540],
                coefficients: STO3G_1S_COEFFS.to_vec(),
            }],
        }),
        2 => Ok(AtomBasis {
            atomic_number: 2,
            shells: vec![Shell {
                shell_type: ShellType::S,
                exponents: vec![6.36242139, 1.15892300, 0.31364979],
                coefficients: STO3G_1S_COEFFS.to_vec(),
            }],
        }),
        6 => Ok(AtomBasis {
            atomic_number: 6,
            shells: vec![
                Shell {
                    shell_type: ShellType::S,
                    exponents: vec![71.6168370, 13.0450960, 3.5305122],
                    coefficients: STO3G_1S_COEFFS.to_vec(),
                },
                Shell {
                    shell_type: ShellType::S,
                    exponents: vec![2.9412494, 0.6834831, 0.2222899],
                    coefficients: STO3G_2S_COEFFS.to_vec(),
                },
                Shell {
                    shell_type: ShellType::P,
                    exponents: vec![2.9412494, 0.6834831, 0.2222899],
                    coefficients: STO3G_2P_COEFFS.to_vec(),
                },
            ],
        }),
        7 => Ok(AtomBasis {
            atomic_number: 7,
            shells: vec![
                Shell {
                    shell_type: ShellType::S,
                    exponents: vec![99.1061690, 18.0523120, 4.8856602],
                    coefficients: STO3G_1S_COEFFS.to_vec(),
                },
                Shell {
                    shell_type: ShellType::S,
                    exponents: vec![3.7804559, 0.8784966, 0.2857144],
                    coefficients: STO3G_2S_COEFFS.to_vec(),
                },
                Shell {
                    shell_type: ShellType::P,
                    exponents: vec![3.7804559, 0.8784966, 0.2857144],
                    coefficients: STO3G_2P_COEFFS.to_vec(),
                },
            ],
        }),
        8 => Ok(AtomBasis {
            atomic_number: 8,
            shells: vec![
                Shell {
                    shell_type: ShellType::S,
                    exponents: vec![130.7093200, 23.8088610, 6.4436083],
                    coefficients: STO3G_1S_COEFFS.to_vec(),
                },
                Shell {
                    shell_type: ShellType::S,
                    exponents: vec![5.0331513, 1.1695961, 0.3803890],
                    coefficients: STO3G_2S_COEFFS.to_vec(),
                },
                Shell {
                    shell_type: ShellType::P,
                    exponents: vec![5.0331513, 1.1695961, 0.3803890],
                    coefficients: STO3G_2P_COEFFS.to_vec(),
                },
            ],
        }),
        9 => Ok(AtomBasis {
            atomic_number: 9,
            shells: vec![
                Shell {
                    shell_type: ShellType::S,
                    exponents: vec![166.6791340, 30.3608120, 8.2168207],
                    coefficients: STO3G_1S_COEFFS.to_vec(),
                },
                Shell {
                    shell_type: ShellType::S,
                    exponents: vec![6.4648032, 1.5022812, 0.4885885],
                    coefficients: STO3G_2S_COEFFS.to_vec(),
                },
                Shell {
                    shell_type: ShellType::P,
                    exponents: vec![6.4648032, 1.5022812, 0.4885885],
                    coefficients: STO3G_2P_COEFFS.to_vec(),
                },
            ],
        }),
        _ => Err(BridgeError::NoBasisForElement(z)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h2_basis_has_2_functions() {
        let atoms = vec![(1u8, [0.0, 0.0, 0.0]), (1, [0.0, 0.0, 1.4])];
        let basis = build_basis(&atoms, "sto-3g").unwrap();
        assert_eq!(basis.len(), 2);
        assert_eq!(basis[0].angular, [0, 0, 0]);
        assert_eq!(basis[1].angular, [0, 0, 0]);
    }

    #[test]
    fn water_basis_has_7_functions() {
        let atoms = vec![
            (8u8, [0.0, 0.0, 0.0]),
            (1, [0.0, 1.43, -1.10]),
            (1, [0.0, -1.43, -1.10]),
        ];
        let basis = build_basis(&atoms, "sto-3g").unwrap();
        // O: 1s + 2s + 2px + 2py + 2pz = 5, H: 1s each = 2, total = 7
        assert_eq!(basis.len(), 7);
    }

    #[test]
    fn norm_s_primitive() {
        // Normalization: integral of |g|^2 should be 1.
        // For s-type: N^2 * (pi / (2*alpha))^{3/2} = 1
        // So N = (2*alpha/pi)^{3/4}
        let alpha = 1.0;
        let n = norm_primitive(alpha, 0, 0, 0);
        let expected = (2.0 * alpha / std::f64::consts::PI).powf(0.75);
        assert!((n - expected).abs() < 1e-12);
    }

    #[test]
    fn double_factorial_values() {
        assert_eq!(double_factorial(0), 1);
        assert_eq!(double_factorial(1), 1);
        assert_eq!(double_factorial(2), 3);   // (2*2-1)!! = 3!! = 3
        assert_eq!(double_factorial(3), 15);  // 5!! = 5*3*1 = 15
    }

    #[test]
    fn unsupported_basis_errors() {
        let atoms = vec![(1u8, [0.0; 3])];
        assert!(build_basis(&atoms, "cc-pvdz").is_err());
    }
}
