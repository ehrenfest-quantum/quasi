"""Ehrenfest noise channel annotations (QUASI-025).

Python types and validation for the three decoherence channels defined in
the Ehrenfest CBOR schema (``spec/ehrenfest-v0.1.cddl``):

- Depolarizing noise      (type=1): ``p ∈ [0, 1]``
- Amplitude damping       (type=2): ``gamma ∈ [0, 1]``
- Phase damping           (type=3): ``gamma ∈ [0, 1]``

Noise channels are optional metadata on an Ehrenfest program.  Backends may
use them to drive realistic noise simulation or to select an appropriate
error-mitigation strategy.

Typical usage::

    from afana.noise_model import DepolarizingChannel, encode_noise_channel, validate_noise_channels

    channels = [DepolarizingChannel(qubit=0, p=0.01), DepolarizingChannel(qubit=1, p=0.01)]
    validate_noise_channels(channels)
    cbor_data = [encode_noise_channel(ch) for ch in channels]
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List, Union

# ---------------------------------------------------------------------------
# Type tags (match CDDL literal values)
# ---------------------------------------------------------------------------

NOISE_TYPE_DEPOLARIZING: int = 1
NOISE_TYPE_AMPLITUDE_DAMPING: int = 2
NOISE_TYPE_PHASE_DAMPING: int = 3

# ---------------------------------------------------------------------------
# Channel dataclasses
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class DepolarizingChannel:
    """Depolarizing noise on *qubit*: uniform Pauli error with probability *p*."""

    qubit: int
    p: float


@dataclass(frozen=True)
class AmplitudeDampingChannel:
    """Amplitude damping on *qubit*: T1 energy-relaxation with Kraus strength *gamma*."""

    qubit: int
    gamma: float


@dataclass(frozen=True)
class PhaseDampingChannel:
    """Phase damping on *qubit*: pure T2* dephasing with strength *gamma*."""

    qubit: int
    gamma: float


#: Union type covering all three supported noise channels.
NoiseChannel = Union[DepolarizingChannel, AmplitudeDampingChannel, PhaseDampingChannel]

# ---------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------


class NoiseChannelError(ValueError):
    """Raised when a noise channel fails schema validation."""


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------


def validate_noise_channel(channel: NoiseChannel) -> None:
    """Validate a single noise channel against the Ehrenfest schema.

    Raises:
        NoiseChannelError: If any parameter is out of range.
    """
    if not isinstance(channel, (DepolarizingChannel, AmplitudeDampingChannel, PhaseDampingChannel)):
        raise NoiseChannelError(f"Unknown noise channel type: {type(channel)}")

    if channel.qubit < 0:
        raise NoiseChannelError(
            f"qubit index must be >= 0, got {channel.qubit}"
        )

    if isinstance(channel, DepolarizingChannel):
        if not 0.0 <= channel.p <= 1.0:
            raise NoiseChannelError(
                f"Depolarizing p={channel.p} is out of range [0, 1]"
            )
    else:
        if not 0.0 <= channel.gamma <= 1.0:
            raise NoiseChannelError(
                f"{type(channel).__name__} gamma={channel.gamma} is out of range [0, 1]"
            )


def validate_noise_channels(channels: List[NoiseChannel]) -> None:
    """Validate a list of noise channels.

    Raises:
        NoiseChannelError: If any channel is invalid.
    """
    for ch in channels:
        validate_noise_channel(ch)


# ---------------------------------------------------------------------------
# Encoding / decoding (CBOR-compatible dicts)
# ---------------------------------------------------------------------------


def encode_noise_channel(channel: NoiseChannel) -> Dict:
    """Encode *channel* as a CBOR-compatible dict.

    The integer ``"type"`` key matches the CDDL literal tag so that the dict
    can be serialised directly with any CBOR library.

    Returns:
        Dict with ``"type"``, ``"qubit"``, and one of ``"p"`` or ``"gamma"``.

    Raises:
        TypeError: If *channel* is not a recognised noise channel type.
    """
    if isinstance(channel, DepolarizingChannel):
        return {"type": NOISE_TYPE_DEPOLARIZING, "qubit": channel.qubit, "p": channel.p}
    if isinstance(channel, AmplitudeDampingChannel):
        return {"type": NOISE_TYPE_AMPLITUDE_DAMPING, "qubit": channel.qubit, "gamma": channel.gamma}
    if isinstance(channel, PhaseDampingChannel):
        return {"type": NOISE_TYPE_PHASE_DAMPING, "qubit": channel.qubit, "gamma": channel.gamma}
    raise TypeError(f"Cannot encode unknown noise channel: {type(channel)}")


def decode_noise_channel(data: Dict) -> NoiseChannel:
    """Decode a CBOR dict into a typed ``NoiseChannel``.

    Args:
        data: Dict with at least ``"type"`` and ``"qubit"`` keys.

    Returns:
        One of :class:`DepolarizingChannel`, :class:`AmplitudeDampingChannel`,
        or :class:`PhaseDampingChannel`.

    Raises:
        NoiseChannelError: If ``"type"`` is missing or unrecognised.
    """
    t = data.get("type")
    qubit = int(data["qubit"])

    if t == NOISE_TYPE_DEPOLARIZING:
        return DepolarizingChannel(qubit=qubit, p=float(data["p"]))
    if t == NOISE_TYPE_AMPLITUDE_DAMPING:
        return AmplitudeDampingChannel(qubit=qubit, gamma=float(data["gamma"]))
    if t == NOISE_TYPE_PHASE_DAMPING:
        return PhaseDampingChannel(qubit=qubit, gamma=float(data["gamma"]))

    raise NoiseChannelError(
        f"Unknown noise channel type tag: {t!r}. Expected 1, 2, or 3."
    )
