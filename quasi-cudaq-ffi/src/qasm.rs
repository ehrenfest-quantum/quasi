// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Minimal OpenQASM 2.0 parser for CUDA-Q circuit ingestion.
//!
//! Parses the restricted gate set that CUDA-Q emits through QIR lowering:
//! h, x, y, z, s, t, rx, ry, rz, cx, cz, ccx, measure.
//!
//! Does NOT handle custom gate definitions, classical control flow, or
//! include statements — CUDA-Q's QIR→QASM output does not use them.

/// A parsed gate operation.
#[derive(Debug, Clone, PartialEq)]
pub enum GateOp {
    H(usize),
    X(usize),
    Y(usize),
    Z(usize),
    S(usize),
    T(usize),
    Rx(f64, usize),
    Ry(f64, usize),
    Rz(f64, usize),
    Cx(usize, usize),
    Cz(usize, usize),
    Ccx(usize, usize, usize),
    Measure(usize, usize),
}

/// A parsed QASM circuit.
#[derive(Debug, Clone)]
pub struct Circuit {
    pub n_qubits: usize,
    pub n_clbits: usize,
    pub ops: Vec<GateOp>,
}

/// Parse error.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid qubit reference: {0}")]
    InvalidQubit(String),
    #[error("invalid gate syntax: {0}")]
    InvalidGate(String),
    #[error("no qreg declaration found")]
    NoQreg,
}

/// Parse an OpenQASM 2.0 string into a `Circuit`.
pub fn parse_qasm(qasm: &str) -> Result<Circuit, ParseError> {
    let mut n_qubits: Option<usize> = None;
    let mut n_clbits: usize = 0;
    let mut ops = Vec::new();

    for line in qasm.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("//")
            || line.starts_with("OPENQASM")
            || line.starts_with("include")
        {
            continue;
        }

        // Strip trailing semicolon
        let line = line.trim_end_matches(';').trim();

        if line.starts_with("qreg ") {
            n_qubits = Some(parse_reg_size(line)?);
        } else if line.starts_with("creg ") {
            n_clbits = parse_reg_size(line)?;
        } else if line.starts_with("barrier") {
            // Ignored — CUDA-Q barriers have no semantic effect for simulation
        } else if line.starts_with("measure") {
            let (q, c) = parse_measure(line)?;
            ops.push(GateOp::Measure(q, c));
        } else {
            if let Some(op) = parse_gate_line(line)? {
                ops.push(op);
            }
        }
    }

    let n_qubits = n_qubits.ok_or(ParseError::NoQreg)?;
    Ok(Circuit {
        n_qubits,
        n_clbits,
        ops,
    })
}

/// Parse a register declaration like `qreg q[5]` → 5.
fn parse_reg_size(line: &str) -> Result<usize, ParseError> {
    let bracket_start = line
        .find('[')
        .ok_or_else(|| ParseError::InvalidGate(line.to_string()))?;
    let bracket_end = line
        .find(']')
        .ok_or_else(|| ParseError::InvalidGate(line.to_string()))?;
    line[bracket_start + 1..bracket_end]
        .parse::<usize>()
        .map_err(|_| ParseError::InvalidGate(line.to_string()))
}

/// Parse a qubit reference like `q[3]` → 3.
fn parse_qubit(s: &str) -> Result<usize, ParseError> {
    let s = s.trim();
    let bracket_start = s
        .find('[')
        .ok_or_else(|| ParseError::InvalidQubit(s.to_string()))?;
    let bracket_end = s
        .find(']')
        .ok_or_else(|| ParseError::InvalidQubit(s.to_string()))?;
    s[bracket_start + 1..bracket_end]
        .parse::<usize>()
        .map_err(|_| ParseError::InvalidQubit(s.to_string()))
}

/// Parse `measure q[0] -> c[0]` → (0, 0).
fn parse_measure(line: &str) -> Result<(usize, usize), ParseError> {
    // "measure q[0] -> c[0]"
    let rest = line
        .strip_prefix("measure")
        .ok_or_else(|| ParseError::InvalidGate(line.to_string()))?
        .trim();
    let parts: Vec<&str> = rest.split("->").collect();
    if parts.len() != 2 {
        return Err(ParseError::InvalidGate(line.to_string()));
    }
    let q = parse_qubit(parts[0])?;
    let c = parse_qubit(parts[1])?;
    Ok((q, c))
}

