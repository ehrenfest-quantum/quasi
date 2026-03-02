// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Ehrenfest `.ef` text parser — converts `.ef` source to an AST.
//!
//! Grammar (v0.2):
//! ```text
//! program  ::= type_decl* header stmt*
//! type_decl ::= 'type' NAME '=' type_expr
//! header   ::= 'program' STRING 'qubits' INT ('prepare' 'basis' STATE)?
//! stmt     ::= gate_stmt | measure_stmt | expect_stmt
//!            | conditional_stmt | variational_block | comment
//! ```
//!
//! No external dependencies beyond regex for tokenization.

use crate::ast::*;
use crate::error::ParseError;

use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

// ── Regex patterns ───────────────────────────────────────────────────────────

static GATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(h|x|y|z|s|t|sdg|tdg|cx|cnot|cz|swap|ccx|toffoli|rx|ry|rz)$").unwrap()
});
static QUBIT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^q(\d+)$").unwrap());
static CBIT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^c(\d+)$").unwrap());
static FLOAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^-?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?(pi)?$").unwrap()
});
static PARAM_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap());

// ── Token helpers ────────────────────────────────────────────────────────────

fn parse_qubit(tok: &str, line: usize) -> Result<usize, ParseError> {
    QUBIT_RE
        .captures(tok)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<usize>().ok())
        .ok_or_else(|| ParseError::Syntax {
            line,
            message: format!("expected qubit (e.g. q0), got {tok:?}"),
        })
}

fn parse_cbit(tok: &str, line: usize) -> Result<usize, ParseError> {
    CBIT_RE
        .captures(tok)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<usize>().ok())
        .ok_or_else(|| ParseError::Syntax {
            line,
            message: format!("expected cbit (e.g. c0), got {tok:?}"),
        })
}

fn parse_float_param(tok: &str, line: usize) -> Result<f64, ParseError> {
    let t = tok.to_lowercase();
    if t.ends_with("pi") {
        let prefix = &t[..t.len() - 2];
        let factor = match prefix {
            "" | "+" => 1.0,
            "-" => -1.0,
            _ => prefix.parse::<f64>().map_err(|_| ParseError::Syntax {
                line,
                message: format!("invalid float parameter {tok:?}"),
            })?,
        };
        return Ok(factor * std::f64::consts::PI);
    }
    tok.parse::<f64>().map_err(|_| ParseError::Syntax {
        line,
        message: format!("invalid float parameter {tok:?}"),
    })
}

fn is_gate_token(tok: &str) -> bool {
    GATE_RE.is_match(tok)
}

fn is_qubit_token(tok: &str) -> bool {
    QUBIT_RE.is_match(tok)
}

fn is_cbit_token(tok: &str) -> bool {
    CBIT_RE.is_match(tok)
}

fn is_float_token(tok: &str) -> bool {
    FLOAT_RE.is_match(tok)
}

fn is_param_name(tok: &str) -> bool {
    PARAM_NAME_RE.is_match(tok) && !is_qubit_token(tok) && !is_cbit_token(tok)
}

/// Strip inline `//` comments and trim whitespace.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(idx) => line[..idx].trim(),
        None => line.trim(),
    }
}

/// Simple shell-like tokenizer that respects double-quoted strings.
fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                // Don't include the quotes in the token.
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

// ── Tokenized line ───────────────────────────────────────────────────────────

struct Line {
    lineno: usize,
    tokens: Vec<String>,
}

fn prepare_lines(source: &str) -> Vec<Line> {
    source
        .lines()
        .enumerate()
        .filter_map(|(idx, raw)| {
            let clean = strip_comment(raw);
            if clean.is_empty() {
                return None;
            }
            let tokens = tokenize(clean);
            if tokens.is_empty() {
                return None;
            }
            Some(Line {
                lineno: idx + 1,
                tokens,
            })
        })
        .collect()
}

// ── Main parser ──────────────────────────────────────────────────────────────

