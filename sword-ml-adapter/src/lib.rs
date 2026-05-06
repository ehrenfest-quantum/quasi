/// SWORD ML Adapter
///
/// Translates a SWORD document with `recommended_path = "ml_training"` into
/// a [`TrainingConfig`] that downstream PyTorch / JAX code can consume.
///
/// SWORD structure consumed here:
/// ```json
/// {
///   "recommended_path": "ml_training",
///   "hierarchical_partition": {
///     "layer_partition": [
///       { "name": "embed", "role": "stable" },
///       { "name": "head",  "role": "volatile", "island_id": 0 }
///     ],
///     "parameter_partitions": [
///       { "layer": "head", "param_index": 0, "role": "train" },
///       { "layer": "head", "param_index": 1, "role": "freeze" }
///     ],
///     "total_params":     10000,
///     "frozen_params":    8000,
///     "trainable_params": 2000,
///     "trainable_pct":    20.0
///   }
/// }
/// ```
pub mod adapter;
pub mod config;
pub mod onnx;
pub mod pytorch;

pub use adapter::sword_to_training_config;
pub use config::{FreezeMap, LrSchedule, ParallelIsland, TrainableLayer, TrainingConfig};
pub use onnx::{to_onnx_json, to_onnx_training_spec, OnnxTrainingSpec};
pub use pytorch::to_pytorch_script;
