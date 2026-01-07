use crate::mcts::mcts_result::MCTSResult;
use crate::neural::manager::NNArchitecture;
use std::path::Path;
use tch::{IndexOp, Kind, Tensor};

#[allow(dead_code)]
pub fn load_game_data(file_path: &str) -> Vec<MCTSResult> {
    load_game_data_with_arch(file_path, NNArchitecture::Cnn)
}

pub fn load_game_data_with_arch(file_path: &str, arch: NNArchitecture) -> Vec<MCTSResult> {
    // Ajouter le suffixe d'architecture au chemin
    let arch_suffix = match arch {
        NNArchitecture::Cnn => "_cnn",
        NNArchitecture::Gnn => "_gnn",
    };
    let prefixed_path = format!("{}{}", file_path, arch_suffix);

    // Paths for the .pt files
    let states_path = format!("{}_states.pt", prefixed_path);
    let positions_path = format!("{}_positions.pt", prefixed_path);
    let subscores_path = format!("{}_subscores.pt", prefixed_path);

    // Check if all files exist
    if !Path::new(&states_path).exists() {
        println!(
            "⚠️  Warning: '{}' not found. Returning empty dataset.",
            states_path
        );
        return Vec::new();
    }
    if !Path::new(&positions_path).exists() {
        println!(
            "⚠️  Warning: '{}' not found. Returning empty dataset.",
            positions_path
        );
        return Vec::new();
    }
    if !Path::new(&subscores_path).exists() {
        println!(
            "⚠️  Warning: '{}' not found. Returning empty dataset.",
            subscores_path
        );
        return Vec::new();
    }

    println!("🚀 Loading game data from .pt files...");

    // Load the saved tensors (with error handling)
    let state_tensor = match Tensor::load(&states_path) {
        Ok(tensor) => {
            // Fix: squeeze dimension 1 if shape is [N, 1, C, H, W] -> [N, C, H, W]
            if tensor.size().len() == 5 && tensor.size()[1] == 1 {
                tensor.squeeze_dim(1)
            } else {
                tensor
            }
        },
        Err(e) => {
            eprintln!("⚠️  Error loading states from '{}': {}. Returning empty dataset.", states_path, e);
            return Vec::new();
        }
    };
    let position_tensor = match Tensor::load(&positions_path) {
        Ok(tensor) => tensor,
        Err(e) => {
            eprintln!("⚠️  Error loading positions from '{}': {}. Returning empty dataset.", positions_path, e);
            return Vec::new();
        }
    };
    let subscore_tensor = match Tensor::load(&subscores_path) {
        Ok(tensor) => tensor,
        Err(e) => {
            eprintln!("⚠️  Error loading subscores from '{}': {}. Returning empty dataset.", subscores_path, e);
            return Vec::new();
        }
    };
    let policy_raw_path = format!("{}_policy_raw.pt", prefixed_path);
    let policy_boosted_path = format!("{}_policy_boosted.pt", prefixed_path);
    let boosts_path = format!("{}_boosts.pt", prefixed_path);

    let policy_raw_tensor = if Path::new(&policy_raw_path).exists() {
        match Tensor::load(&policy_raw_path) {
            Ok(tensor) => Some(tensor),
            Err(e) => {
                eprintln!("⚠️  Warning: Failed to load policy_raw from '{}': {}. Skipping.", policy_raw_path, e);
                None
            }
        }
    } else {
        None
    };

    let policy_boosted_tensor = if Path::new(&policy_boosted_path).exists() {
        match Tensor::load(&policy_boosted_path) {
            Ok(tensor) => Some(tensor),
            Err(e) => {
                eprintln!("⚠️  Warning: Failed to load policy_boosted from '{}': {}. Skipping.", policy_boosted_path, e);
                None
            }
        }
    } else {
        None
    };

    let boosts_tensor = if Path::new(&boosts_path).exists() {
        match Tensor::load(&boosts_path) {
            Ok(tensor) => Some(tensor),
            Err(e) => {
                eprintln!("⚠️  Warning: Failed to load boost tensor from '{}': {}. Skipping.", boosts_path, e);
                None
            }
        }
    } else {
        None
    };

    // Convert them back into MCTSResult objects
    let mut data = Vec::new();
    for i in 0..state_tensor.size()[0] {
        let best_position = position_tensor.get(i).int64_value(&[]) as usize;
        let policy_distribution = if let Some(ref policies) = policy_raw_tensor {
            if i < policies.size()[0] {
                policies.get(i)
            } else {
                build_one_hot(best_position)
            }
        } else {
            build_one_hot(best_position)
        };

        let policy_distribution_boosted = if let Some(ref policies) = policy_boosted_tensor {
            if i < policies.size()[0] {
                policies.get(i)
            } else {
                build_one_hot(best_position)
            }
        } else {
            policy_distribution.shallow_clone()
        };

        let boost_intensity = if let Some(ref boosts) = boosts_tensor {
            if i < boosts.size()[0] {
                boosts.get(i).double_value(&[]) as f32
            } else {
                0.0
            }
        } else {
            0.0
        };

        data.push(MCTSResult {
            board_tensor: state_tensor.get(i),
            best_position,
            subscore: subscore_tensor.get(i).double_value(&[]),
            policy_distribution,
            policy_distribution_boosted,
            boost_intensity,
            graph_features: None,
            plateau: None,
            current_turn: None,
            total_turns: None,
            q_value_distribution: None,
        });
    }
    println!("✅ Loaded {} game records.", data.len());
    data
}

fn build_one_hot(best_position: usize) -> Tensor {
    let one_hot = Tensor::zeros([19], (Kind::Float, tch::Device::Cpu));
    let clamped = best_position.min(18) as i64;
    let _ = one_hot.i(clamped).fill_(1.0);
    one_hot
}