/// Parse `.ef` source text into an [`EhrenfestAst`].
pub fn parse(source: &str) -> Result<EhrenfestAst, ParseError> {
    let lines = prepare_lines(source);
    let mut iter = lines.iter().peekable();

    // ── Type declarations (before header) ────────────────────────────────
    let mut type_decls: Vec<TypeDecl> = Vec::new();

    while let Some(line) = iter.peek() {
        if line.tokens[0].to_lowercase() == "type" {
            let line = iter.next().unwrap();
            type_decls.push(parse_type_decl(line)?);
        } else {
            break;
        }
    }

    // ── Header: program NAME ─────────────────────────────────────────────
    let line = iter.next().ok_or_else(|| ParseError::UnexpectedEof {
        expected: "'program' declaration".into(),
    })?;
    if line.tokens[0].to_lowercase() != "program" {
        return Err(ParseError::Syntax {
            line: line.lineno,
            message: format!("expected 'program', got {:?}", line.tokens[0]),
        });
    }
    if line.tokens.len() < 2 {
        return Err(ParseError::Syntax {
            line: line.lineno,
            message: "'program' requires a name".into(),
        });
    }
    let program_name = line.tokens[1].clone();

    // ── Header: qubits N ─────────────────────────────────────────────────
    let line = iter.next().ok_or_else(|| ParseError::UnexpectedEof {
        expected: "'qubits' declaration".into(),
    })?;
    if line.tokens[0].to_lowercase() != "qubits" {
        return Err(ParseError::Syntax {
            line: line.lineno,
            message: format!("expected 'qubits', got {:?}", line.tokens[0]),
        });
    }
    let n_qubits: usize =
        line.tokens
            .get(1)
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| ParseError::Syntax {
                line: line.lineno,
                message: "'qubits' requires a positive integer".into(),
            })?;
    if n_qubits == 0 {
        return Err(ParseError::Syntax {
            line: line.lineno,
            message: "qubit count must be >= 1".into(),
        });
    }

    // ── Body ─────────────────────────────────────────────────────────────
    let mut prepare: Option<String> = None;
    let mut gates: Vec<Gate> = Vec::new();
    let mut measures: Vec<Measure> = Vec::new();
    let mut conditionals: Vec<ConditionalGate> = Vec::new();
    let mut expects: Vec<Expect> = Vec::new();
    let mut variational_loops: Vec<VariationalLoop> = Vec::new();

    while let Some(line) = iter.next() {
        let kw = line.tokens[0].to_lowercase();

        match kw.as_str() {
            "prepare" => {
                if line.tokens.len() < 3 || line.tokens[1].to_lowercase() != "basis" {
                    return Err(ParseError::Syntax {
                        line: line.lineno,
                        message: "'prepare' syntax: prepare basis |STATE>".into(),
                    });
                }
                prepare = Some(line.tokens[2].clone());
            }

            "measure" => {
                if line.tokens.len() < 4 || line.tokens[2] != "->" {
                    return Err(ParseError::Syntax {
                        line: line.lineno,
                        message: "'measure' syntax: measure qN -> cN".into(),
                    });
                }
                let qubit = parse_qubit(&line.tokens[1], line.lineno)?;
                let cbit = parse_cbit(&line.tokens[3], line.lineno)?;
                if qubit >= n_qubits {
                    return Err(ParseError::Syntax {
                        line: line.lineno,
                        message: format!(
                            "qubit q{qubit} out of range (n_qubits={n_qubits})"
                        ),
                    });
                }
                measures.push(Measure { qubit, cbit });
            }

            "expect" => {
                if line.tokens.len() < 3 {
                    return Err(ParseError::Syntax {
                        line: line.lineno,
                        message: "'expect' syntax: expect state|counts|relation \"...\"".into(),
                    });
                }
                let kind = match line.tokens[1].to_lowercase().as_str() {
                    "state" => ExpectKind::State,
                    "counts" => ExpectKind::Counts,
                    "relation" => ExpectKind::Relation,
                    other => {
                        return Err(ParseError::Syntax {
                            line: line.lineno,
                            message: format!(
                                "'expect' kind must be 'state', 'counts', or 'relation', got {other:?}"
                            ),
                        });
                    }
                };
                expects.push(Expect {
                    kind,
                    value: line.tokens[2].clone(),
                });
            }

            "if" => {
                if line.tokens.len() < 5 {
                    return Err(ParseError::Syntax {
                        line: line.lineno,
                        message: "'if' syntax: if cN == M: gate qN".into(),
                    });
                }
                let cbit = parse_cbit(&line.tokens[1], line.lineno)?;
                if line.tokens[2] != "==" {
                    return Err(ParseError::Syntax {
                        line: line.lineno,
                        message: format!("'if' expects '==', got {:?}", line.tokens[2]),
                    });
                }
                let val_tok = line.tokens[3].trim_end_matches(':');
                let cbit_value: u32 =
                    val_tok.parse().map_err(|_| ParseError::Syntax {
                        line: line.lineno,
                        message: format!(
                            "'if' condition value must be an integer, got {:?}",
                            line.tokens[3]
                        ),
                    })?;
                let gate = parse_gate_tokens(&line.tokens[4..], line.lineno, n_qubits)?;
                conditionals.push(ConditionalGate {
                    cbit,
                    cbit_value,
                    gate,
                });
            }

            "type" => {
                type_decls.push(parse_type_decl(line)?);
            }

            "variational" => {
                let vloop =
                    parse_variational_block(line, &mut iter, n_qubits)?;
                variational_loops.push(vloop);
            }

            _ if is_gate_token(&kw) => {
                let gate = parse_gate_tokens(&line.tokens, line.lineno, n_qubits)?;
                gates.push(gate);
            }

            _ => {
                return Err(ParseError::Syntax {
                    line: line.lineno,
                    message: format!("unknown directive {:?}", line.tokens[0]),
                });
            }
        }
    }

    Ok(EhrenfestAst {
        name: program_name,
        n_qubits,
        prepare,
        gates,
        measures,
        conditionals,
        expects,
        type_decls,
        variational_loops,
    })
}

