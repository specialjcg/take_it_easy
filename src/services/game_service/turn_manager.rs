// src/services/game_service/turn_manager.rs - Gestion des tours et démarrage

use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Response, Status};

use crate::generated::takeiteasygame::v1::*;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::qvalue_net::QValueNet;
use crate::services::game_manager::{
    create_take_it_easy_game, start_new_turn, TakeItEasyGameState,
};
use crate::services::session_manager::{
    get_store_from_manager, update_session_in_store, SessionManager,
};
use crate::utils::image::generate_tile_image_names;

use super::response_builders::{start_turn_error_response, start_turn_success_response};
use super::session_utils::get_session_by_code_or_id_from_store;

// ============================================================================
// LOGIQUE DE GESTION DES TOURS
// ============================================================================

/// Start a new turn - MCTS plays reactively after human move, not here
#[allow(clippy::too_many_arguments)]
pub async fn start_turn_logic(
    session_manager: &Arc<SessionManager>,
    _policy_net: &Arc<Mutex<PolicyNet>>,
    _value_net: &Arc<Mutex<ValueNet>>,
    _qvalue_net: Option<Arc<Mutex<QValueNet>>>,
    _num_simulations: usize,
    _top_k: usize,
    session_id: String,
) -> Result<Response<StartTurnResponse>, Status> {
    let store = get_store_from_manager(session_manager);
    let session = match get_session_by_code_or_id_from_store(store, &session_id).await {
        Some(session) => session,
        None => {
            return Ok(Response::new(start_turn_error_response(
                "Session not found".to_string(),
            )));
        }
    };

    // Récupérer ou créer l'état de jeu
    let game_state: TakeItEasyGameState =
        if session.board_state.is_empty() || session.board_state == "{}" {
            // Première fois - créer le jeu
            let player_ids: Vec<String> = session.players.keys().cloned().collect();
            create_take_it_easy_game(session_id.clone(), player_ids)
        } else {
            // Désérialiser l'état existant
            match serde_json::from_str::<TakeItEasyGameState>(&session.board_state) {
                Ok(mut state) => {
                    state.session_id = session_id.clone();
                    state
                }
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
                return Ok(Response::new(start_turn_error_response(format!(
                    "Failed to start turn: {}",
                    e
                ))));
            }
        }
    };

    // 🚀 SOLUTION UI RÉACTIVE: NE PAS faire jouer MCTS automatiquement dans start_turn
    // MCTS jouera seulement après que le joueur humain ait fait son mouvement
    // Cela permet au joueur de cliquer immédiatement sans attendre 30s
    let final_state = new_state;

    // MCTS est gardé dans waiting_for_players mais ne joue pas automatiquement ici
    // Il jouera via le système async après le clic du joueur humain

    // Extraire les informations de la tuile
    let announced_tile = final_state.current_tile.unwrap();
    let announced_tile_str = format!(
        "{}-{}-{}",
        announced_tile.0, announced_tile.1, announced_tile.2
    );
    let tile_image = generate_tile_image_names(&[announced_tile])[0].clone();

    let turn_number = final_state.current_turn as i32;
    let waiting_for_players = final_state.waiting_for_players.clone();
    let game_state_json = serde_json::to_string(&final_state).unwrap_or_default();

    // 🚀 SOLUTION RÉACTIVITÉ: Enrichir immédiatement avec available_positions
    // Cela évite d'attendre le polling pour avoir les positions disponibles
    let enhanced_game_state_json =
        crate::services::game_service::state_provider::enhance_game_state_with_images(
            &game_state_json,
        );

    // Sauvegarder l'état mis à jour ET enrichi
    let mut updated_session = session;
    updated_session.board_state = enhanced_game_state_json.clone();

    if let Err(e) = update_session_in_store(store, updated_session).await {
        return Ok(Response::new(start_turn_error_response(format!(
            "Failed to update session: {}",
            e
        ))));
    }

    let response = start_turn_success_response(
        announced_tile_str,
        tile_image,
        turn_number,
        waiting_for_players,
        enhanced_game_state_json,
    );
    Ok(Response::new(response))
}
