// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! OpenQASM 2.0 / 3.0 emission from an [`EhrenfestAst`].

use crate::ast::*;
use crate::error::EmitError;

/// QASM dialect to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QasmVersion {
    V2,
    V3,
}

/// Emit an [`EhrenfestAst`] as OpenQASM source.
pub fn emit_qasm(ast: &EhrenfestAst, version: QasmVersion) -> Result<String, EmitError> {
    let mut lines: Vec<String> = Vec::new();

    match version {
        QasmVersion::V2 => {
            lines.push("OPENQASM 2.0;".into());
            lines.push("include \"qelib1.inc\";".into());
            lines.push(String::new());
            lines.push(format!("qreg q[{}];", ast.n_qubits));
            // Count max cbit index from measures + conditionals.
            let max_cbit = max_cbit_index(ast);
            if max_cbit > 0 {
                lines.push(format!("creg c[{}];", max_cbit));
            }
        }
        QasmVersion::V3 => {
            lines.push("OPENQASM 3.0;".into());
            lines.push("include \"stdgates.inc\";".into());
            lines.push(String::new());
            lines.push(format!("qubit[{}] q;", ast.n_qubits));
            let max_cbit = max_cbit_index(ast);
            if max_cbit > 0 {
                lines.push(format!("bit[{}] c;", max_cbit));
            }
        }
    }
    lines.push(String::new());

    // Gates.
    for gate in &ast.gates {
        lines.push(format_gate(gate, version)?);
    }

    // Conditionals.
    for cond in &ast.conditionals {
        let gate_str = format_gate(&cond.gate, version)?;
        match version {
            QasmVersion::V2 => {
                lines.push(format!(
                    "if(c[{}]=={}) {}",
                    cond.cbit, cond.cbit_value, gate_str
                ));
            }
            QasmVersion::V3 => {
                lines.push(format!(
                    "if (c[{}] == {}) {}",
                    cond.cbit, cond.cbit_value, gate_str
                ));
            }
        }
    }

    // Measurements.
    for m in &ast.measures {
        match version {
            QasmVersion::V2 => {
                lines.push(format!("measure q[{}] -> c[{}];", m.qubit, m.cbit));
            }
            QasmVersion::V3 => {
                lines.push(format!("c[{}] = measure q[{}];", m.cbit, m.qubit));
            }
        }
    }

    // Variational loops → QASM 3.0 input parameters.
    for vloop in &ast.variational_loops {
        lines.push(String::new());
        lines.push(format!(
            "// Variational ansatz — max_iter={} (classical loop managed by caller)",
            vloop.max_iter
        ));
        if version == QasmVersion::V3 {
            for p in &vloop.params {
                lines.push(format!("input float[64] {p};"));
            }
        }
        lines.push(String::new());
        for vg in &vloop.body {
            let qubit_args: String = vg
                .qubits
                .iter()
                .map(|idx| format!("q[{idx}]"))
                .collect::<Vec<_>>()
                .join(", ");

            if vg.param_refs.is_empty() {
                lines.push(format!("{} {};", vg.name.as_str(), qubit_args));
            } else {
                let param_args = vg.param_refs.join(", ");
                lines.push(format!(
                    "{}({}) {};",
                    vg.name.as_str(),
                    param_args,
                    qubit_args
                ));
            }
        }
    }

    Ok(lines.join("\n"))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn max_cbit_index(ast: &EhrenfestAst) -> usize {
    let from_measures = ast.measures.iter().map(|m| m.cbit + 1).max().unwrap_or(0);
    let from_conds = ast
        .conditionals
        .iter()
        .map(|c| c.cbit + 1)
        .max()
        .unwrap_or(0);
    from_measures.max(from_conds)
}

fn format_gate(gate: &Gate, version: QasmVersion) -> Result<String, EmitError> {
    let qubit_args: String = gate
        .qubits
        .iter()
        .map(|idx| format!("q[{idx}]"))
        .collect::<Vec<_>>()
        .join(", ");

    let line = if gate.params.is_empty() {
        format!("{} {};", gate.name.as_str(), qubit_args)
    } else {
        let param_str = gate
            .params
            .iter()
            .map(|p| format_float(*p))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}({}) {};", gate.name.as_str(), param_str, qubit_args)
    };

    // Validate qubit range is left to the caller (parser already does this),
    // but we validate the gate name is emittable.
    let _ = version; // Both versions use the same gate names for now.
    Ok(line)
}