/// Parse a `.ef` file from disk.
pub fn parse_file(path: &Path) -> Result<EhrenfestAst, ParseError> {
    let source = std::fs::read_to_string(path)?;
    parse(&source)
}

// ── Sub-parsers ──────────────────────────────────────────────────────────────

fn parse_type_decl(line: &Line) -> Result<TypeDecl, ParseError> {
    if line.tokens.len() < 2 {
        return Err(ParseError::Syntax {
            line: line.lineno,
            message: "'type' syntax: type NAME = TYPE_EXPR".into(),
        });
    }
    let name = &line.tokens[1];
    if line.tokens.len() < 3 || line.tokens[2] != "=" {
        let got = line.tokens.get(2).map_or("<end of line>", |s| s.as_str());
        return Err(ParseError::Syntax {
            line: line.lineno,
            message: format!("'type' declaration expects '=', got {got:?}"),
        });
    }
    let definition = line.tokens[3..].join(" ");
    if definition.is_empty() {
        return Err(ParseError::Syntax {
            line: line.lineno,
            message: "'type' declaration requires a type expression".into(),
        });
    }
    Ok(TypeDecl {
        name: name.clone(),
        definition,
    })
}

/// Parse gate tokens like `["h", "q0"]` or `["rx", "1.57", "q0"]`.
fn parse_gate_tokens(
    tokens: &[String],
    lineno: usize,
    n_qubits: usize,
) -> Result<Gate, ParseError> {
    if tokens.is_empty() {
        return Err(ParseError::Syntax {
            line: lineno,
            message: "expected gate name".into(),
        });
    }

    let gate_name = GateName::from_token(&tokens[0].to_lowercase()).ok_or_else(|| {
        ParseError::Syntax {
            line: lineno,
            message: format!("unknown gate {:?}", tokens[0]),
        }
    })?;

    let rest = &tokens[1..];
    let mut params: Vec<f64> = Vec::new();
    let mut qubits: Vec<usize> = Vec::new();

    // Rotation gates take one float parameter before qubit args.
    let qubit_start = if gate_name.is_parametric() {
        if rest.is_empty() {
            return Err(ParseError::Syntax {
                line: lineno,
                message: format!("{} requires an angle parameter", gate_name),
            });
        }
        params.push(parse_float_param(&rest[0], lineno)?);
        1
    } else {
        0
    };

    for tok in &rest[qubit_start..] {
        let idx = parse_qubit(tok, lineno)?;
        if idx >= n_qubits {
            return Err(ParseError::Syntax {
                line: lineno,
                message: format!("qubit q{idx} out of range (n_qubits={n_qubits})"),
            });
        }
        qubits.push(idx);
    }

    if qubits.is_empty() {
        return Err(ParseError::Syntax {
            line: lineno,
            message: format!("gate {:?} requires at least one qubit", tokens[0]),
        });
    }

    Ok(Gate {
        name: gate_name,
        qubits,
        params,
    })
}

