# Ehrenfest Noise Channel Spec — QUASI-025

This document describes the optional `noise_channels` field added to the
Ehrenfest CBOR schema (`ehrenfest-v0.1.cddl`) and the corresponding Python
API in `afana.noise_model`.

---

## Motivation

The existing `noise` field in an Ehrenfest program expresses *hardware
requirements* (minimum T1, T2, gate fidelity).  Those are compile-time
constraints — if the target backend cannot satisfy them, Afana rejects the
program.

Noise *channels* are different: they describe the expected decoherence
physics during execution.  Backends and simulators use this information to

- apply realistic noise models in simulation,
- choose appropriate error-mitigation strategies (dynamical decoupling,
  measurement error mitigation, randomised compiling).

Noise channels are **optional metadata** — a program without them is still
valid, and backends that do not understand them MUST ignore the field.

---

## CBOR Encoding

Noise channels live in the top-level `noise_channels` array:

```cbor
{
  ...
  "noise_channels": [
    {"type": 1, "qubit": 0, "p": 0.01},
    {"type": 2, "qubit": 0, "gamma": 0.05},
    {"type": 3, "qubit": 1, "gamma": 0.03}
  ]
}
```

### Type Tags

| Tag | Channel type       | Parameter | Physical meaning                    |
|----:|--------------------|-----------|-------------------------------------|
|   1 | Depolarizing       | `p`       | Error probability per gate, p ∈ [0,1] |
|   2 | Amplitude damping  | `gamma`   | T1 relaxation strength, γ ∈ [0,1]  |
|   3 | Phase damping      | `gamma`   | T2\* dephasing strength, γ ∈ [0,1] |

Integer type tags are used (rather than strings) so that programs using
compact CBOR integer-key encoding remain concise.

---

## CDDL Schema

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

---

## Python API

`afana.noise_model` provides dataclasses and validation functions:

```python
from afana.noise_model import (
    DepolarizingChannel,
    AmplitudeDampingChannel,
    PhaseDampingChannel,
    encode_noise_channel,
    decode_noise_channel,
    validate_noise_channels,
)

# Create channels
channels = [
    DepolarizingChannel(qubit=0, p=0.01),
    AmplitudeDampingChannel(qubit=0, gamma=0.05),
    PhaseDampingChannel(qubit=1, gamma=0.03),
]

# Validate (raises NoiseChannelError if invalid)
validate_noise_channels(channels)

# Encode to CBOR-compatible dicts
cbor_data = [encode_noise_channel(ch) for ch in channels]
# → [{"type": 1, "qubit": 0, "p": 0.01}, {"type": 2, ...}, ...]

# Decode from CBOR dicts
decoded = [decode_noise_channel(d) for d in cbor_data]
```

### Validation rules

- `qubit` must be a non-negative integer.
- `p` (depolarizing) must satisfy `0.0 ≤ p ≤ 1.0`.
- `gamma` (amplitude or phase damping) must satisfy `0.0 ≤ gamma ≤ 1.0`.
- The array MAY contain multiple channels for the same qubit.
- An empty `noise_channels` array is valid.

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

## Relationship to `NoiseConstraint`

| Field              | Meaning                                          | Enforcement       |
|--------------------|--------------------------------------------------|-------------------|
| `noise.t1_us`      | Minimum hardware T1 time required                | Compile-time type error if violated |
| `noise.t2_us`      | Minimum hardware T2 time required                | Compile-time type error if violated |
| `noise_channels[]` | Expected per-qubit decoherence during execution  | Optional hint; backends may ignore  |

---

## Changelog

| Version | Change |
|---------|--------|
| v0.1 (QUASI-025) | Added `noise_channels` optional field; defined DepolarizingChannel, AmplitudeDampingChannel, PhaseDampingChannel |
