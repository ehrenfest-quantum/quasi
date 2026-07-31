use crate::config::TrainingConfig;

/// Generate a Python snippet that sets `requires_grad` appropriately for every
/// parameter group described by `config`.
///
/// The snippet is self-contained and assumes:
/// - `model` is a `torch.nn.Module` already instantiated.
/// - `optimizer_groups` will be consumed by a `torch.optim.*` constructor.
///
/// Example output:
/// ```python
/// # SWORD ML Adapter — auto-generated, do not edit by hand
/// # Frozen layers: ['embed']
/// for name, param in model.named_parameters():
///     param.requires_grad = False   # freeze all by default
///
/// # Trainable layer: head (island 0, lr=0.001000)
/// #   param indices to train: [0]
/// for name, param in model.named_parameters():
///     parts = name.split('.')
///     if parts[0] == 'head':
///         idx = int(parts[-1]) if parts[-1].isdigit() else None
///         if idx is None or idx in {0}:
///             param.requires_grad = True
/// ...
/// ```
pub fn to_pytorch_script(config: &TrainingConfig) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push("# SWORD ML Adapter — auto-generated, do not edit by hand".into());

    // Frozen layer summary comment
    if !config.freeze_map.frozen_layers.is_empty() {
        let frozen_list = config
            .freeze_map
            .frozen_layers
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("# Frozen layers: [{frozen_list}]"));
    }

    lines.push(String::new());

    // Step 1: freeze everything
    lines.push("for name, param in model.named_parameters():".into());
    lines.push("    param.requires_grad = False   # freeze all by default".into());
    lines.push(String::new());

    // Step 2: selectively unfreeze trainable layers
    for layer in &config.freeze_map.trainable_layers {
        // Find the LR for this island
        let lr = config
            .suggested_lr_schedule
            .iter()
            .find(|s| s.island_id == layer.island_id)
            .map(|s| s.base_lr)
            .unwrap_or(1e-3);

        lines.push(format!(
            "# Trainable layer: {} (island {}, lr={:.6})",
            layer.name, layer.island_id, lr
        ));

        if layer.param_indices_to_train.is_empty() {
            // All params in this layer are trainable
            lines.push("#   all parameters trainable".to_string());
            lines.push("for name, param in model.named_parameters():".into());
            lines.push("    parts = name.split('.')".into());
            lines.push(format!("    if parts[0] == '{}':", layer.name));
            lines.push("        param.requires_grad = True".into());
        } else {
            // Only specific indices are trainable
            let idx_set = layer
                .param_indices_to_train
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "#   param indices to train: [{}]",
                idx_set
            ));
            lines.push("for name, param in model.named_parameters():".into());
            lines.push("    parts = name.split('.')".into());
            lines.push(format!("    if parts[0] == '{}':", layer.name));
            lines.push("        idx = int(parts[-1]) if parts[-1].isdigit() else None".into());
            lines.push(format!(
                "        if idx is None or idx in {{{}}}:",
                idx_set
            ));
            lines.push("            param.requires_grad = True".into());
        }
        lines.push(String::new());
    }

    // Step 3: optimizer param groups, one per island
    if !config.parallel_islands.is_empty() {
        lines.push("# Optimizer param groups (one per island)".into());
        lines.push("optimizer_groups = []".into());
        for island in &config.parallel_islands {
            let lr = config
                .suggested_lr_schedule
                .iter()
                .find(|s| s.island_id == island.island_id)
                .map(|s| s.base_lr)
                .unwrap_or(1e-3);
            let layer_list = island
                .layer_names
                .iter()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "optimizer_groups.append({{'params': [p for n, p in model.named_parameters() if n.split('.')[0] in [{}] and p.requires_grad], 'lr': {:.6}}})",
                layer_list, lr
            ));
        }
        lines.push(String::new());
        lines.push(
            "# Example: optimizer = torch.optim.AdamW(optimizer_groups)".into(),
        );
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::sword_to_training_config;
    use serde_json::json;

    #[test]
    fn script_contains_freeze_all() {
        let doc = json!({
            "recommended_path": "ml_training",
            "hierarchical_partition": {
                "layer_partition": [
                    { "name": "embed", "role": "stable" },
                    { "name": "head",  "role": "volatile", "island_id": 0 }
                ],
                "parameter_partitions": [],
                "total_params": 100, "frozen_params": 80,
                "trainable_params": 20, "trainable_pct": 20.0
            }
        });
        let cfg = sword_to_training_config(&doc).unwrap();
        let script = to_pytorch_script(&cfg);
        assert!(script.contains("param.requires_grad = False"));
        assert!(script.contains("param.requires_grad = True"));
        assert!(script.contains("optimizer_groups"));
    }

    #[test]
    fn frozen_layers_listed_in_comment() {
        let doc = json!({
            "recommended_path": "ml_training",
            "hierarchical_partition": {
                "layer_partition": [
                    { "name": "backbone", "role": "stable" }
                ],
                "parameter_partitions": [],
                "total_params": 100, "frozen_params": 100,
                "trainable_params": 0, "trainable_pct": 0.0
            }
        });
        let cfg = sword_to_training_config(&doc).unwrap();
        let script = to_pytorch_script(&cfg);
        assert!(script.contains("'backbone'"));
    }
}