fn parse_variational_block<'a, I>(
    header: &Line,
    iter: &mut std::iter::Peekable<I>,
    n_qubits: usize,
) -> Result<VariationalLoop, ParseError>
where
    I: Iterator<Item = &'a Line>,
{
    if header.tokens.len() < 2 || header.tokens[1].to_lowercase() != "params" {
        return Err(ParseError::Syntax {
            line: header.lineno,
            message: "'variational' syntax: variational params NAME+ [max_iter INT]".into(),
        });
    }

    let rest = &header.tokens[2..];
    let mut params: Vec<String> = Vec::new();
    let mut max_iter: u32 = 100;
    let mut i = 0;

    while i < rest.len() {
        if rest[i].to_lowercase() == "max_iter" {
            if i + 1 >= rest.len() {
                return Err(ParseError::Syntax {
                    line: header.lineno,
                    message: "'max_iter' must be followed by a positive integer".into(),
                });
            }
            max_iter = rest[i + 1].parse().map_err(|_| ParseError::Syntax {
                line: header.lineno,
                message: "'max_iter' must be followed by a positive integer".into(),
            })?;
            i += 2;
        } else if is_param_name(&rest[i]) {
            params.push(rest[i].clone());
            i += 1;
        } else {
            return Err(ParseError::Syntax {
                line: header.lineno,
                message: format!(
                    "unexpected token in 'variational' header: {:?}",
                    rest[i]
                ),
            });
        }
    }

    if params.is_empty() {
        return Err(ParseError::Syntax {
            line: header.lineno,
            message: "'variational' block requires at least one parameter name".into(),
        });
    }

    let mut body: Vec<VariationalGate> = Vec::new();

    loop {
        let line = iter.next().ok_or_else(|| ParseError::Syntax {
            line: header.lineno,
            message: "'variational' block opened here is never closed with 'end'".into(),
        })?;

        if line.tokens[0].to_lowercase() == "end" {
            break;
        }

        let gate_tok = line.tokens[0].to_lowercase();
        let gate_name = GateName::from_token(&gate_tok).ok_or_else(|| ParseError::Syntax {
            line: line.lineno,
            message: format!(
                "variational body expects a gate or 'end', got {:?}",
                line.tokens[0]
            ),
        })?;

        let mut param_refs: Vec<String> = Vec::new();
        let mut qubits: Vec<usize> = Vec::new();

        for tok in &line.tokens[1..] {
            if is_qubit_token(tok) {
                let idx = parse_qubit(tok, line.lineno)?;
                if idx >= n_qubits {
                    return Err(ParseError::Syntax {
                        line: line.lineno,
                        message: format!(
                            "qubit q{idx} out of range (n_qubits={n_qubits})"
                        ),
                    });
                }
                qubits.push(idx);
            } else if is_param_name(tok) || is_float_token(tok) {
                param_refs.push(tok.clone());
            } else {
                return Err(ParseError::Syntax {
                    line: line.lineno,
                    message: format!("unexpected token in variational gate: {tok:?}"),
                });
            }
        }

        if qubits.is_empty() {
            return Err(ParseError::Syntax {
                line: line.lineno,
                message: format!(
                    "variational gate {:?} requires at least one qubit",
                    line.tokens[0]
                ),
            });
        }

        body.push(VariationalGate {
            name: gate_name,
            qubits,
            param_refs,
        });
    }

    Ok(VariationalLoop {
        params,
        max_iter,
        body,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bell_state() {
        let source = r#"
program "bell"
qubits 2
prepare basis |00>
h q0
cnot q0 q1
measure q0 -> c0
measure q1 -> c1
expect state "(|00> + |11>) / sqrt(2)"
"#;
        let ast = parse(source).unwrap();
        assert_eq!(ast.name, "bell");
        assert_eq!(ast.n_qubits, 2);
        assert_eq!(ast.prepare.as_deref(), Some("|00>"));
        assert_eq!(ast.gates.len(), 2);
        assert_eq!(ast.gates[0].name, GateName::H);
        assert_eq!(ast.gates[0].qubits, vec![0]);
        assert_eq!(ast.gates[1].name, GateName::Cx);
        assert_eq!(ast.gates[1].qubits, vec![0, 1]);
        assert_eq!(ast.measures.len(), 2);
        assert_eq!(ast.expects.len(), 1);
        assert_eq!(ast.expects[0].kind, ExpectKind::State);
    }

    #[test]
    fn parse_rotation_gate() {
        let source = r#"
program "rotation"
qubits 1
rx 1.57 q0
ry 3.14pi q0
"#;
        let ast = parse(source).unwrap();
        assert_eq!(ast.gates.len(), 2);
        assert_eq!(ast.gates[0].name, GateName::Rx);
        assert!((ast.gates[0].params[0] - 1.57).abs() < 1e-10);
        assert_eq!(ast.gates[1].name, GateName::Ry);
        assert!((ast.gates[1].params[0] - 3.14 * std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn parse_conditional() {
        let source = r#"
program "teleport"
qubits 3
h q0
cnot q0 q1
measure q0 -> c0
if c0 == 1: x q2
"#;
        let ast = parse(source).unwrap();
        assert_eq!(ast.conditionals.len(), 1);
        assert_eq!(ast.conditionals[0].cbit, 0);
        assert_eq!(ast.conditionals[0].cbit_value, 1);
        assert_eq!(ast.conditionals[0].gate.name, GateName::X);
    }

    #[test]
    fn parse_variational() {
        let source = r#"
program "vqe"
qubits 2
variational params theta phi max_iter 200
  rx theta q0
  ry phi q1
  cnot q0 q1
end
"#;
        let ast = parse(source).unwrap();
        assert_eq!(ast.variational_loops.len(), 1);
        let vl = &ast.variational_loops[0];
        assert_eq!(vl.params, vec!["theta", "phi"]);
        assert_eq!(vl.max_iter, 200);
        assert_eq!(vl.body.len(), 3);
    }

    #[test]
    fn parse_type_decl() {
        let source = r#"
type QubitPair = (Qubit, Qubit)
program "typed"
qubits 2
h q0
"#;
        let ast = parse(source).unwrap();
        assert_eq!(ast.type_decls.len(), 1);
        assert_eq!(ast.type_decls[0].name, "QubitPair");
        assert_eq!(ast.type_decls[0].definition, "(Qubit, Qubit)");
    }

    #[test]
    fn parse_comments_stripped() {
        let source = r#"
// This is a comment
program "test"
qubits 1
h q0 // inline comment
"#;
        let ast = parse(source).unwrap();
        assert_eq!(ast.gates.len(), 1);
    }

    #[test]
    fn error_qubit_out_of_range() {
        let source = r#"
program "bad"
qubits 2
h q5
"#;
        let err = parse(source).unwrap_err();
        assert!(err.to_string().contains("out of range"));
    }

    #[test]
    fn error_missing_program() {
        let source = "qubits 2\nh q0\n";
        let err = parse(source).unwrap_err();
        assert!(err.to_string().contains("expected 'program'"));
    }
}
