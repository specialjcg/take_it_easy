// src/services/game_service/async_move_handler.rs - Gestion asynchrone des mouvements
// Permet un feedback immédiat à l'UI pendant que MCTS calcule en arrière-plan

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use tonic::{Response, Status};

use crate::generated::takeiteasygame::v1::*;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::qvalue_net::QValueNet;
use crate::services::game_manager::{
    ensure_current_tile, player_move_from_json, process_player_move_with_mcts,
    process_player_move_with_hybrid_mcts, MoveResult, PlayerMove, TakeItEasyGameState,
};
use crate::services::session_manager::{
    get_store_from_manager, update_session_in_store, SessionManager,
};

use super::response_builders::{make_move_error_response, make_move_success_response};
use super::session_utils::get_session_by_code_or_id_from_store;

pub struct AsyncMoveRequest {
    pub session_id: String,
    pub player_id: String,
    pub move_data: String,
    pub timestamp: i64,
}

/// Version asynchrone qui retourne immédiatement une confirmation
/// et traite MCTS en arrière-plan (supporte Q-Net hybrid)
pub async fn make_move_async_logic(
    session_manager: &Arc<SessionManager>,
    policy_net: &Arc<Mutex<PolicyNet>>,
    value_net: &Arc<Mutex<ValueNet>>,
    qvalue_net: Option<Arc<Mutex<QValueNet>>>,
    _num_simulations: usize,  // Unused - simulations come from session config
    top_k: usize,
    request: AsyncMoveRequest,
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

    // ✅ NOUVEAU: Retourner immédiatement une confirmation de mouvement
    let immediate_response = create_immediate_move_confirmation(&player_move, &game_state);

    // ✅ Lancer MCTS en arrière-plan (hybrid si Q-Net disponible)
    // Utiliser le num_simulations de la session (configuré par le frontend)
    let session_simulations = session.num_simulations;
    let session_manager_clone = session_manager.clone();
    let policy_net_clone = policy_net.clone();
    let value_net_clone = value_net.clone();
    let qvalue_net_clone = qvalue_net.clone();
    let session_id_clone = request.session_id.clone();
    let game_mode = session.game_mode.clone();

    log::info!("🎯 MCTS avec {} simulations (mode: {})", session_simulations, game_mode);

    task::spawn(async move {
        process_mcts_in_background(
            session_manager_clone,
            policy_net_clone,
            value_net_clone,
            qvalue_net_clone,
            session_simulations,  // Utilise la config de session
            top_k,
            game_state,
            player_move,
            session_id_clone,
            game_mode,
        )
        .await;
    });

    // Retourner immédiatement la confirmation
    Ok(Response::new(immediate_response))
}

/// Crée une réponse immédiate de confirmation de mouvement
fn create_immediate_move_confirmation(
    player_move: &PlayerMove,
    game_state: &TakeItEasyGameState,
) -> MakeMoveResponse {
    // Créer une réponse de succès immédiate avec statut "processing"
    let mock_move_result = MoveResult {
        new_game_state: game_state.clone(),
        mcts_response: None,
        points_earned: 0, // Sera calculé par MCTS
        is_game_over: false,
        turn_completed: false, // Le tour n'est pas encore terminé
    };

    // Utiliser le constructeur existant mais avec un indicateur de traitement
    let mut response = make_move_success_response(mock_move_result, "standard");

    // Modifier pour indiquer que le traitement est en cours
    if let Some(make_move_response::Result::Success(ref mut success)) = response.result {
        // Marquer que MCTS calcule en arrière-plan
        success.mcts_response = format!(
            "{{\"status\":\"PROCESSING\",\"message\":\"Move accepted for player {} at position {}, MCTS calculating...\"}}",
            player_move.player_id, player_move.position
        );
    }

    response
}

