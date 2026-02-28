# Ehrenfest Noise Channels

`noise.channels` extends the Ehrenfest noise block with optional metadata about
the decoherence model assumed by a program. These annotations are advisory:
backends may ignore them, but simulators and backend-selection layers can use
them when estimating fidelity or choosing a target device.

## Encoding

The field lives inside the existing `noise` map:

```cddl
"noise": {
  "t1_us": float,
  "t2_us": float,
  ? "channels": [* NoiseChannel],
}
```

Each element in `channels` is one of three tagged maps:

### Depolarizing noise

```json
{ "type": 1, "qubit": 0, "p": 0.02 }
```

- `type = 1`
- `qubit`: affected qubit index
- `p`: depolarizing probability in `[0.0, 1.0]`

### Amplitude damping

```json
{ "type": 2, "qubit": 1, "gamma": 0.08 }
```

- `type = 2`
- `qubit`: affected qubit index
- `gamma`: damping factor in `[0.0, 1.0]`

### Phase damping

```json
{ "type": 3, "qubit": 1, "gamma": 0.12 }
```

- `type = 3`
- `qubit`: affected qubit index
- `gamma`: dephasing factor in `[0.0, 1.0]`

## Validation rules

- `noise.channels`, when present, must be an array
- every channel entry must be a map
- `qubit` must be in range for `system.n_qubits`
- `type` must be `1`, `2`, or `3`
- `p` / `gamma` values must be floats in `[0.0, 1.0]`

## Example

```json
{
  "noise": {
    "t1_us": 100.0,
    "t2_us": 80.0,
    "channels": [
      { "type": 1, "qubit": 0, "p": 0.03 },
      { "type": 2, "qubit": 1, "gamma": 0.07 },
      { "type": 3, "qubit": 1, "gamma": 0.11 }
    ]
  }
}
```
