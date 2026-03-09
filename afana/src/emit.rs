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

/// Verify that all `VariationalGate.param_refs` resolve to declared
/// `VariationalLoop.params`. Must be called before QASM emission.
pub fn verify_parameter_bindings(ast: &EhrenfestAst) -> Result<(), EmitError> {
    for vloop in &ast.variational_loops {
        let declared: std::collections::HashSet<&str> =
            vloop.params.iter().map(|s| s.as_str()).collect();
        for vg in &vloop.body {
            for pref in &vg.param_refs {
                if !declared.contains(pref.as_str()) {
                    return Err(EmitError::UnboundParameter {
                        param: pref.clone(),
                        gate: vg.name.as_str().to_string(),
                        declared: vloop.params.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Emit an [`EhrenfestAst`] as OpenQASM source.
pub fn emit_qasm(ast: &EhrenfestAst, version: QasmVersion) -> Result<String, EmitError> {
    // Verify parameter bindings before emission.
    verify_parameter_bindings(ast)?;

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

    // Variational loops → QASM 3.0 with classical control flow.
    for vloop in &ast.variational_loops {
        lines.push(String::new());
        lines.push(format!(
            "// Variational ansatz — max_iter={}",
            vloop.max_iter
        ));
        if version == QasmVersion::V3 {
            // Declare variational parameters as mutable floats.
            for p in &vloop.params {
                lines.push(format!("mutable float[64] {p};"));
            }
            // Emit the for loop with classical control.
            lines.push(format!("for int i in [0:{}-1] {{", vloop.max_iter));
            for vg in &vloop.body {
                let qubit_args: String = vg
                    .qubits
                    .iter()
                    .map(|idx| format!("q[{idx}]"))
                    .collect::<Vec<_>>()
                    .join(", ");

                if vg.param_refs.is_empty() {
                    lines.push(format!("    {} {};", vg.name.as_str(), qubit_args));
                } else {
                    let param_args = vg.param_refs.join(", ");
                    lines.push(format!(
                        "    {}({}) {};",
                        vg.name.as_str(),
                        param_args,
                        qubit_args
                    ));
                }
            }
            lines.push("    // Classical parameter update would occur here".into());
            lines.push("}".into());
        } else {
            // QASM 2.0 fallback: emit parameters as comments.
            lines.push("// QASM 2.0 does not support variational loops".into());
            for p in &vloop.params {
                lines.push(format!("// parameter: {p}"));
            }
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
    let qubit_args: String = gate.qubits.iter().map(|idx| format!("q[{idx}]"))    .collect::<Vec<_>>().join(", ");
    let line = match gate.name {
        GateName::H => format!("h {};", qubit_args),
        GateName::X => format!("x {};", qubit_args),
        GateName::Y => format!("y {};", qubit_args),
        GateName::Z => format!("z {};", qubit_args),
        GateName::S => format!("s {};", qubit_args),
        GateName::T => format!("t {};", qubit_args),
        GateName::Sdg => format!("sdg {};", qubit_args),
        GateName::Tdg => format!("tdg {};", qubit_args),
        GateName::Rx => format!("rx({}) {};", gate.params[0], qubit_args),
        GateName::Ry => format!("ry({}) {};", gate.params[0], qubit_args),
        GateName::Rz => format!("rz({}) {};", gate.params[0], qubit_args),
        _ => return Err(EmitError::UnsupportedGate(gate.name.to_string())),
    };
    Ok(line)
} {
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

    /// Helper: build a Bell-state AST directly (no text parsing).
    fn bell_ast() -> EhrenfestAst {
        EhrenfestAst {
            name: "bell".into(),
            n_qubits: 2,
            prepare: None,
            gates: vec![
                Gate { name: GateName::H, qubits: vec![0], params: vec![] },
                Gate { name: GateName::Cx, qubits: vec![0, 1], params: vec![] },
            ],
            measures: vec![
                Measure { qubit: 0, cbit: 0 },
                Measure { qubit: 1, cbit: 1 },
            ],
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        }
    }

    #[test]
    fn emit_bell_v2() {
        let ast = bell_ast();
        let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
        assert!(qasm.contains("OPENQASM 2.0;"));
        assert!(qasm.contains("include \"qelib1.inc\";"));
        assert!(qasm.contains("h q[0];"));
        assert!(qasm.contains("cx q[0], q[1];"));
        assert!(qasm.contains("measure q[0] -> c[0];"));
    }

    #[test]
    fn emit_bell_v3() {
        let ast = bell_ast();
        let qasm = emit_qasm(&ast, QasmVersion::V3).unwrap();
        assert!(qasm.contains("OPENQASM 3.0;"));
        assert!(qasm.contains("include \"stdgates.inc\";"));
        assert!(qasm.contains("qubit[2] q;"));
        assert!(qasm.contains("c[0] = measure q[0];"));
    }

    #[test]
    fn emit_conditional_v2() {
        let ast = EhrenfestAst {
            name: "cond".into(),
            n_qubits: 2,
            prepare: None,
            gates: vec![
                Gate { name: GateName::H, qubits: vec![0], params: vec![] },
            ],
            measures: vec![
                Measure { qubit: 0, cbit: 0 },
            ],
            conditionals: vec![
                ConditionalGate {
                    cbit: 0,
                    cbit_value: 1,
                    gate: Gate { name: GateName::X, qubits: vec![1], params: vec![] },
                },
            ],
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };
        let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
        assert!(qasm.contains("if(c[0]==1) x q[1];"));
    }

    #[test]
    fn emit_rotation_pi_fraction() {
        let ast = EhrenfestAst {
            name: "rot".into(),
            n_qubits: 1,
            prepare: None,
            gates: vec![
                Gate {
                    name: GateName::Rx,
                    qubits: vec![0],
                    params: vec![std::f64::consts::FRAC_PI_2],
                },
            ],
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: Vec::new(),
        };
        let qasm = emit_qasm(&ast, QasmVersion::V2).unwrap();
        assert!(qasm.contains("rx(pi/2) q[0];"));
    }

    #[test]
    fn verify_param_binding_ok() {
        let ast = EhrenfestAst {
            name: "vqe".into(),
            n_qubits: 2,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into(), "phi".into()],
                max_iter: 100,
                body: vec![
                    VariationalGate {
                        name: GateName::Ry,
                        qubits: vec![0],
                        param_refs: vec!["theta".into()],
                    },
                    VariationalGate {
                        name: GateName::Rz,
                        qubits: vec![1],
                        param_refs: vec!["phi".into()],
                    },
                ],
            }],
        };
        assert!(verify_parameter_bindings(&ast).is_ok());
        // Should also emit successfully.
        assert!(emit_qasm(&ast, QasmVersion::V3).is_ok());
    }

    #[test]
    fn verify_param_binding_unbound() {
        let ast = EhrenfestAst {
            name: "bad_vqe".into(),
            n_qubits: 1,
            prepare: None,
            gates: Vec::new(),
            measures: Vec::new(),
            conditionals: Vec::new(),
            expects: Vec::new(),
            type_decls: Vec::new(),
            variational_loops: vec![VariationalLoop {
                params: vec!["theta".into()],
                max_iter: 50,
                body: vec![VariationalGate {
                    name: GateName::Ry,
                    qubits: vec![0],
                    param_refs: vec!["gamma".into()],
                }],
            }],
        };
        let err = verify_parameter_bindings(&ast).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("gamma"), "error should name the unbound param");
        assert!(msg.contains("theta"), "error should list declared params");

        // emit_qasm should also fail.
        assert!(emit_qasm(&ast, QasmVersion::V3).is_err());
    }

    #[test]
    fn verify_param_binding_no_loops_ok() {
        // AST with no variational loops should pass.
        let ast = bell_ast();
        assert!(verify_parameter_bindings(&ast).is_ok());
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
}
