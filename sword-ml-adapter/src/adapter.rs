use crate::config::{
    FreezeMap, LrSchedule, ParallelIsland, PartitionStats, TrainableLayer, TrainingConfig,
};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("wrong recommended_path: expected \"ml_training\", got \"{0}\"")]
    WrongPath(String),
    #[error("malformed hierarchical_partition: {0}")]
    Malformed(String),
}

/// Translate a SWORD JSON document into a [`TrainingConfig`].
///
/// Returns `Err` if the document is missing required fields or has
/// `recommended_path != "ml_training"`.
pub fn sword_to_training_config(sword_json: &Value) -> Result<TrainingConfig, AdapterError> {
    // 1. Guard: correct recommended_path
    let path = sword_json
        .get("recommended_path")
        .and_then(Value::as_str)
        .ok_or(AdapterError::MissingField("recommended_path"))?;
    if path != "ml_training" {
        return Err(AdapterError::WrongPath(path.to_owned()));
    }

    // 2. Navigate into hierarchical_partition
    let hp = sword_json
        .get("hierarchical_partition")
        .ok_or(AdapterError::MissingField("hierarchical_partition"))?;

    // 3. Parse layer_partition
    let layer_partition = hp
        .get("layer_partition")
        .and_then(Value::as_array)
        .ok_or(AdapterError::MissingField("layer_partition"))?;

    let mut frozen_layers: Vec<String> = Vec::new();
    // volatile_meta: layer_name -> island_id
    let mut volatile_meta: HashMap<String, u32> = HashMap::new();

    for entry in layer_partition {
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| AdapterError::Malformed("layer_partition entry missing name".into()))?
            .to_owned();
        let role = entry
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| AdapterError::Malformed("layer_partition entry missing role".into()))?;
        match role {
            "stable" => frozen_layers.push(name),
            "volatile" => {
                let island_id = entry
                    .get("island_id")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;
                volatile_meta.insert(name, island_id);
            }
            other => {
                return Err(AdapterError::Malformed(format!(
                    "unknown layer role: {other}"
                )));
            }
        }
    }

    // 4. Parse parameter_partitions
    //    Build: layer_name -> list of param indices marked "train"
    let mut param_train: HashMap<String, Vec<usize>> = HashMap::new();
    if let Some(param_parts) = hp.get("parameter_partitions").and_then(Value::as_array) {
        for pp in param_parts {
            let layer = pp
                .get("layer")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AdapterError::Malformed("parameter_partition missing layer".into())
                })?
                .to_owned();
            let role = pp
                .get("role")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AdapterError::Malformed("parameter_partition missing role".into())
                })?;
            if role == "train" {
                let idx = pp
                    .get("param_index")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        AdapterError::Malformed("parameter_partition missing param_index".into())
                    })? as usize;
                param_train.entry(layer).or_default().push(idx);
            }
        }
    }

    // 5. Build trainable_layers
    let mut trainable_layers: Vec<TrainableLayer> = volatile_meta
        .iter()
        .map(|(name, &island_id)| {
            let param_indices_to_train = param_train.get(name).cloned().unwrap_or_default();
            TrainableLayer {
                name: name.clone(),
                param_indices_to_train,
                island_id,
            }
        })
        .collect();
    // Stable order — deterministic output
    trainable_layers.sort_by(|a, b| a.name.cmp(&b.name));

    // 6. Build parallel islands
    let mut island_map: HashMap<u32, Vec<String>> = HashMap::new();
    for layer in &trainable_layers {
        island_map
            .entry(layer.island_id)
            .or_default()
            .push(layer.name.clone());
    }
    let mut parallel_islands: Vec<ParallelIsland> = island_map
        .into_iter()
        .map(|(island_id, layer_names)| ParallelIsland {
            island_id,
            layer_names,
        })
        .collect();
    parallel_islands.sort_by_key(|i| i.island_id);

    // 7. Suggested LR schedule — volatile islands get 10× the frozen base
    //    We use 1e-4 as the canonical base LR; volatile layers get 1e-3.
    let base_lr_stable = 1e-4_f64;
    let base_lr_volatile = base_lr_stable * 10.0;
    let suggested_lr_schedule: Vec<LrSchedule> = parallel_islands
        .iter()
        .map(|island| LrSchedule {
            island_id: island.island_id,
            base_lr: base_lr_volatile,
        })
        .collect();
    let _ = base_lr_stable; // kept for documentation purposes

    // 8. Stats
    let stats = PartitionStats {
        total_params: hp.get("total_params").and_then(Value::as_u64).unwrap_or(0),
        frozen_params: hp.get("frozen_params").and_then(Value::as_u64).unwrap_or(0),
        trainable_params: hp
            .get("trainable_params")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        trainable_pct: hp
            .get("trainable_pct")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    };

    Ok(TrainingConfig {
        freeze_map: FreezeMap {
            frozen_layers,
            trainable_layers,
        },
        suggested_lr_schedule,
        parallel_islands,
        stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_sword() -> Value {
        json!({
            "recommended_path": "ml_training",
            "hierarchical_partition": {
                "layer_partition": [
                    { "name": "embed", "role": "stable" },
                    { "name": "head",  "role": "volatile", "island_id": 0 },
                    { "name": "lora",  "role": "volatile", "island_id": 1 }
                ],
                "parameter_partitions": [
                    { "layer": "head", "param_index": 0, "role": "train" },
                    { "layer": "head", "param_index": 1, "role": "freeze" },
                    { "layer": "lora", "param_index": 0, "role": "train" }
                ],
                "total_params":     10000,
                "frozen_params":    8000,
                "trainable_params": 2000,
                "trainable_pct":    20.0
            }
        })
    }

    #[test]
    fn frozen_layers_identified() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        assert_eq!(cfg.freeze_map.frozen_layers, vec!["embed"]);
    }

    #[test]
    fn trainable_layers_correct() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        let names: Vec<&str> = cfg
            .freeze_map
            .trainable_layers
            .iter()
            .map(|l| l.name.as_str())
            .collect();
        assert!(names.contains(&"head"));
        assert!(names.contains(&"lora"));
    }

    #[test]
    fn param_indices_filtered() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        let head = cfg
            .freeze_map
            .trainable_layers
            .iter()
            .find(|l| l.name == "head")
            .unwrap();
        // only index 0 is "train"; index 1 is "freeze"
        assert_eq!(head.param_indices_to_train, vec![0]);
    }

    #[test]
    fn parallel_islands_grouped() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        assert_eq!(cfg.parallel_islands.len(), 2);
        let island0 = cfg.parallel_islands.iter().find(|i| i.island_id == 0).unwrap();
        assert_eq!(island0.layer_names, vec!["head"]);
    }

    #[test]
    fn stats_populated() {
        let cfg = sword_to_training_config(&sample_sword()).unwrap();
        assert_eq!(cfg.stats.total_params, 10000);
        assert_eq!(cfg.stats.trainable_pct, 20.0);
    }

    #[test]
    fn wrong_path_rejected() {
        let doc = json!({ "recommended_path": "gate_synthesis" });
        assert!(sword_to_training_config(&doc).is_err());
    }
}
