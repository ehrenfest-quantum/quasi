# Ehrenfest Specification

The `spec/` directory defines the machine-facing representation of Ehrenfest programs and related examples.

## Contents

- `ehrenfest-v0.1.cddl`: Base CDDL schema for the current Ehrenfest format
- `examples/`: Reference example payloads and companion explanations
- `tools/validate.py`: Lightweight validator for checking example payloads against the schema
- `tools/generate_examples.py`: Helper script used to regenerate bundled examples

## Validation

Validate the bundled examples with:

```bash
python3 spec/tools/validate.py
```

## Example files

Each example in `spec/examples/` is provided in two forms:

- `*.md`: human-readable explanation of the modeled quantum program
- `*.cbor.hex`: the encoded binary payload represented as hexadecimal

## Compatibility

The current schema is `ehrenfest-v0.1.cddl`. Future schema revisions should be added alongside it rather than replacing it in place so tooling can remain backward-compatible.
