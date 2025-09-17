// src/services/game_service/turn_manager.rs - Gestion des tours et démarrage

use tonic::{Response, Status};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::generated::takeiteasygame::v1::*;
use crate::services::game_manager::{
    TakeItEasyGameState, create_take_it_easy_game, start_new_turn, check_turn_completion
};
use crate::services::session_manager::{
    get_store_from_manager, SessionManager, update_session_in_store
};
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::utils::image::generate_tile_image_names;

use super::response_builders::{start_turn_success_response, start_turn_error_response};
use super::session_utils::get_session_by_code_or_id_from_store;
use super::mcts_integration::process_mcts_move_only;

// ============================================================================
// LOGIQUE DE GESTION DES TOURS
// ============================================================================

pub async fn start_turn_logic(
    session_manager: &Arc<SessionManager>,
    policy_net: &Arc<Mutex<PolicyNet>>,
    value_net: &Arc<Mutex<ValueNet>>,
    num_simulations: usize,
    session_id: String
) -> Result<Response<StartTurnResponse>, Status> {
    let store = get_store_from_manager(session_manager);
    let session = match get_session_by_code_or_id_from_store(store, &session_id).await {
        Some(session) => session,
        None => {
            return Ok(Response::new(start_turn_error_response("Session not found".to_string())));
        }
    };

    // Récupérer ou créer l'état de jeu
    let game_state: TakeItEasyGameState = if session.board_state.is_empty() || session.board_state == "{}" {
        // Première fois - créer le jeu
        let player_ids: Vec<String> = session.players.keys().cloned().collect();
        create_take_it_easy_game(session_id.clone(), player_ids)
    } else {
        // Désérialiser l'état existant
        match serde_json::from_str::<TakeItEasyGameState>(&session.board_state) {
            Ok(mut state) => {
                state.session_id = session_id.clone();
                state
            },
            Err(_e) => {
                let player_ids: Vec<String> = session.players.keys().cloned().collect();
                create_take_it_easy_game(session_id.clone(), player_ids)
            }
        }
    };

    // Vérifier si une tuile existe déjà pour ce tour
    let new_state = if game_state.current_tile.is_some() {
        // ✅ Une tuile existe déjà, utiliser l'état actuel
        game_state
    } else {
        match start_new_turn(game_state) {
            Ok(new_state) => new_state,
            Err(e) => {
                return Ok(Response::new(start_turn_error_response(format!("Failed to start turn: {}", e))));
            }
        }
    };

    // ✅ NOUVEAU: FAIRE JOUER MCTS AUTOMATIQUEMENT DÈS QU'UNE TUILE EST DISPONIBLE
    let final_state = if new_state.waiting_for_players.contains(&"mcts_ai".to_string()) {
        // Utiliser la fonction process_mcts_move_only
        match process_mcts_move_only(
            new_state.clone(),
            policy_net,
            value_net,
            num_simulations
        ).await {
            Ok((updated_state, _mcts_move)) => {
                let update_state_clone = updated_state.clone();
                // Vérifier si le tour est terminé après que MCTS ait joué
                match check_turn_completion(update_state_clone) {
                    Ok(completed_state) => completed_state,
                    Err(_e) => updated_state.clone()
                }
            },
            Err(_e) => {
                // Retirer MCTS de la liste pour ne pas bloquer le jeu
                let mut fallback_state = new_state.clone();
                fallback_state.waiting_for_players.retain(|id| id != "mcts_ai");
                fallback_state
            }
        }
    } else {
        new_state
    };

    // Extraire les informations de la tuile
    let announced_tile = final_state.current_tile.unwrap();
    let announced_tile_str = format!("{}-{}-{}", announced_tile.0, announced_tile.1, announced_tile.2);
    let tile_image = generate_tile_image_names(&[announced_tile])[0].clone();

    let turn_number = final_state.current_turn as i32;
    let waiting_for_players = final_state.waiting_for_players.clone();
    let game_state_json = serde_json::to_string(&final_state).unwrap_or_default();

    // Sauvegarder l'état mis à jour (avec le mouvement MCTS si applicable)
    let mut updated_session = session;
    updated_session.board_state = game_state_json.clone();

    if let Err(e) = update_session_in_store(store, updated_session).await {
        return Ok(Response::new(start_turn_error_response(format!("Failed to update session: {}", e))));
    }

    let response = start_turn_success_response(
        announced_tile_str,
        tile_image,
        turn_number,
        waiting_for_players,
        game_state_json
    );
    Ok(Response::new(response))
}