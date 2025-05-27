use std::path::Path;
use tch::Tensor;
use crate::mcts::mcts_result::MCTSResult;

pub fn load_game_data(file_path: &str) -> Vec<MCTSResult> {
    // Paths for the .pt files
    let states_path = format!("{}_states.pt", file_path);
    let positions_path = format!("{}_positions.pt", file_path);
    let subscores_path = format!("{}_subscores.pt", file_path);

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

    // Load the saved tensors
    let state_tensor = Tensor::load(states_path).expect("Failed to load states");
    let position_tensor = Tensor::load(positions_path).expect("Failed to load positions");
    let subscore_tensor = Tensor::load(subscores_path).expect("Failed to load subscores");

    // Convert them back into MCTSResult objects
    let mut data = Vec::new();
    for i in 0..state_tensor.size()[0] {
        data.push(MCTSResult {
            board_tensor: state_tensor.get(i),
            best_position: position_tensor.get(i).int64_value(&[]) as usize,
            subscore: subscore_tensor.get(i).double_value(&[]),
        });
    }
    println!("✅ Loaded {} game records.", data.len());
    data
}