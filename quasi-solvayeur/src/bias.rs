// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//! Adaptive bias update for the ATW algorithm.
//!
//! The ATW bias update rule:
//!   h_i(k+1) = (1 - lambda) * h_i(k) + eta * r(k) * (-1)^{b(k)[i]}
//!
//! Where:
//! - h_i is the bias field for qubit i
//! - lambda is the decay rate (forgetting factor, 0 to 1)
//! - eta is the learning rate
//! - r(k) is the observed reward (0 to 1)
//! - b(k)[i] is bit i of the measurement outcome

/// ATW learning parameters.
#[derive(Debug, Clone)]
pub struct AtwLearning {
    /// Learning rate eta: how fast new observations shift the bias.
    pub eta: f64,
    /// Decay rate lambda: how fast old preferences are forgotten.
    pub lambda: f64,
}

impl Default for AtwLearning {
    fn default() -> Self {
        Self {
            eta: 0.3,
            lambda: 0.1,
        }
    }
}

/// Compute the reward for a dispatching outcome.
///
/// Reward combines fidelity, speed, and cost into [0, 1]:
///   r = w_f * fidelity + w_s * speed_score + w_c * cost_score
#[derive(Debug, Clone)]
pub struct Reward {
    /// Result fidelity, 0 to 1.
    pub fidelity: f64,
    /// Job latency in milliseconds (lower is better).
    pub latency_ms: f64,
    /// Cost of the job (lower is better).
    pub cost: f64,
}

impl Reward {
    /// Compute scalar reward in [0, 1].
    pub fn score(&self) -> f64 {
        let speed_score = 1.0 / (1.0 + self.latency_ms / 1000.0);
        let cost_score = 1.0 / (1.0 + self.cost);
        // Weighted combination
        let r = 0.5 * self.fidelity + 0.3 * speed_score + 0.2 * cost_score;
        r.clamp(0.0, 1.0)
    }
}

/// Update bias fields after one ATW round.
///
/// Returns the new bias vector.
pub fn update_bias(
    current_bias: &[f64],
    measurement: &[bool], // bitstring as bools (true = |1>)
    reward: &Reward,
    learning: &AtwLearning,
) -> Vec<f64> {
    let r = reward.score();
    current_bias
        .iter()
        .enumerate()
        .map(|(i, &h)| {
            // (-1)^{b[i]}: +1 if bit is 0, -1 if bit is 1
            let sign = if i < measurement.len() && measurement[i] {
                -1.0
            } else {
                1.0
            };
            (1.0 - learning.lambda) * h + learning.eta * r * sign
        })
        .collect()
}

/// Decode a measurement bitstring to a backend index.
///
/// The bitstring is interpreted as a binary number (little-endian).
/// If the index exceeds the number of backends, wrap around (modulo).
pub fn decode_backend(measurement: &[bool], n_backends: usize) -> usize {
    if n_backends == 0 {
        return 0;
    }
    let mut index: usize = 0;
    for (i, &bit) in measurement.iter().enumerate() {
        if bit {
            index |= 1 << i;
        }
    }
    index % n_backends
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_bias_positive_reward_shifts_toward_selected_state() {
        let bias = vec![0.0, 0.0];
        let measurement = vec![true, false]; // selected state: qubit 0 = |1>, qubit 1 = |0>
        let reward = Reward {
            fidelity: 1.0,
            latency_ms: 0.0,
            cost: 0.0,
        };
        let learning = AtwLearning::default();

        let new_bias = update_bias(&bias, &measurement, &reward, &learning);

        // Qubit 0 measured |1> → sign = -1, so bias decreases (goes negative)
        assert!(new_bias[0] < 0.0, "qubit 0 bias should decrease: {}", new_bias[0]);
        // Qubit 1 measured |0> → sign = +1, so bias increases (goes positive)
        assert!(new_bias[1] > 0.0, "qubit 1 bias should increase: {}", new_bias[1]);
    }

    #[test]
    fn update_bias_zero_reward_decays() {
        let bias = vec![1.0, -0.5];
        let measurement = vec![false, false];
        let reward = Reward {
            fidelity: 0.0,
            latency_ms: f64::MAX,
            cost: f64::MAX,
        };
        let learning = AtwLearning {
            eta: 0.3,
            lambda: 0.1,
        };

        let new_bias = update_bias(&bias, &measurement, &reward, &learning);

        // With zero reward, bias should decay toward zero
        assert!(new_bias[0].abs() < bias[0].abs(), "bias[0] should decay");
        assert!(new_bias[1].abs() < bias[1].abs(), "bias[1] should decay");
    }

    #[test]
    fn decode_backend_correct_indices() {
        // [false, false] = 0b00 = 0
        assert_eq!(decode_backend(&[false, false], 4), 0);
        // [true, false] = 0b01 = 1
        assert_eq!(decode_backend(&[true, false], 4), 1);
        // [false, true] = 0b10 = 2
        assert_eq!(decode_backend(&[false, true], 4), 2);
        // [true, true] = 0b11 = 3
        assert_eq!(decode_backend(&[true, true], 4), 3);
    }

    #[test]
    fn decode_backend_wraps_around() {
        // [true, true] = 3, but only 2 backends → 3 % 2 = 1
        assert_eq!(decode_backend(&[true, true], 2), 1);
        // [true, true, true] = 7, but only 5 backends → 7 % 5 = 2
        assert_eq!(decode_backend(&[true, true, true], 5), 2);
    }

    #[test]
    fn decode_backend_zero_backends_returns_zero() {
        assert_eq!(decode_backend(&[true, false], 0), 0);
    }

    #[test]
    fn reward_score_in_valid_range() {
        let cases = vec![
            Reward { fidelity: 1.0, latency_ms: 0.0, cost: 0.0 },
            Reward { fidelity: 0.0, latency_ms: 10000.0, cost: 100.0 },
            Reward { fidelity: 0.5, latency_ms: 500.0, cost: 1.0 },
            Reward { fidelity: 0.0, latency_ms: 0.0, cost: 0.0 },
        ];
        for reward in &cases {
            let score = reward.score();
            assert!(
                (0.0..=1.0).contains(&score),
                "score {score} out of range for {reward:?}"
            );
        }
    }

    #[test]
    fn reward_perfect_score_is_high() {
        let reward = Reward {
            fidelity: 1.0,
            latency_ms: 0.0,
            cost: 0.0,
        };
        assert!(reward.score() > 0.9);
    }

    #[test]
    fn default_learning_params_are_reasonable() {
        let learning = AtwLearning::default();
        assert!(learning.eta > 0.0 && learning.eta < 1.0);
        assert!(learning.lambda > 0.0 && learning.lambda < 1.0);
    }
}
