# Ehrenfest Binary Format — CBOR Encoding Rules

**QUASI-001** | Ehrenfest v0.1

Ehrenfest programs are encoded as CBOR (RFC 8949) binary documents.
There is no text form. The `.paul` extension is used for binary files.

## Wire Format

### Canonical CBOR (RFC 8949 §4.2)

All Ehrenfest encoders MUST produce **deterministically encoded CBOR**:

1. **Map keys**: Text strings, sorted by length first, then lexicographically
2. **Integers**: Shortest possible encoding (no overlong forms)
3. **Floats**: Preferred serialization — use the shortest IEEE 754 form
   that preserves the value (float16 if lossless, else float32, else float64)
4. **No indefinite-length** maps, arrays, or byte strings

### Float Encoding

| Value class | CBOR major type | Bytes |
|---|---|---|
| Zero, ±Infinity, NaN | Float16 (0xf9) | 3 |
| Representable in float16 | Float16 (0xf9) | 3 |
| Representable in float32 | Float32 (0xfa) | 5 |
| All other | Float64 (0xfb) | 9 |

Ehrenfest coefficients (GHz·rad) and times (μs) typically require float64.
Fidelity values near 1.0 (e.g., 0.999) require float64 for precision.

### Integer Encoding

PauliAxis values (0–3) encode as CBOR unsigned integers:

| Axis | Value | CBOR byte |
|---|---|---|
| I (identity) | 0 | `0x00` |
| X (bit-flip) | 1 | `0x01` |
| Y (bit+phase-flip) | 2 | `0x02` |
| Z (phase-flip) | 3 | `0x03` |

## Field Key Table

All map keys are text strings. The following table lists every key
in an Ehrenfest v0.1 program:

| Key | Parent | CBOR type | Required |
|---|---|---|---|
| `version` | root | uint | yes |
| `system` | root | map | yes |
| `hamiltonian` | root | map | yes |
| `evolution` | root | map | yes |
| `observables` | root | array | yes |
| `noise` | root | map | yes |
| `n_qubits` | system | uint | yes |
| `backend_hint` | system | tstr | no |
| `cooling_profile` | system | map | no |
| `target_temp_mk` | cooling_profile | float | yes |
| `ramp_time_us` | cooling_profile | float | no |
| `terms` | hamiltonian | array | yes |
| `constant_offset` | hamiltonian | float | yes |
| `coefficient` | PauliTerm | float | yes |
| `paulis` | PauliTerm | array | yes |
| `qubit` | PauliOp | uint | yes |
| `axis` | PauliOp | uint (PauliAxis) | yes |
| `total_us` | evolution | float | yes |
| `steps` | evolution | uint | yes |
| `dt_us` | evolution | float | yes |
| `type` | Observable | tstr | yes |
| `t1_us` | noise | float | yes |
| `t2_us` | noise | float | yes |
| `gate_fidelity_min` | noise | float | no |
| `readout_fidelity_min` | noise | float | no |

## CBOR Tags

No CBOR tags are assigned in v0.1. Tag space is reserved for future use:

- Tag 55799 (self-describe CBOR) MAY be used as a magic number
- Custom tag range (> 256) reserved for Ehrenfest-specific extensions

## Observable Type Discriminator

The `"type"` field in Observable maps determines the variant:

| `type` value | Variant | Additional fields |
|---|---|---|
| `"SZ"` | SigmaZ | `qubit`: uint |
| `"SX"` | SigmaX | `qubit`: uint |
| `"E"` | Energy | (none) |
| `"rho"` | Density | `qubits`: [+ uint] |
| `"F"` | Fidelity | `target_state`: bstr |

## Validation Rules

Afana enforces these constraints at deserialization:

1. `version` MUST be 1
2. `n_qubits` MUST be ≥ 1
3. `dt_us` MUST equal `total_us / steps` (within floating-point tolerance)
4. `t2_us` ≤ 2 × `t1_us` (physical bound, enforced by compiler)
5. All qubit indices MUST be in `[0, n_qubits - 1]`
6. `observables` MUST contain at least one entry

## Example

A minimal 1-qubit program (Rabi oscillation) in annotated hex:

```
a6                          # map(6)
  67 76657273696f6e         #   "version"
  01                        #   1
  66 73797374656d           #   "system"
  a1                        #   map(1)
    68 6e5f717562697473     #     "n_qubits"
    01                      #     1
  6b 68616d696c746f6e69616e #   "hamiltonian"
  a2                        #   map(2)
    ...                     #     (terms + constant_offset)
```

Full hex-encoded examples are in `spec/examples/`.
Binary `.paul` files are in `examples/`.
