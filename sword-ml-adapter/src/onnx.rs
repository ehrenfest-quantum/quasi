//! ONNX Training Specification output.
//!
//! Instead of generating a PyTorch script, this emits a framework-agnostic
//! ONNX-compatible training specification that describes:
//! - Which graph nodes (layers) are frozen vs trainable
//! - Per-island optimizer groups with LR recommendations
//! - Parallel training structure
//!
//! Output is a JSON document that can be consumed by:
//! - onnxruntime TrainingSession (via training_info)
//! - Nathan's existing ONNX pipeline (garm-analysis)
//! - Any custom training loop that reads freeze maps
//!
//! We deliberately do NOT generate an ONNX protobuf here — that requires
//! the full onnx crate and a model file to modify. Instead we emit the
//! SPECIFICATION that a thin wrapper (Python or Rust+ort) applies to a
//! model. This keeps the adapter dependency-free.
use crate::config::TrainingConfig;
use serde::{Deserialize, Serialize};

/// ONNX-compatible training specification.
///
/// Designed to be consumed by `onnxruntime.training.api.Module` or
/// applied to an ONNX graph via `onnx.helper.make_training_info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnnxTrainingSpec {
    /// Schema version for forward compatibility.
    pub version: u32,
    /// Frozen parameters — these become non-trainable initializers.
    pub frozen_params: Vec<OnnxFrozenParam>,
    /// Trainable parameter groups — one per island.
    pub trainable_groups: Vec<OnnxTrainableGroup>,
    /// Summary statistics.
    pub stats: OnnxTrainingStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnnxFrozenParam {
    /// ONNX graph node name pattern (matches named_parameters convention).
    /// e.g. "encoder.layer.0.weight", "embed.weight"
    pub name_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnnxTrainableGroup {
    /// Island ID for parallel training.
    pub island_id: u32,
    /// Parameter name patterns in this group.
    pub param_patterns: Vec<String>,
    /// Specific parameter indices within each layer (empty = all params).
    pub param_indices: Vec<usize>,
    /// Recommended learning rate for this group.
    pub learning_rate: f64,
    /// Weight decay recommendation (0.01 default, 0.0 for bias/norm).
    pub weight_decay: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnnxTrainingStats {
    pub total_params: u64,
    pub frozen_params: u64,
    pub trainable_params: u64,
    pub trainable_pct: f64,
    pub n_islands: u32,
}

/// Convert a TrainingConfig into an ONNX-compatible training specification.
pub fn to_onnx_training_spec(config: &TrainingConfig) -> OnnxTrainingSpec {
    let frozen_params = config
        .freeze_map
        .frozen_layers
        .iter()
        .map(|name| OnnxFrozenParam {
            name_pattern: format!("{name}.*"),
        })
        .collect();

    let trainable_groups = config
        .parallel_islands
        .iter()
        .map(|island| {
            let lr = config
                .suggested_lr_schedule
                .iter()
                .find(|s| s.island_id == island.island_id)
                .map(|s| s.base_lr)
                .unwrap_or(1e-3);

            // Collect param indices from all trainable layers in this island
            let param_indices: Vec<usize> = config
                .freeze_map
                .trainable_layers
                .iter()
                .filter(|l| l.island_id == island.island_id)
                .flat_map(|l| l.param_indices_to_train.iter().copied())
                .collect();

            OnnxTrainableGroup {
                island_id: island.island_id,
                param_patterns: island
                    .layer_names
                    .iter()
                    .map(|n| format!("{n}.*"))
                    .collect(),
                param_indices,
                learning_rate: lr,
                weight_decay: 0.01,
            }
        })
        .collect();

    OnnxTrainingSpec {
        version: 1,
        frozen_params,
        trainable_groups,
        stats: OnnxTrainingStats {
            total_params: config.stats.total_params,
            frozen_params: config.stats.frozen_params,
            trainable_params: config.stats.trainable_params,
            trainable_pct: config.stats.trainable_pct,
            n_islands: config.parallel_islands.len() as u32,
        },
    }
}

/// Serialize the spec as JSON — the primary wire format.
pub fn to_onnx_json(config: &TrainingConfig) -> String {
    let spec = to_onnx_training_spec(config);
    serde_json::to_string_pretty(&spec).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::sword_to_training_config;
    use serde_json::json;

    fn sample_sword() -> serde_json::Value {
        json!({
            "recommended_path": "ml_training",
            "hierarchical_partition": {
                "layer_partition": [
                    { "name": "encoder", "role": "stable" },
                    { "name": "decoder", "role": "stable" },
                    { "name": "head", "role": "volatile", "island_id": 0 },
                    { "name": "classifier", "role": "volatile", "island_id": 1 }
                ],
                "parameter_partitions": [
                    { "layer": "head", "param_index": 0, "role": "train" },
                    { "layer": "head", "param_index": 1, "role": "freeze" },
                    { "layer": "classifier", "param_index": 0, "role": "train" }
                ],
                "total_params": 50000,
                "frozen_params": 45000,
                "trainable_params": 5000,
                "trainable_pct": 10.0
            }
        })
    }

    #[test]
    fn onnx_spec_has_frozen_layers() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        let spec = to_onnx_training_spec(&cfg);
        assert_eq!(spec.frozen_params.len(), 2);
        assert!(spec.frozen_params.iter().any(|p| p.name_pattern == "encoder.*"));
        assert!(spec.frozen_params.iter().any(|p| p.name_pattern == "decoder.*"));
    }

    #[test]
    fn onnx_spec_has_trainable_groups() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        let spec = to_onnx_training_spec(&cfg);
        assert_eq!(spec.trainable_groups.len(), 2);
        assert!(spec.trainable_groups[0].learning_rate > 0.0);
    }

    #[test]
    fn onnx_spec_preserves_stats() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        let spec = to_onnx_training_spec(&cfg);
        assert_eq!(spec.stats.total_params, 50000);
        assert_eq!(spec.stats.trainable_params, 5000);
        assert!((spec.stats.trainable_pct - 10.0).abs() < 0.1);
    }

    #[test]
    fn onnx_json_is_valid() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        let json_str = to_onnx_json(&cfg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["version"], 1);
        assert!(parsed["frozen_params"].is_array());
        assert!(parsed["trainable_groups"].is_array());
    }

    #[test]
    fn onnx_spec_param_indices_propagated() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        let spec = to_onnx_training_spec(&cfg);
        // Island 0 (head) should have param_index 0
        let island_0 = spec.trainable_groups.iter().find(|g| g.island_id == 0).unwrap();
        assert!(island_0.param_indices.contains(&0));
        // param_index 1 was "freeze", should not appear
        assert!(!island_0.param_indices.contains(&1));
    }
}
