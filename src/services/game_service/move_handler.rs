// src/services/game_service/move_handler.rs - Gestion des mouvements de joueurs

use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Response, Status};

use crate::generated::takeiteasygame::v1::*;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::services::game_manager::{
    ensure_current_tile, player_move_from_json, process_player_move_with_mcts, TakeItEasyGameState,
};
use crate::services::session_manager::{
    get_store_from_manager, update_session_in_store, SessionManager,
};

use super::response_builders::{make_move_error_response, make_move_success_response};
use super::session_utils::get_session_by_code_or_id_from_store;

// ============================================================================
// LOGIQUE DE GESTION DES MOUVEMENTS
// ============================================================================

pub struct MoveRequest {
    pub session_id: String,
    pub player_id: String,
    pub move_data: String,
    pub timestamp: i64,
}

pub async fn make_move_logic(
    session_manager: &Arc<SessionManager>,
    policy_net: &Arc<Mutex<PolicyNet>>,
    value_net: &Arc<Mutex<ValueNet>>,
    num_simulations: usize,
    request: MoveRequest,
) -> Result<Response<MakeMoveResponse>, Status> {
    let store = get_store_from_manager(session_manager);

    let session = match get_session_by_code_or_id_from_store(store, &request.session_id).await {
        Some(session) => session,
        None => {
            let response = make_move_error_response(
                "SESSION_NOT_FOUND".to_string(),
                "Session not found".to_string(),
            );
            return Ok(Response::new(response));
        }
    };

    // Récupérer l'état de jeu
    let game_state: TakeItEasyGameState =
        if session.board_state.is_empty() || session.board_state == "{}" {
            let response = make_move_error_response(
                "GAME_NOT_STARTED".to_string(),
                "No game state found. Please start a turn first.".to_string(),
            );
            return Ok(Response::new(response));
        } else {
            serde_json::from_str(&session.board_state)
                .map_err(|e| Status::internal(format!("Failed to parse game state: {}", e)))?
        };

    // Vérification: S'assurer qu'une tuile courante existe
    let game_state = match ensure_current_tile(game_state) {
        Ok(state) => state,
        Err(e) => {
            log::error!("❌ Échec garantie tuile: {}", e);
            return Ok(Response::new(make_move_error_response(
                "NO_CURRENT_TILE".to_string(),
                format!("No current tile available: {}", e),
            )));
        }
    };

    // Parser le mouvement du joueur
    let player_move = match player_move_from_json(&request.move_data, &request.player_id) {
        Ok(mv) => {
            let mut mv = mv;
            mv.timestamp = request.timestamp;
            if let Some(current_tile) = game_state.current_tile {
                mv.tile = current_tile;
            }
            mv
        }
        Err(e) => {
            let response = make_move_error_response(
                "INVALID_MOVE_FORMAT".to_string(),
                format!("Failed to parse move: {}", e),
            );
            return Ok(Response::new(response));
        }
    };

    // Traitement du mouvement avec MCTS
    match process_player_move_with_mcts(
        game_state,
        player_move,
        policy_net,
        value_net,
        num_simulations,
    )
    .await
    {
        Ok(move_result) => {
            let final_state = move_result.new_game_state.clone();
            let game_mode = session.game_mode.clone();

            // Sauvegarder l'état final
            let mut updated_session = session;
            updated_session.board_state = serde_json::to_string(&final_state).unwrap_or_default();

            // ✅ CRITICAL: Synchroniser les scores entre TakeItEasyGameState et Session.players
            for (player_id, score) in &final_state.scores {
                if let Some(player) = updated_session.players.get_mut(player_id) {
                    player.score = *score;
                    log::info!("🏆 Score mis à jour: {} = {} points", player_id, score);
                }
            }

            update_session_in_store(store, updated_session)
                .await
                .map_err(Status::internal)?;

            let response = make_move_success_response(move_result, &game_mode);
            Ok(Response::new(response))
        }
        Err(error_code) => {
            let response =
                make_move_error_response(error_code, "Failed to process move".to_string());
            Ok(Response::new(response))
        }
    }
}