/// Traite MCTS en arrière-plan et met à jour la session (hybrid si Q-Net disponible)
#[allow(clippy::too_many_arguments)]
async fn process_mcts_in_background(
    session_manager: Arc<SessionManager>,
    policy_net: Arc<Mutex<PolicyNet>>,
    value_net: Arc<Mutex<ValueNet>>,
    qvalue_net: Option<Arc<Mutex<QValueNet>>>,
    num_simulations: usize,
    top_k: usize,
    game_state: TakeItEasyGameState,
    player_move: PlayerMove,
    session_id: String,
    _game_mode: String,
) {
    let mcts_type = if qvalue_net.is_some() { "HYBRID" } else { "CNN" };
    log::info!(
        "🔄 Démarrage traitement MCTS {} en arrière-plan pour joueur {}",
        mcts_type, player_move.player_id
    );

    // Use hybrid MCTS if Q-Net is available, otherwise standard CNN MCTS
    let result = if let Some(qnet) = qvalue_net {
        process_player_move_with_hybrid_mcts(
            game_state,
            player_move,
            &policy_net,
            &value_net,
            &qnet,
            num_simulations,
            top_k,
        )
        .await
    } else {
        process_player_move_with_mcts(
            game_state,
            player_move,
            &policy_net,
            &value_net,
            num_simulations,
        )
        .await
    };

    match result {
        Ok(move_result) => {
            let final_state = move_result.new_game_state.clone();
            let store = get_store_from_manager(&session_manager);

            if let Some(mut session) =
                get_session_by_code_or_id_from_store(store, &session_id).await
            {
                // Sauvegarder l'état final
                session.board_state = serde_json::to_string(&final_state).unwrap_or_default();

                // ✅ METTRE À JOUR L'ÉTAT DE LA SESSION QUAND LE JEU EST TERMINÉ
                use crate::services::game_manager::is_game_finished;
                if is_game_finished(&final_state) {
                    session.state = 2; // SessionState::FINISHED
                    log::info!("🏁 Session {} marquée comme FINISHED", session_id);
                }

                // Synchroniser les scores
                for (player_id, score) in &final_state.scores {
                    if let Some(player) = session.players.get_mut(player_id) {
                        player.score = *score;
                        log::info!(
                            "🏆 Score mis à jour (async): {} = {} points",
                            player_id,
                            score
                        );
                    }
                }

                if let Err(e) = update_session_in_store(store, session).await {
                    log::error!("❌ Échec mise à jour session async: {}", e);
                } else {
                    log::info!(
                        "✅ Traitement MCTS terminé avec succès pour session {}",
                        session_id
                    );
                }
            }
        }
        Err(error_code) => {
            log::error!("❌ Échec traitement MCTS async: {}", error_code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::create_deck::create_deck;
    use crate::game::tile::Tile;
    use std::collections::HashMap;

    fn create_test_game_state() -> TakeItEasyGameState {
        TakeItEasyGameState {
            session_id: "test_session".to_string(),
            deck: create_deck(),
            player_plateaus: HashMap::new(),
            current_tile: Some(Tile(1, 2, 3)),
            current_turn: 1,
            total_turns: 19,
            game_status: crate::services::game_manager::GameStatus::InProgress,
            scores: HashMap::new(),
            waiting_for_players: vec!["player1".to_string()],
        }
    }

    #[tokio::test]
    async fn test_immediate_response_speed() {
        // Test que la réponse est immédiate (< 100ms)
        let start = std::time::Instant::now();

        // Créer un mouvement test
        let player_move = PlayerMove {
            player_id: "test_player".to_string(),
            position: 23, // Position valide sur le plateau
            tile: Tile(1, 2, 3),
            timestamp: 0,
        };

        let game_state = create_test_game_state();
        let response = create_immediate_move_confirmation(&player_move, &game_state);

        let duration = start.elapsed();

        // Vérifier que la réponse est immédiate
        assert!(
            duration.as_millis() < 100,
            "Response should be immediate (< 100ms), was {}ms",
            duration.as_millis()
        );

        // Vérifier qu'on a une réponse de succès
        assert!(response.result.is_some());
        if let Some(make_move_response::Result::Success(success)) = response.result {
            assert!(success.mcts_response.contains("PROCESSING"));
            assert!(success.mcts_response.contains("player test_player"));
        }
    }

    #[test]
    fn test_immediate_confirmation_content() {
        let player_move = PlayerMove {
            player_id: "player1".to_string(),
            position: 15,
            tile: Tile(5, 6, 7),
            timestamp: 12345,
        };

        let game_state = create_test_game_state();
        let response = create_immediate_move_confirmation(&player_move, &game_state);

        // Vérifier le contenu de la réponse
        assert!(response.result.is_some());
        if let Some(make_move_response::Result::Success(success)) = response.result {
            assert!(success.mcts_response.contains("PROCESSING"));
            assert!(success.mcts_response.contains("player1"));
            assert!(success.mcts_response.contains("position 15"));
            assert!(!success.is_game_over);
        }
    }
}