/// Parse a gate line like `h q[0]`, `cx q[0],q[1]`, `rx(1.57) q[0]`.
fn parse_gate_line(line: &str) -> Result<Option<GateOp>, ParseError> {
    // Split gate name from arguments
    let (name, args) = if let Some(paren_pos) = line.find('(') {
        // Parametric gate: rx(1.57) q[0]
        let name = &line[..paren_pos];
        let rest = &line[paren_pos..];
        let close_paren = rest
            .find(')')
            .ok_or_else(|| ParseError::InvalidGate(line.to_string()))?;
        let angle_str = &rest[1..close_paren];
        let qubits_str = rest[close_paren + 1..].trim();
        let angle: f64 = angle_str
            .trim()
            .parse()
            .map_err(|_| ParseError::InvalidGate(line.to_string()))?;
        let qubits = parse_qubit_list(qubits_str)?;
        return match name.trim() {
            "rx" => {
                ensure_qubit_count(&qubits, 1, line)?;
                Ok(Some(GateOp::Rx(angle, qubits[0])))
            }
            "ry" => {
                ensure_qubit_count(&qubits, 1, line)?;
                Ok(Some(GateOp::Ry(angle, qubits[0])))
            }
            "rz" => {
                ensure_qubit_count(&qubits, 1, line)?;
                Ok(Some(GateOp::Rz(angle, qubits[0])))
            }
            _ => Err(ParseError::InvalidGate(line.to_string())),
        };
    } else {
        // Non-parametric gate: h q[0]  or  cx q[0],q[1]
        let first_space = line
            .find(|c: char| c.is_whitespace())
            .ok_or_else(|| ParseError::InvalidGate(line.to_string()))?;
        let name = line[..first_space].trim();
        let args = line[first_space..].trim();
        (name, args)
    };

    let qubits = parse_qubit_list(args)?;

    match name {
        "h" => {
            ensure_qubit_count(&qubits, 1, line)?;
            Ok(Some(GateOp::H(qubits[0])))
        }
        "x" => {
            ensure_qubit_count(&qubits, 1, line)?;
            Ok(Some(GateOp::X(qubits[0])))
        }
        "y" => {
            ensure_qubit_count(&qubits, 1, line)?;
            Ok(Some(GateOp::Y(qubits[0])))
        }
        "z" => {
            ensure_qubit_count(&qubits, 1, line)?;
            Ok(Some(GateOp::Z(qubits[0])))
        }
        "s" => {
            ensure_qubit_count(&qubits, 1, line)?;
            Ok(Some(GateOp::S(qubits[0])))
        }
        "t" => {
            ensure_qubit_count(&qubits, 1, line)?;
            Ok(Some(GateOp::T(qubits[0])))
        }
        "cx" | "CX" => {
            ensure_qubit_count(&qubits, 2, line)?;
            Ok(Some(GateOp::Cx(qubits[0], qubits[1])))
        }
        "cz" => {
            ensure_qubit_count(&qubits, 2, line)?;
            Ok(Some(GateOp::Cz(qubits[0], qubits[1])))
        }
        "ccx" => {
            ensure_qubit_count(&qubits, 3, line)?;
            Ok(Some(GateOp::Ccx(qubits[0], qubits[1], qubits[2])))
        }
        _ => {
            // Unknown gate — fail rather than skip. Silently dropping a gate
            // yields a circuit that is not the one the caller wrote, and the
            // wrong measurement statistics are returned with full confidence.
            // Parametric gates already fail this way; this keeps the two
            // paths consistent.
            Err(ParseError::InvalidGate(line.to_string()))
        }
    }
}

/// Parse a comma-separated list of qubit references: `q[0],q[1]` → [0, 1].
fn parse_qubit_list(s: &str) -> Result<Vec<usize>, ParseError> {
    s.split(',')
        .map(|part| parse_qubit(part.trim()))
        .collect()
}

