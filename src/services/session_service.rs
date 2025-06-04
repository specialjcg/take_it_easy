// src/services/session_service.rs - Version avec debugging amélioré

use tonic::{Request, Response, Status};
use std::sync::Arc;
use crate::generated::takeiteasygame::v1::{
    GetSessionStateRequest,
    GetSessionStateResponse,
    SessionState as ProtoSessionState
};
// Import des types générés par tonic
use crate::generated::takeiteasygame::v1::*;
use crate::generated::takeiteasygame::v1::session_service_server::SessionService;

use crate::services::session_manager::{SessionManager, new_session_manager, get_store_from_manager, add_player_to_session, set_player_ready_in_session, session_to_game_state, transform_session_in_store, get_session_by_code_with_manager, update_session_with_manager, create_session_functional_with_manager, get_session_by_id_with_manager};

#[derive(Clone)]
pub struct SessionServiceImpl {
    session_manager: Arc<SessionManager>,
}

impl SessionServiceImpl {
    pub fn new() -> Self {
        Self {
            session_manager: Arc::new(new_session_manager()), // Fonction extraite !
        }
    }
    pub fn new_with_manager(session_manager: Arc<SessionManager>) -> Self {
        Self {
            session_manager,
        }
    }
    
}

// ============================================================================
// FONCTIONS PURES - CRÉATION DE RÉPONSES (inchangées)
// ============================================================================

fn create_success_response(
    session_code: String,
    session_id: String,
    player_id: String,
    player: Player
) -> CreateSessionResponse {
    CreateSessionResponse {
        result: Some(create_session_response::Result::Success(
            CreateSessionSuccess {
                session_code,
                session_id,
                player_id,
                player: Some(player),
            }
        )),
    }
}

fn create_error_response(code: String, message: String) -> CreateSessionResponse {
    CreateSessionResponse {
        result: Some(create_session_response::Result::Error(Error {
            code,
            message,
            details: std::collections::HashMap::new(),
        })),
    }
}

fn join_success_response(
    session_id: String,
    player_id: String,
    game_state: GameState
) -> JoinSessionResponse {
    JoinSessionResponse {
        result: Some(join_session_response::Result::Success(
            JoinSessionSuccess {
                session_id,
                player_id,
                game_state: Some(game_state),
            }
        )),
    }
}

fn join_error_response(code: String, message: String) -> JoinSessionResponse {
    JoinSessionResponse {
        result: Some(join_session_response::Result::Error(Error {
            code,
            message,
            details: std::collections::HashMap::new(),
        })),
    }
}

fn set_ready_success_response(game_started: bool) -> SetReadyResponse {
    SetReadyResponse {
        success: true,
        error: None,
        game_started,
    }
}

fn set_ready_error_response(code: String, message: String) -> SetReadyResponse {
    SetReadyResponse {
        success: false,
        error: Some(Error {
            code,
            message,
            details: std::collections::HashMap::new(),
        }),
        game_started: false,
    }
}

// ============================================================================
// FONCTIONS COMPOSABLES - LOGIQUE MÉTIER AVEC DEBUG AMÉLIORÉ
// ============================================================================

// session_service.rs - dans create_session_logic_with_manager
async fn create_session_logic_with_manager(
    manager: &SessionManager,
    player_name: String,
    max_players: i32,
    game_mode: String
) -> Result<Response<CreateSessionResponse>, Status> {
    match create_session_functional_with_manager(manager, max_players, game_mode).await {
        Ok(session_code) => {
            if let Some(mut session) = get_session_by_code_with_manager(manager, &session_code).await {
                // Ajouter le joueur humain
                match add_player_to_session(session.clone(), player_name.clone()) {
                    Ok((mut updated_session, player_id)) => {

                        // 🤖 AJOUTER AUTOMATIQUEMENT MCTS À CHAQUE SESSION
                        let mcts_player = Player {
                            id: "mcts_ai".to_string(),
                            name: "🤖 MCTS IA".to_string(),
                            score: 0,
                            is_ready: true,  // MCTS toujours prêt
                            is_connected: true,
                            joined_at: chrono::Utc::now().timestamp(),
                        };

                        updated_session.players.insert("mcts_ai".to_string(), mcts_player);
                        let player = updated_session.players.get(&player_id).cloned()
                            .ok_or_else(|| Status::internal("Player not found after creation"))?;

                        let session_id = updated_session.id.clone();

                        // Sauvegarder avec MCTS
                        update_session_with_manager(manager, updated_session).await
                            .map_err(|e| Status::internal(e))?;

                        let response = create_success_response(session_code, session_id, player_id, player);
                        Ok(Response::new(response))
                    },
                    Err(e) => {
                        log::error!("❌ Échec ajout joueur: {}", e);
                        let response = create_error_response(e, "Failed to add player to session".to_string());
                        Ok(Response::new(response))
                    }
                }
            } else {
                log::error!("❌ Session introuvable après création: {}", session_code);
                Err(Status::internal("Failed to retrieve created session"))
            }
        },
        Err(e) => {
            log::error!("❌ Échec création session: {}", e);
            let response = create_error_response(
                "CREATION_FAILED".to_string(),
                "Failed to create session".to_string()
            );
            Ok(Response::new(response))
        }
    }
}