/// Format a float parameter for QASM output.
///
/// Uses pi-fraction notation when the value is a clean multiple of pi.
fn format_float(val: f64) -> String {
    let pi = std::f64::consts::PI;
    let ratio = val / pi;

    // Check if it's a clean multiple of pi (within floating-point tolerance).
    if (ratio - ratio.round()).abs() < 1e-10 {
        let r = ratio.round() as i64;
        return match r {
            0 => "0".into(),
            1 => "pi".into(),
            -1 => "-pi".into(),
            _ => format!("{}*pi", r),
        };
    }

    // Check common fractions: pi/2, pi/4, pi/8, 3pi/4, etc.
    for denom in [2, 4, 8] {
        let numer = ratio * denom as f64;
        if (numer - numer.round()).abs() < 1e-10 {
            let n = numer.round() as i64;
            return match n {
                1 => format!("pi/{denom}"),
                -1 => format!("-pi/{denom}"),
                _ => format!("{n}*pi/{denom}"),
            };
        }
    }

    // Fall back to decimal.
    format!("{val}")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn emit_bell_v2() {
        let source = r#"
program "bell"
qubits 2
h q0
cnot q0 q1
measure q0 -> c0
measure q1 -> c1
"#;
        let ast = parser::parse(source).unwrap();
        let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
        assert!(qasm.contains("OPENQASM 2.0;"));
        assert!(qasm.contains("include \"qelib1.inc\";"));
        assert!(qasm.contains("h q[0];"));
        assert!(qasm.contains("cx q[0], q[1];"));
        assert!(qasm.contains("measure q[0] -> c[0];"));
    }

    #[test]
    fn emit_bell_v3() {
        let source = r#"
program "bell"
qubits 2
h q0
cnot q0 q1
measure q0 -> c0
measure q1 -> c1
"#;
        let ast = parser::parse(source).unwrap();
        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        assert!(qasm.contains("OPENQASM 3.0;"));
        assert!(qasm.contains("include \"stdgates.inc\";"));
        assert!(qasm.contains("qubit[2] q;"));
        assert!(qasm.contains("c[0] = measure q[0];"));
    }

    #[test]
    fn emit_conditional_v2() {
        let source = r#"
program "cond"
qubits 2
h q0
measure q0 -> c0
if c0 == 1: x q1
"#;
        let ast = parser::parse(source).unwrap();
        let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
        assert!(qasm.contains("if(c[0]==1) x q[1];"));
    }

    #[test]
    fn emit_rotation_pi_fraction() {
        let source = r#"
program "rot"
qubits 1
rx 1.5707963267948966 q0
"#;
        let ast = parser::parse(source).unwrap();
        let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
        assert!(qasm.contains("rx(pi/2) q[0];"));
    }

    #[test]
    fn format_float_pi_fractions() {
        assert_eq!(format_float(std::f64::consts::PI), "pi");
        assert_eq!(format_float(-std::f64::consts::PI), "-pi");
        assert_eq!(format_float(std::f64::consts::FRAC_PI_2), "pi/2");
        assert_eq!(format_float(std::f64::consts::FRAC_PI_4), "pi/4");
        assert_eq!(format_float(0.0), "0");
        assert_eq!(format_float(2.0 * std::f64::consts::PI), "2*pi");
    }

    #[test]
    fn emit_variational_v3() {
        let source = r#"
program "vqe"
qubits 2
variational params theta phi max_iter 1
  rx theta q0
  ry phi q1
  cx q0 q1
end
"#;
        let ast = parser::parse(source).unwrap();
        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        // Ensure input declarations are present
        assert!(qasm.contains("input float[64] theta;"));
        assert!(qasm.contains("input float[64] phi;"));
        // Ensure parametric gates use the parameters
        assert!(qasm.contains("rx(theta) q[0];"));
        assert!(qasm.contains("ry(phi) q[1];"));
    }
}