fn ensure_qubit_count(qubits: &[usize], expected: usize, line: &str) -> Result<(), ParseError> {
    if qubits.len() != expected {
        return Err(ParseError::InvalidGate(format!(
            "expected {} qubit(s), got {}: {}",
            expected,
            qubits.len(),
            line
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bell_circuit() {
        let qasm = r#"
OPENQASM 2.0;
include "qelib1.inc";
qreg q[2];
creg c[2];
h q[0];
cx q[0],q[1];
measure q[0] -> c[0];
measure q[1] -> c[1];
"#;
        let circuit = parse_qasm(qasm).unwrap();
        assert_eq!(circuit.n_qubits, 2);
        assert_eq!(circuit.n_clbits, 2);
        assert_eq!(circuit.ops.len(), 4);
        assert_eq!(circuit.ops[0], GateOp::H(0));
        assert_eq!(circuit.ops[1], GateOp::Cx(0, 1));
        assert_eq!(circuit.ops[2], GateOp::Measure(0, 0));
        assert_eq!(circuit.ops[3], GateOp::Measure(1, 1));
    }

    #[test]
    fn parse_parametric_gates() {
        let qasm = r#"
OPENQASM 2.0;
qreg q[1];
creg c[1];
rx(1.5707963) q[0];
ry(3.14159) q[0];
rz(0.785) q[0];
"#;
        let circuit = parse_qasm(qasm).unwrap();
        assert_eq!(circuit.ops.len(), 3);
        match &circuit.ops[0] {
            GateOp::Rx(angle, 0) => assert!((angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6),
            other => panic!("expected Rx, got {:?}", other),
        }
    }

    #[test]
    fn parse_ghz_5() {
        let qasm = r#"
OPENQASM 2.0;
include "qelib1.inc";
qreg q[5];
creg c[5];
h q[0];
cx q[0],q[1];
cx q[1],q[2];
cx q[2],q[3];
cx q[3],q[4];
"#;
        let circuit = parse_qasm(qasm).unwrap();
        assert_eq!(circuit.n_qubits, 5);
        assert_eq!(circuit.ops.len(), 5);
    }

    #[test]
    fn parse_all_single_qubit_gates() {
        let qasm = r#"
OPENQASM 2.0;
qreg q[1];
h q[0];
x q[0];
y q[0];
z q[0];
s q[0];
t q[0];
"#;
        let circuit = parse_qasm(qasm).unwrap();
        assert_eq!(circuit.ops.len(), 6);
        assert_eq!(circuit.ops[0], GateOp::H(0));
        assert_eq!(circuit.ops[1], GateOp::X(0));
        assert_eq!(circuit.ops[2], GateOp::Y(0));
        assert_eq!(circuit.ops[3], GateOp::Z(0));
        assert_eq!(circuit.ops[4], GateOp::S(0));
        assert_eq!(circuit.ops[5], GateOp::T(0));
    }

    #[test]
    fn parse_barrier_ignored() {
        let qasm = r#"
OPENQASM 2.0;
qreg q[2];
h q[0];
barrier q[0],q[1];
cx q[0],q[1];
"#;
        let circuit = parse_qasm(qasm).unwrap();
        assert_eq!(circuit.ops.len(), 2);
    }

    #[test]
    fn unknown_gate_is_rejected_not_skipped() {
        // Silently dropping an unsupported gate would return confident but
        // wrong statistics for a circuit the caller never wrote.
        let qasm = "OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[2];\nsdg q[0];\n";
        assert!(parse_qasm(qasm).is_err());

        let qasm = "OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[2];\nswap q[0],q[1];\n";
        assert!(parse_qasm(qasm).is_err());

        let qasm = "OPENQASM 2.0;\ninclude \"qelib1.inc\";\nqreg q[1];\nreset q[0];\n";
        assert!(parse_qasm(qasm).is_err());
    }

    #[test]
    fn parse_no_qreg_fails() {
        let qasm = "OPENQASM 2.0;\nh q[0];";
        assert!(parse_qasm(qasm).is_err());
    }
}
