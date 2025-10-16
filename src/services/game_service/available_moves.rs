// src/services/game_service/available_moves.rs - Gestion des mouvements disponibles

use std::sync::Arc;
use tonic::{Response, Status};

use crate::generated::takeiteasygame::v1::*;
use crate::services::game_manager::{
    get_available_positions, is_game_finished, TakeItEasyGameState,
};
use crate::services::session_manager::{
    get_session_by_id_from_store, get_store_from_manager, SessionManager,
};

use super::response_builders::{available_moves_error_response, available_moves_success_response};

// ============================================================================
// LOGIQUE DES MOUVEMENTS DISPONIBLES
// ============================================================================

pub async fn get_available_moves_logic(
    session_manager: &Arc<SessionManager>,
    session_id: String,
    player_id: String,
) -> Result<Response<GetAvailableMovesResponse>, Status> {
    let store = get_store_from_manager(session_manager);

    // Récupérer la session
    let session = match get_session_by_id_from_store(store, &session_id).await {
        Some(session) => session,
        None => {
            let response = available_moves_error_response(
                "SESSION_NOT_FOUND".to_string(),
                "Session not found".to_string(),
            );
            return Ok(Response::new(response));
        }
    };

    // Récupérer l'état de jeu
    let game_state: TakeItEasyGameState =
        if session.board_state.is_empty() || session.board_state == "{}" {
            let response = available_moves_error_response(
                "GAME_NOT_STARTED".to_string(),
                "Game has not started yet".to_string(),
            );
            return Ok(Response::new(response));
        } else {
            serde_json::from_str(&session.board_state)
                .map_err(|e| Status::internal(format!("Failed to parse game state: {}", e)))?
        };

    // Vérifier si le jeu est terminé
    if is_game_finished(&game_state) {
        let response = available_moves_error_response(
            "GAME_FINISHED".to_string(),
            "Game is already finished".to_string(),
        );
        return Ok(Response::new(response));
    }

    // Obtenir les positions disponibles
    let available_positions = get_available_positions(&game_state, &player_id);
    let current_tile = game_state.current_tile;

    let response = available_moves_success_response(available_positions, current_tile);
    Ok(Response::new(response))
}