// session_service.rs - dans join_session_logic
async fn join_session_logic(
    manager: &SessionManager,
    session_code: String,
    player_name: String
) -> Result<Response<JoinSessionResponse>, Status> {
    let session = match get_session_by_code_with_manager(manager, &session_code).await {
        Some(session) => {            session
        },
        None => {
            log::error!("❌ Session introuvable avec code: {}", session_code);
            return Ok(Response::new(join_error_response(
                "SESSION_NOT_FOUND".to_string(),
                format!("Session with code {} not found", session_code)
            )));
        }
    };

    // 🔧 NOUVEAU: Gestion spéciale pour les viewers
    if player_name.contains("Viewer") || player_name.contains("Observer") {
        // Créer un joueur viewer (read-only)
        let viewer_id = format!("viewer_{}", uuid::Uuid::new_v4().to_string()[0..8].to_string());
        let viewer_player = Player {
            id: viewer_id.clone(),
            name: player_name.clone(),
            score: 0,
            is_ready: true,  // Toujours prêt (n'affecte pas le jeu)
            is_connected: true,
            joined_at: chrono::Utc::now().timestamp(),
        };

        // 🔧 NE PAS ajouter le viewer à la session (juste retourner l'état)
        let session_id = session.id.clone();
        let game_state = session_to_game_state(&session);
        let response = join_success_response(session_id, viewer_id, game_state);
        return Ok(Response::new(response));
    }

    match add_player_to_session(session, player_name.clone()) {
        Ok((mut updated_session, player_id)) => {

            // 🤖 VÉRIFIER SI MCTS EST DÉJÀ PRÉSENT, SINON L'AJOUTER
            if !updated_session.players.contains_key("mcts_ai") {
                let mcts_player = Player {
                    id: "mcts_ai".to_string(),
                    name: "🤖 MCTS IA".to_string(),
                    score: 0,
                    is_ready: true,
                    is_connected: true,
                    joined_at: chrono::Utc::now().timestamp(),
                };

                updated_session.players.insert("mcts_ai".to_string(), mcts_player);            } else {            }

            let session_id = updated_session.id.clone();
            let game_state = session_to_game_state(&updated_session);
            update_session_with_manager(manager, updated_session).await
                .map_err(|e| Status::internal(e))?;

            let response = join_success_response(session_id, player_id, game_state);
            Ok(Response::new(response))
        },
        Err(e) => {
            log::error!("❌ Échec join session: {}", e);
            let response = join_error_response(e, "Failed to join session".to_string());
            Ok(Response::new(response))
        }
    }
}

// 🔧 FONCTION SET_READY AVEC DEBUG ULTRA-DÉTAILLÉ
async fn set_ready_logic(
    manager: &SessionManager,
    session_id: String,
    player_id: String,
    ready: bool
) -> Result<Response<SetReadyResponse>, Status> {
    // 🔍 Étape 1: Vérifier l'existence de la session AVANT transform
    match get_session_by_id_with_manager(manager, &session_id).await {
        Some(session) => {
            // Vérifier si le joueur existe
            if let Some(player) = session.players.get(&player_id) {            } else {
                log::error!("❌ Joueur {} introuvable dans session {}", player_id, session_id);
                return Ok(Response::new(set_ready_error_response(
                    "PLAYER_NOT_FOUND".to_string(),
                    format!("Player {} not found in session {}", player_id, session_id)
                )));
            }
        },
        None => {
            log::error!("❌ Session {} introuvable lors de SET_READY", session_id);

            // 🔍 Debug: Lister toutes les sessions existantes
            let store = get_store_from_manager(manager);
            let state = store.read().await;
            log::error!("🔍 Sessions existantes ({} total):", state.sessions.len());
            for (sid, session) in &state.sessions {
                log::error!("  - id={}, code={}, players={}", sid, session.code, session.players.len());
            }
            drop(state);

            return Ok(Response::new(set_ready_error_response(
                "SESSION_NOT_FOUND".to_string(),
                format!("Session {} not found", session_id)
            )));
        }
    }

    // 🔧 Étape 2: Continuer avec la logique normale
    let store = get_store_from_manager(manager);

    // Utilisation directe de transform_session_in_store pour récupérer game_started
    let result = transform_session_in_store(store, &session_id, |session| {        set_player_ready_in_session(session, &player_id, ready)
    }).await;

    match result {
        Ok(Some(game_started)) => {            Ok(Response::new(set_ready_success_response(game_started)))
        },
        Ok(None) => {
            log::error!("❌ Session {} disparue pendant transform", session_id);
            Ok(Response::new(set_ready_error_response(
                "SESSION_NOT_FOUND".to_string(),
                "Session not found during update".to_string()
            )))
        },
        Err(error_code) => {
            log::error!("❌ Erreur pendant SET_READY: {}", error_code);
            Ok(Response::new(set_ready_error_response(
                error_code,
                "Failed to set ready status".to_string()
            )))
        }
    }
}

