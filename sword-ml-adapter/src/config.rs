use serde::{Deserialize, Serialize};

/// A single trainable layer and which of its parameter indices are active.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainableLayer {
    /// Layer name as it appears in the model's named_parameters().
    pub name: String,
    /// Indices of parameters within this layer that should be trained.
    /// An empty vec means every parameter in the layer is trainable.
    pub param_indices_to_train: Vec<usize>,
    /// Island id — layers sharing the same island can be trained in one
    /// parallel group (e.g. data-parallel or tensor-parallel slice).
    pub island_id: u32,
}

/// Which layers are frozen and which are trainable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeMap {
    pub frozen_layers: Vec<String>,
    pub trainable_layers: Vec<TrainableLayer>,
}

/// Per-island learning-rate suggestion.
///
/// Volatile (trainable) layers are assigned a higher LR than stable ones.
/// The exact scaling factor (10×) is intentionally conservative and should
/// be tuned per experiment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrSchedule {
    pub island_id: u32,
    /// Suggested base learning rate for this island.
    pub base_lr: f64,
}

/// A parallel training island — all layers here share gradient sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelIsland {
    pub island_id: u32,
    pub layer_names: Vec<String>,
}

/// Full training configuration derived from a SWORD document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub freeze_map: FreezeMap,
    /// Per-island LR suggestion.  Islands with more volatile parameters
    /// receive a higher recommended LR.
    pub suggested_lr_schedule: Vec<LrSchedule>,
    /// Groups of layers that can be trained independently in parallel.
    pub parallel_islands: Vec<ParallelIsland>,
    /// Snapshot of the partition statistics from the SWORD document.
    pub stats: PartitionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionStats {
    pub total_params: u64,
    pub frozen_params: u64,
    pub trainable_params: u64,
    pub trainable_pct: f64,
}
