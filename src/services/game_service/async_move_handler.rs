// src/services/game_service/async_move_handler.rs - Gestion asynchrone des mouvements
// Permet un feedback immédiat à l'UI pendant que MCTS calcule en arrière-plan

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;
use tonic::{Response, Status};

use crate::generated::takeiteasygame::v1::*;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::qvalue_net::QValueNet;
use crate::services::game_manager::{
    compute_ai_move_background, ensure_current_tile, is_game_finished, player_move_from_json,
    process_ai_turn_direct, process_player_move_immediate,
    process_player_move_with_direct_inference, process_player_move_with_hybrid_mcts,
    process_player_move_with_mcts, MoveResult, PlayerMove, TakeItEasyGameState,
};
use crate::services::session_manager::{
    get_store_from_manager, update_session_in_store, SessionManager,
};

use super::response_builders::{make_move_error_response, make_move_success_response};
use super::session_utils::get_session_by_code_or_id_from_store;

// Global pending AI tasks: session_id → JoinHandle
static PENDING_AI_TASKS: OnceLock<TokioMutex<HashMap<String, JoinHandle<()>>>> = OnceLock::new();

fn pending_ai_tasks() -> &'static TokioMutex<HashMap<String, JoinHandle<()>>> {
    PENDING_AI_TASKS.get_or_init(|| TokioMutex::new(HashMap::new()))
}

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
    _num_simulations: usize, // Unused - simulations come from session config
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

    // Utiliser le num_simulations de la session (configuré par le frontend)
    let session_simulations = session.num_simulations;
    let game_mode = session.game_mode.clone();

    log::info!(
        "🎯 Graph Transformer avec {} simulations max (mode: {})",
        session_simulations,
        game_mode
    );

    // ✅ CORRECTION: Attendre la fin du traitement MCTS avant de retourner
    let response = process_mcts_and_respond(
        session_manager.clone(),
        policy_net.clone(),
        value_net.clone(),
        qvalue_net.clone(),
        session_simulations,
        top_k,
        game_state,
        player_move,
        request.session_id.clone(),
        game_mode,
    )
    .await;

    Ok(Response::new(response))
}

/// Crée une réponse immédiate de confirmation de mouvement
/// NOTE: Cette fonction n'est plus utilisée, gardée pour les tests
#[allow(dead_code)]
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

/// Process player move with async AI: returns immediately, AI computes in background
/// Falls back to synchronous MCTS only when Q-Net hybrid is explicitly enabled
#[allow(clippy::too_many_arguments)]
async fn process_mcts_and_respond(
    session_manager: Arc<SessionManager>,
    policy_net: Arc<Mutex<PolicyNet>>,
    value_net: Arc<Mutex<ValueNet>>,
    qvalue_net: Option<Arc<Mutex<QValueNet>>>,
    num_simulations: usize,
    top_k: usize,
    game_state: TakeItEasyGameState,
    player_move: PlayerMove,
    session_id: String,
    game_mode: String,
) -> MakeMoveResponse {
    // 1. Await any pending background AI task from the previous turn
    let had_pending_task;
    {
        let mut tasks = pending_ai_tasks().lock().await;
        if let Some(handle) = tasks.remove(&session_id) {
            log::info!("Awaiting pending AI task for session {}", session_id);
            let _ = handle.await;
            had_pending_task = true;
        } else {
            had_pending_task = false;
        }
    }

    // Hybrid MCTS mode: fall back to synchronous processing (legacy path)
    if qvalue_net.is_some() {
        log::info!("🎯 Traitement HYBRID MCTS pour joueur {}", player_move.player_id);
        return process_mcts_and_respond_sync(
            session_manager, policy_net, value_net, qvalue_net,
            num_simulations, top_k, game_state, player_move, session_id, game_mode,
        ).await;
    }

    // 2. Re-fetch game state if background AI updated it
    let game_state = if had_pending_task {
        let store = get_store_from_manager(&session_manager);
        match get_session_by_code_or_id_from_store(store, &session_id).await {
            Some(session) => {
                serde_json::from_str(&session.board_state).unwrap_or(game_state)
            }
            None => game_state,
        }
    } else {
        game_state
    };

    // 3. Process human move immediately (no AI wait)
    log::info!(
        "🎯 Traitement async GT pour joueur {} (AI en background)",
        player_move.player_id
    );

    let result = process_player_move_immediate(game_state, player_move.clone()).await;

    match result {
        Ok((mut move_result, ai_context)) => {
            let game_over = is_game_finished(&move_result.new_game_state);

            // Handle AI context: sync on game over, async otherwise
            if let Some(ctx) = ai_context {
                if game_over {
                    // Game over: compute AI's last move synchronously for complete final screen
                    log::info!("Game over: computing AI last move synchronously");
                    match process_ai_turn_direct(ctx.game_state, &policy_net).await {
                        Ok((updated_ai_state, ai_move)) => {
                            if let Some(ai_plateau) = updated_ai_state.player_plateaus.get("mcts_ai") {
                                move_result.new_game_state.player_plateaus
                                    .insert("mcts_ai".to_string(), ai_plateau.clone());
                                let ai_score = crate::scoring::scoring::result(ai_plateau);
                                move_result.new_game_state.scores
                                    .insert("mcts_ai".to_string(), ai_score);
                            }
                            move_result.mcts_response = Some(ai_move);
                        }
                        Err(e) => log::error!("Failed to compute AI last move: {}", e),
                    }
                } else {
                    // Mid-game: spawn background AI task
                    let sm = session_manager.clone();
                    let pn = policy_net.clone();
                    let sid = session_id.clone();
                    let handle = tokio::spawn(async move {
                        compute_ai_move_background(ctx, sm, pn).await;
                    });
                    pending_ai_tasks().lock().await.insert(sid, handle);
                }
            }

            let final_state = move_result.new_game_state.clone();
            let store = get_store_from_manager(&session_manager);

            if let Some(mut session) =
                get_session_by_code_or_id_from_store(store, &session_id).await
            {
                session.board_state = serde_json::to_string(&final_state).unwrap_or_default();

                if game_over {
                    session.state = 2;
                    log::info!("🏁 Session {} marquée comme FINISHED", session_id);

                    if let Some(recorder) = crate::recording::game_recorder::get_recorder() {
                        if let Err(e) = recorder.finalize_game(
                            &session_id,
                            final_state.scores.clone(),
                        ) {
                            log::error!("Failed to finalize game recording: {}", e);
                        }
                    }
                }

                // Sync all player scores
                for (player_id, score) in &final_state.scores {
                    if let Some(player) = session.players.get_mut(player_id) {
                        player.score = *score;
                    }
                }

                if let Err(e) = update_session_in_store(store, session).await {
                    log::error!("Failed to update session: {}", e);
                }
            }

            log::info!("Réponse pour session {} (game_over={})", session_id, game_over);
            make_move_success_response(move_result, &game_mode)
        }
        Err(error_code) => {
            log::error!("Échec traitement move: {}", error_code);
            make_move_error_response(
                error_code.clone(),
                format!("Move processing failed: {}", error_code),
            )
        }
    }
}