// ============================================================================
// IMPLÉMENTATION GRPC - TRAIT GÉNÉRÉ PAR TONIC (inchangé)
// ============================================================================

#[tonic::async_trait]
impl SessionService for SessionServiceImpl {
    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<CreateSessionResponse>, Status> {
        let req = request.into_inner();
        create_session_logic_with_manager(
            &self.session_manager,
            req.player_name,
            req.max_players,
            req.game_mode
        ).await
    }

    async fn join_session(
        &self,
        request: Request<JoinSessionRequest>,
    ) -> Result<Response<JoinSessionResponse>, Status> {
        let req = request.into_inner();
        join_session_logic(
            &self.session_manager,
            req.session_code,
            req.player_name
        ).await
    }

    async fn set_ready(
        &self,
        request: Request<SetReadyRequest>,
    ) -> Result<Response<SetReadyResponse>, Status> {
        let req = request.into_inner();
        set_ready_logic(
            &self.session_manager,
            req.session_id,
            req.player_id,
            req.ready
        ).await
    }

    // 🔍 GET_SESSION_STATE AVEC DEBUG AMÉLIORÉ
    async fn get_session_state(
        &self,
        request: Request<GetSessionStateRequest>,
    ) -> Result<Response<GetSessionStateResponse>, Status> {
        let req = request.into_inner();
        // Utiliser votre fonction fonctionnelle get_session_by_id_with_manager
        match get_session_by_id_with_manager(&self.session_manager, &req.session_id).await {
            Some(session) => {
                // Convertir votre GameSession en GameState proto
                let proto_players: Vec<crate::generated::takeiteasygame::v1::Player> =
                    session.players.values().map(|p| {
                        crate::generated::takeiteasygame::v1::Player {
                            id: p.id.clone(),
                            name: p.name.clone(),
                            score: p.score,
                            is_ready: p.is_ready,
                            is_connected: p.is_connected,
                            joined_at: p.joined_at,
                        }
                    }).collect();

                let game_state = crate::generated::takeiteasygame::v1::GameState {
                    session_id: session.id.clone(),
                    players: proto_players,
                    current_player_id: session.current_player_id.clone().unwrap_or_default(),
                    state: match session.state {
                        0 => ProtoSessionState::Waiting as i32,
                        1 => ProtoSessionState::InProgress as i32,
                        2 => ProtoSessionState::Finished as i32,
                        _ => ProtoSessionState::Waiting as i32,
                    },
                    board_state: session.board_state.clone(),
                    turn_number: session.turn_number,
                };

                Ok(Response::new(GetSessionStateResponse {
                    game_state: Some(game_state),
                    error: None,
                }))
            }
            None => {
                log::error!("❌ GET_SESSION_STATE: Session {} introuvable", req.session_id);

                // 🔍 Debug: Lister toutes les sessions existantes
                let store = get_store_from_manager(&self.session_manager);
                let state = store.read().await;
                log::error!("🔍 Sessions existantes ({} total):", state.sessions.len());
                for (sid, session) in &state.sessions {
                    log::error!("  - id={}, code={}", sid, session.code);
                }
                drop(state);

                // Session non trouvée
                Ok(Response::new(GetSessionStateResponse {
                    game_state: None,
                    error: Some(crate::generated::takeiteasygame::v1::Error {
                        code: "SESSION_NOT_FOUND".to_string(),
                        message: format!("Session {} not found", req.session_id),
                        details: std::collections::HashMap::new(),
                    }),
                }))
            }
        }
    }
}