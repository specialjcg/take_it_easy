// src/services/game_service/mcts_integration.rs - Intégration MCTS découplée

use crate::game::get_legal_moves::get_legal_moves;
use crate::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::services::game_manager::{apply_player_move, MctsMove, PlayerMove, TakeItEasyGameState};
use tokio::sync::Mutex;

// ============================================================================
// INTÉGRATION MCTS DÉCOUPLÉE
// ============================================================================

pub async fn process_mcts_move_only(
    game_state: TakeItEasyGameState,
    policy_net: &Mutex<PolicyNet>,
    value_net: &Mutex<ValueNet>,
    num_simulations: usize,
) -> Result<(TakeItEasyGameState, MctsMove), String> {
    // ✅ VÉRIFICATION: MCTS doit être en attente
    if !game_state
        .waiting_for_players
        .contains(&"mcts_ai".to_string())
    {
        return Err("MCTS_NOT_WAITING".to_string());
    }

    let current_tile = game_state.current_tile.ok_or("NO_CURRENT_TILE")?;

    // Récupérer le plateau MCTS
    let mcts_plateau = game_state
        .player_plateaus
        .get("mcts_ai")
        .ok_or("MCTS_PLAYER_NOT_FOUND")?
        .clone();

    // ✅ VÉRIFICATION: Mouvements légaux
    let legal_moves = get_legal_moves(mcts_plateau.clone());
    if legal_moves.is_empty() {
        return Err("NO_LEGAL_MOVES_FOR_MCTS".to_string());
    }

    let mut deck_clone = game_state.deck.clone();

    // Verrouiller les réseaux
    let policy_locked = policy_net.lock().await;
    let value_locked = value_net.lock().await;

    // ✅ EXÉCUTION MCTS
    let mut mcts_plateau_mut = mcts_plateau.clone();
    let mcts_result = mcts_find_best_position_for_tile_with_nn(
        &mut mcts_plateau_mut,
        &mut deck_clone,
        current_tile,
        &policy_locked,
        &value_locked,
        num_simulations,
        game_state.current_turn,
        game_state.total_turns,
    );

    // ✅ VALIDATION: Position choisie doit être légale
    if !legal_moves.contains(&mcts_result.best_position) {
        return Err("MCTS_ILLEGAL_MOVE".to_string());
    }

    // Créer le mouvement MCTS
    let mcts_player_move = PlayerMove {
        player_id: "mcts_ai".to_string(),
        position: mcts_result.best_position,
        tile: current_tile,
        timestamp: chrono::Utc::now().timestamp(),
    };

    // Appliquer le mouvement MCTS
    let updated_state = apply_player_move(game_state, mcts_player_move)?;

    let mcts_move = MctsMove {
        position: mcts_result.best_position,
        tile: current_tile,
        evaluation_score: mcts_result.subscore as f32,
        search_depth: num_simulations,
        variations_considered: num_simulations,
    };

    Ok((updated_state, mcts_move))
}