/// Synchronous fallback for hybrid MCTS mode (legacy)
#[allow(clippy::too_many_arguments)]
async fn process_mcts_and_respond_sync(
    session_manager: Arc<SessionManager>,
    policy_net: Arc<Mutex<PolicyNet>>,
    value_net: Arc<Mutex<ValueNet>>,
    qvalue_net: Option<Arc<Mutex<QValueNet>>>,
    num_simulations: usize,
    top_k: usize,
    game_state: TakeItEasyGameState,
    player_move: PlayerMove,
    session_id: String,
    game_mode: String,
) -> MakeMoveResponse {
    let result = if let Some(ref qnet) = qvalue_net {
        process_player_move_with_hybrid_mcts(
            game_state, player_move.clone(), &policy_net, &value_net, qnet,
            num_simulations, top_k,
        ).await
    } else {
        process_player_move_with_direct_inference(
            game_state, player_move.clone(), &policy_net,
        ).await
    };

    match result {
        Ok(move_result) => {
            let final_state = move_result.new_game_state.clone();
            let store = get_store_from_manager(&session_manager);

            if let Some(mut session) =
                get_session_by_code_or_id_from_store(store, &session_id).await
            {
                session.board_state = serde_json::to_string(&final_state).unwrap_or_default();

                if is_game_finished(&final_state) {
                    session.state = 2;
                    log::info!("🏁 Session {} marquée comme FINISHED", session_id);

                    if let Some(recorder) = crate::recording::game_recorder::get_recorder() {
                        if let Err(e) = recorder.finalize_game(
                            &session_id,
                            final_state.scores.clone(),
                        ) {
                            log::error!("Failed to finalize game recording: {}", e);
                        }
                    }
                }

                for (player_id, score) in &final_state.scores {
                    if let Some(player) = session.players.get_mut(player_id) {
                        player.score = *score;
                    }
                }

                if let Err(e) = update_session_in_store(store, session).await {
                    log::error!("Failed to update session: {}", e);
                }
            }

            make_move_success_response(move_result, &game_mode)
        }
        Err(error_code) => {
            log::error!("Échec traitement AI: {}", error_code);
            make_move_error_response(
                error_code.clone(),
                format!("AI processing failed: {}", error_code),
            )
        }
    }
}

/// Traite MCTS en arrière-plan et met à jour la session (hybrid si Q-Net disponible)
/// NOTE: Cette fonction n'est plus utilisée, gardée pour référence
#[allow(dead_code)]
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
    let mcts_type = if qvalue_net.is_some() {
        "HYBRID"
    } else {
        "CNN"
    };
    log::info!(
        "🔄 Démarrage traitement MCTS {} en arrière-plan pour joueur {}",
        mcts_type,
        player_move.player_id
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
