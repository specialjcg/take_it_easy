use crate::mcts::mcts_result::MCTSResult;
use tch::{Kind, Tensor};

pub fn save_game_data(file_path: &str, game_data: Vec<MCTSResult>) {
    println!("🚀 Saving game data to .pt files...");

    let mut tensors = vec![];
    let mut positions = vec![];
    let mut subscores = vec![];
    let mut policies_raw = vec![];
    let mut policies_boosted = vec![];
    let mut boosts = vec![];

    for result in game_data {
        tensors.push(result.board_tensor.shallow_clone());
        positions.push(result.best_position as i64);
        subscores.push(result.subscore as f32);
        policies_raw.push(
            result
                .policy_distribution
                .to_kind(Kind::Float)
                .flatten(0, -1)
                .shallow_clone(),
        );
        policies_boosted.push(
            result
                .policy_distribution_boosted
                .to_kind(Kind::Float)
                .flatten(0, -1)
                .shallow_clone(),
        );
        boosts.push(result.boost_intensity);
    }

    // Création des nouveaux tensors
    let state_tensor = Tensor::stack(&tensors, 0);
    let position_tensor = Tensor::from_slice(&positions).view([-1, 1]);
    let subscore_tensor = Tensor::from_slice(&subscores).view([-1, 1]);
    let policy_raw_tensor = Tensor::stack(&policies_raw, 0);
    let policy_boosted_tensor = Tensor::stack(&policies_boosted, 0);
    let boost_tensor = Tensor::from_slice(&boosts).view([-1, 1]);

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

    let combined_policy_raw = match Tensor::load(format!("{}_policy_raw.pt", file_path)) {
        Ok(prev) => {
            let prev_rows = prev.size()[0];
            let total_rows = combined_positions.size()[0];
            let new_rows = policy_raw_tensor.size()[0];
            if prev_rows + new_rows == total_rows {
                Tensor::cat(&[prev, policy_raw_tensor], 0)
            } else {
                rebuild_policy(&combined_positions, &policy_raw_tensor)
            }
        }
        Err(_) => rebuild_policy(&combined_positions, &policy_raw_tensor),
    };

    let combined_policy_boosted = match Tensor::load(format!("{}_policy_boosted.pt", file_path)) {
        Ok(prev) => {
            let prev_rows = prev.size()[0];
            let total_rows = combined_positions.size()[0];
            let new_rows = policy_boosted_tensor.size()[0];
            if prev_rows + new_rows == total_rows {
                Tensor::cat(&[prev, policy_boosted_tensor], 0)
            } else {
                rebuild_policy(&combined_positions, &policy_boosted_tensor)
            }
        }
        Err(_) => rebuild_policy(&combined_positions, &policy_boosted_tensor),
    };

    let combined_boosts = if let Ok(prev) = Tensor::load(format!("{}_boosts.pt", file_path)) {
        Tensor::cat(&[prev, boost_tensor], 0)
    } else {
        boost_tensor
    };

    // 🔄 Sauvegarde des tensors concaténés
    if let Err(_e) = combined_states.save(format!("{}_states.pt", file_path)) {}
    if let Err(_e) = combined_positions.save(format!("{}_positions.pt", file_path)) {}
    if let Err(_e) = combined_subscores.save(format!("{}_subscores.pt", file_path)) {}
    if let Err(_e) = combined_policy_raw.save(format!("{}_policy_raw.pt", file_path)) {}
    if let Err(_e) = combined_policy_boosted.save(format!("{}_policy_boosted.pt", file_path)) {}
    if let Err(_e) = combined_boosts.save(format!("{}_boosts.pt", file_path)) {}
}

fn positions_to_one_hot(positions: &Tensor, policy_len: i64) -> Tensor {
    let num_samples = positions.size()[0];
    let vocab = policy_len.max(1) as usize;
    let mut distributions = Vec::with_capacity(num_samples as usize);
    for idx in 0..num_samples {
        let mut best_position = positions.get(idx).int64_value(&[]);
        if best_position < 0 {
            best_position = 0;
        }
        if best_position >= policy_len {
            best_position = policy_len - 1;
        }
        let mut one_hot = vec![0f32; vocab];
        one_hot[best_position as usize] = 1.0;
        distributions.push(Tensor::from_slice(&one_hot));
    }
    Tensor::stack(&distributions, 0)
}

fn rebuild_policy(combined_positions: &Tensor, policy_tensor: &Tensor) -> Tensor {
    let policy_len = policy_tensor.size()[1].max(19);
    let fallback = positions_to_one_hot(combined_positions, policy_len);
    let new_rows = policy_tensor.size()[0];
    if new_rows > 0 {
        let start = fallback.size()[0] - new_rows;
        fallback.narrow(0, start, new_rows).copy_(policy_tensor);
    }
    fallback
}
