"""Tests for afana.noise_model — QUASI-025 Ehrenfest noise channel spec."""

import pytest

from afana.noise_model import (
    NOISE_TYPE_AMPLITUDE_DAMPING,
    NOISE_TYPE_DEPOLARIZING,
    NOISE_TYPE_PHASE_DAMPING,
    AmplitudeDampingChannel,
    DepolarizingChannel,
    NoiseChannelError,
    PhaseDampingChannel,
    decode_noise_channel,
    encode_noise_channel,
    validate_noise_channel,
    validate_noise_channels,
)


# ---------------------------------------------------------------------------
# Type tag constants
# ---------------------------------------------------------------------------


def test_type_tags():
    assert NOISE_TYPE_DEPOLARIZING == 1
    assert NOISE_TYPE_AMPLITUDE_DAMPING == 2
    assert NOISE_TYPE_PHASE_DAMPING == 3


# ---------------------------------------------------------------------------
# Valid channels
# ---------------------------------------------------------------------------


def test_depolarizing_channel_fields():
    ch = DepolarizingChannel(qubit=0, p=0.01)
    assert ch.qubit == 0
    assert ch.p == 0.01


def test_amplitude_damping_channel_fields():
    ch = AmplitudeDampingChannel(qubit=1, gamma=0.05)
    assert ch.qubit == 1
    assert ch.gamma == 0.05


def test_phase_damping_channel_fields():
    ch = PhaseDampingChannel(qubit=2, gamma=0.03)
    assert ch.qubit == 2
    assert ch.gamma == 0.03


# ---------------------------------------------------------------------------
# Validation — valid cases
# ---------------------------------------------------------------------------


def test_validate_depolarizing_zero_ok():
    validate_noise_channel(DepolarizingChannel(qubit=0, p=0.0))


def test_validate_depolarizing_one_ok():
    validate_noise_channel(DepolarizingChannel(qubit=0, p=1.0))


def test_validate_amplitude_damping_boundaries():
    validate_noise_channel(AmplitudeDampingChannel(qubit=0, gamma=0.0))
    validate_noise_channel(AmplitudeDampingChannel(qubit=0, gamma=1.0))


def test_validate_phase_damping_boundaries():
    validate_noise_channel(PhaseDampingChannel(qubit=0, gamma=0.0))
    validate_noise_channel(PhaseDampingChannel(qubit=0, gamma=1.0))


def test_validate_list_of_channels():
    channels = [
        DepolarizingChannel(qubit=0, p=0.01),
        AmplitudeDampingChannel(qubit=1, gamma=0.02),
        PhaseDampingChannel(qubit=0, gamma=0.015),
    ]
    validate_noise_channels(channels)  # should not raise


def test_validate_empty_list_ok():
    validate_noise_channels([])  # empty list is valid


# ---------------------------------------------------------------------------
# Validation — error cases
# ---------------------------------------------------------------------------


def test_depolarizing_p_above_one_raises():
    with pytest.raises(NoiseChannelError, match="out of range"):
        validate_noise_channel(DepolarizingChannel(qubit=0, p=1.01))


def test_depolarizing_p_negative_raises():
    with pytest.raises(NoiseChannelError, match="out of range"):
        validate_noise_channel(DepolarizingChannel(qubit=0, p=-0.001))


def test_amplitude_damping_gamma_above_one_raises():
    with pytest.raises(NoiseChannelError, match="out of range"):
        validate_noise_channel(AmplitudeDampingChannel(qubit=0, gamma=1.5))


def test_phase_damping_gamma_negative_raises():
    with pytest.raises(NoiseChannelError, match="out of range"):
        validate_noise_channel(PhaseDampingChannel(qubit=0, gamma=-0.1))


def test_negative_qubit_index_raises():
    with pytest.raises(NoiseChannelError, match="qubit index"):
        validate_noise_channel(DepolarizingChannel(qubit=-1, p=0.01))


def test_invalid_channel_type_raises():
    with pytest.raises(NoiseChannelError):
        validate_noise_channel("not_a_channel")  # type: ignore


def test_validate_list_with_invalid_channel_raises():
    channels = [
        DepolarizingChannel(qubit=0, p=0.01),
        AmplitudeDampingChannel(qubit=0, gamma=2.0),  # invalid
    ]
    with pytest.raises(NoiseChannelError, match="out of range"):
        validate_noise_channels(channels)


# ---------------------------------------------------------------------------
# Encoding
# ---------------------------------------------------------------------------


def test_encode_depolarizing():
    ch = DepolarizingChannel(qubit=0, p=0.01)
    data = encode_noise_channel(ch)
    assert data == {"type": 1, "qubit": 0, "p": 0.01}


def test_encode_amplitude_damping():
    ch = AmplitudeDampingChannel(qubit=2, gamma=0.05)
    data = encode_noise_channel(ch)
    assert data == {"type": 2, "qubit": 2, "gamma": 0.05}


def test_encode_phase_damping():
    ch = PhaseDampingChannel(qubit=1, gamma=0.03)
    data = encode_noise_channel(ch)
    assert data == {"type": 3, "qubit": 1, "gamma": 0.03}


def test_encode_unknown_type_raises():
    with pytest.raises(TypeError):
        encode_noise_channel("bad")  # type: ignore


# ---------------------------------------------------------------------------
# Decoding
# ---------------------------------------------------------------------------


def test_decode_depolarizing():
    ch = decode_noise_channel({"type": 1, "qubit": 0, "p": 0.01})
    assert isinstance(ch, DepolarizingChannel)
    assert ch.qubit == 0
    assert ch.p == 0.01


def test_decode_amplitude_damping():
    ch = decode_noise_channel({"type": 2, "qubit": 3, "gamma": 0.07})
    assert isinstance(ch, AmplitudeDampingChannel)
    assert ch.qubit == 3
    assert ch.gamma == 0.07


def test_decode_phase_damping():
    ch = decode_noise_channel({"type": 3, "qubit": 1, "gamma": 0.04})
    assert isinstance(ch, PhaseDampingChannel)
    assert ch.qubit == 1
    assert ch.gamma == 0.04


def test_decode_unknown_type_raises():
    with pytest.raises(NoiseChannelError, match="Unknown noise channel type"):
        decode_noise_channel({"type": 99, "qubit": 0, "p": 0.01})


def test_decode_missing_type_raises():
    with pytest.raises(NoiseChannelError, match="Unknown noise channel type"):
        decode_noise_channel({"qubit": 0, "p": 0.01})


# ---------------------------------------------------------------------------
# Round-trip encode → decode
# ---------------------------------------------------------------------------


def test_roundtrip_depolarizing():
    original = DepolarizingChannel(qubit=0, p=0.015)
    assert decode_noise_channel(encode_noise_channel(original)) == original


def test_roundtrip_amplitude_damping():
    original = AmplitudeDampingChannel(qubit=2, gamma=0.08)
    assert decode_noise_channel(encode_noise_channel(original)) == original


def test_roundtrip_phase_damping():
    original = PhaseDampingChannel(qubit=1, gamma=0.02)
    assert decode_noise_channel(encode_noise_channel(original)) == original


def test_roundtrip_list():
    channels = [
        DepolarizingChannel(qubit=0, p=0.01),
        AmplitudeDampingChannel(qubit=0, gamma=0.05),
        PhaseDampingChannel(qubit=1, gamma=0.03),
    ]
    decoded = [decode_noise_channel(encode_noise_channel(ch)) for ch in channels]
    assert decoded == channels
