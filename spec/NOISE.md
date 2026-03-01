# Noise Channel Spec — QUASI-025

This document defines the `NoiseChannel` types used by the HAL Contract
submission layer (`SubmitCircuitInput.noiseChannels`) and the corresponding
Python API in `afana.noise_model`.

---

## Architectural Boundary

QUASI separates two distinct noise concepts:

| Concept | Where it lives | What it means |
|---------|---------------|---------------|
| `NoiseConstraint` | `EhrenfestProgram.noise` | Hardware *requirements* (minimum T1, T2, fidelity). Compile-time type constraint — Afana rejects programs whose target backend cannot satisfy them. |
| `NoiseChannel` | `SubmitCircuitInput.noiseChannels` (HAL Contract) | Hardware *behaviour* during execution. Passed at job-submission time after querying `GET /hal/backends/{name}`. Simulators use this to inject realistic decoherence; real-hardware drivers use it for error-mitigation strategy selection. |

**Noise channels do not belong in the Ehrenfest program.** The Ehrenfest
program expresses physics requirements; it does not describe hardware
behaviour.  Noise channels are selected at HAL submission time, informed
by the backend capabilities endpoint.

---

## Channel Types

Three channel types are defined (numeric tag for compact CBOR encoding):

| Tag | Channel type       | Parameter | Physical meaning                    |
|----:|--------------------|-----------|-------------------------------------|
|   1 | Depolarizing       | `p`       | Error probability per gate, p ∈ [0,1] |
|   2 | Amplitude damping  | `gamma`   | T1 relaxation strength, γ ∈ [0,1]  |
|   3 | Phase damping      | `gamma`   | T2\* dephasing strength, γ ∈ [0,1] |

Integer type tags are used so that programs using compact CBOR integer-key
encoding remain concise.

---

## CDDL Schema

These types are defined in `spec/ehrenfest-v0.1.cddl` as reference types.
They are **not** part of `EhrenfestProgram` — they appear in the HAL
Contract `SubmitCircuitInput.noiseChannels` field.

```cddl
NoiseChannel = DepolarizingChannel / AmplitudeDampingChannel / PhaseDampingChannel

DepolarizingChannel = {
  "type":   1,
  "qubit":  uint,
  "p":      float,   ; p ∈ [0, 1]
}

AmplitudeDampingChannel = {
  "type":   2,
  "qubit":  uint,
  "gamma":  float,   ; gamma ∈ [0, 1]
}

PhaseDampingChannel = {
  "type":   3,
  "qubit":  uint,
  "gamma":  float,   ; gamma ∈ [0, 1]
}
```

The TypeScript equivalent lives in `ts-halcontract/src/types.ts` as the
`NoiseChannel` interface, exported from `@quasi/hal-contract`.

---

## Python API

`afana.noise_model` provides dataclasses and encode/decode helpers for
constructing `noiseChannels` arrays to pass in `SubmitCircuitInput`:

```python
from afana.noise_model import (
    DepolarizingChannel,
    AmplitudeDampingChannel,
    PhaseDampingChannel,
    encode_noise_channel,
    decode_noise_channel,
    validate_noise_channels,
)

# Build channels based on GET /hal/backends/{name} noise_profile
channels = [
    DepolarizingChannel(qubit=0, p=0.01),
    AmplitudeDampingChannel(qubit=0, gamma=0.05),
    PhaseDampingChannel(qubit=1, gamma=0.03),
]

# Validate (raises NoiseChannelError if invalid)
validate_noise_channels(channels)

# Encode to dicts for SubmitCircuitInput.noiseChannels
noise_channels = [encode_noise_channel(ch) for ch in channels]
# → [{"type": 1, "qubit": 0, "p": 0.01}, {"type": 2, ...}, ...]
```

### Validation rules

- `qubit` must be a non-negative integer.
- `p` (depolarizing) must satisfy `0.0 ≤ p ≤ 1.0`.
- `gamma` (amplitude or phase damping) must satisfy `0.0 ≤ gamma ≤ 1.0`.
- The array MAY contain multiple channels for the same qubit.
- An empty `noiseChannels` array is equivalent to omitting the field.

---

## Physical Interpretation

### Depolarizing noise (type 1)

After each single-qubit gate on qubit $q$, a uniformly random Pauli error
($X$, $Y$, or $Z$) is applied with probability $p$.  The Kraus operators are:

$$
K_0 = \sqrt{1-p}\,I, \quad
K_1 = \sqrt{p/3}\,X, \quad
K_2 = \sqrt{p/3}\,Y, \quad
K_3 = \sqrt{p/3}\,Z
$$

Typical values on superconducting QPUs: $p \approx 10^{-3}$.

### Amplitude damping (type 2)

Models longitudinal relaxation ($T_1$ process): population decays from
$|1\rangle$ to $|0\rangle$.  The Kraus operators are:

$$
K_0 = \begin{pmatrix}1 & 0\\ 0 & \sqrt{1-\gamma}\end{pmatrix}, \quad
K_1 = \begin{pmatrix}0 & \sqrt{\gamma}\\ 0 & 0\end{pmatrix}
$$

$\gamma$ relates to the T1 time by $\gamma = 1 - e^{-\Delta t / T_1}$.

### Phase damping (type 3)

Models pure dephasing ($T_2^*$ process): off-diagonal coherence decays
without energy exchange.  The Kraus operators are:

$$
K_0 = \begin{pmatrix}1 & 0\\ 0 & \sqrt{1-\gamma}\end{pmatrix}, \quad
K_1 = \begin{pmatrix}0 & 0\\ 0 & \sqrt{\gamma}\end{pmatrix}
$$

$\gamma$ relates to the T2\* time by $\gamma = 1 - e^{-\Delta t / T_2^*}$.

---

## Changelog

| Version | Change |
|---------|--------|
| v0.1 (QUASI-025) | Defined DepolarizingChannel, AmplitudeDampingChannel, PhaseDampingChannel as HAL Contract reference types |
| v0.1.1 (2026-03-01) | Clarified architectural boundary: NoiseChannel belongs at HAL submission layer, not in EhrenfestProgram; removed `noise_channels` from EhrenfestProgram CDDL |
