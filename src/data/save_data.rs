use tch::Tensor;
use crate::mcts::mcts_result::MCTSResult;

pub fn save_game_data(file_path: &str, game_data: Vec<MCTSResult>) {
    println!("🚀 Saving game data to .pt files...");

    let mut tensors = vec![];
    let mut positions = vec![];
    let mut subscores = vec![];

    for result in game_data {
        tensors.push(result.board_tensor.shallow_clone());
        positions.push(result.best_position as i64);
        subscores.push(result.subscore as f32);
    }

    // Création des nouveaux tensors
    let state_tensor = Tensor::stack(&tensors, 0);
    let position_tensor = Tensor::from_slice(&positions).view([-1, 1]);
    let subscore_tensor = Tensor::from_slice(&subscores).view([-1, 1]);

    // 🔄 Append logic: charger les anciens tensors s'ils existent
    let combined_states = if let Ok(prev) = Tensor::load(format!("{}_states.pt", file_path)) {
        Tensor::cat(&[prev, state_tensor], 0)
    } else {
        state_tensor
    };

    let combined_positions = if let Ok(prev) = Tensor::load(format!("{}_positions.pt", file_path)) {
        Tensor::cat(&[prev, position_tensor], 0)
    } else {
        position_tensor
    };

    let combined_subscores = if let Ok(prev) = Tensor::load(format!("{}_subscores.pt", file_path)) {
        Tensor::cat(&[prev, subscore_tensor], 0)
    } else {
        subscore_tensor
    };

    // 🔄 Sauvegarde des tensors concaténés
    if let Err(e) = combined_states.save(format!("{}_states.pt", file_path)) {
        log::info!("❌ Error saving states: {:?}", e);
    }
    if let Err(e) = combined_positions.save(format!("{}_positions.pt", file_path)) {
        log::info!("❌ Error saving positions: {:?}", e);
    }
    if let Err(e) = combined_subscores.save(format!("{}_subscores.pt", file_path)) {
        log::info!("❌ Error saving subscores: {:?}", e);
    }

    log::info!("✅ Save complete!");
}
