// src/services/session_service.rs - Version avec debugging amélioré

use crate::generated::takeiteasygame::v1::{
    GetSessionStateRequest, GetSessionStateResponse, SessionState as ProtoSessionState,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};
// Import des types générés par tonic
use crate::generated::takeiteasygame::v1::create_session_response;
use crate::generated::takeiteasygame::v1::join_session_response;
use crate::generated::takeiteasygame::v1::session_service_server::SessionService;
use crate::generated::takeiteasygame::v1::*;

use crate::services::session_manager::{
    add_player_to_session, all_players_ready, create_session_functional_with_manager,
    get_session_by_code_with_manager, get_session_by_id_with_manager, get_store_from_manager,
    session_to_game_state, set_player_ready_in_session_with_min, start_game,
    transform_session_in_store, update_session_with_manager, SessionManager,
};

#[derive(Clone)]
pub struct SessionServiceImpl {
    session_manager: Arc<SessionManager>,
    single_player_mode: bool,
}

impl SessionServiceImpl {
    pub fn new_with_manager_and_mode(
        session_manager: Arc<SessionManager>,
        single_player: bool,
    ) -> Self {
        Self {
            session_manager,
            single_player_mode: single_player,
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
    player: Player,
) -> CreateSessionResponse {
    CreateSessionResponse {
        result: Some(create_session_response::Result::Success(
            CreateSessionSuccess {
                session_code,
                session_id,
                player_id,
                player: Some(player),
            },
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
    game_state: GameState,
) -> JoinSessionResponse {
    JoinSessionResponse {
        result: Some(join_session_response::Result::Success(JoinSessionSuccess {
            session_id,
            player_id,
            game_state: Some(game_state),
        })),
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
    service: &SessionServiceImpl,
    player_name: String,
    max_players: i32,
    game_mode: String,
) -> Result<Response<CreateSessionResponse>, Status> {
    let manager = &service.session_manager;
    match create_session_functional_with_manager(manager, max_players, game_mode).await {
        Ok(session_code) => {
            if let Some(session) = get_session_by_code_with_manager(manager, &session_code).await {
                // Ajouter le joueur humain
                match add_player_to_session(session.clone(), player_name.clone()) {
                    Ok((mut updated_session, player_id)) => {
                        // 🤖 AJOUTER MCTS AUTOMATIQUEMENT POUR LES MODES SINGLE-PLAYER ET MULTIPLAYER
                        if updated_session.game_mode.starts_with("single-player")
                            || updated_session.game_mode == "training"
                            || updated_session.game_mode == "multiplayer"
                        {
                            // ✅ En multiplayer, MCTS attend que d'autres joueurs rejoignent
                            let mcts_is_ready = updated_session.game_mode != "multiplayer";

                            let mcts_player = Player {
                                id: "mcts_ai".to_string(),
                                name: "🤖 MCTS IA".to_string(),
                                score: 0,
                                is_ready: mcts_is_ready, // Prêt en solo, pas prêt en multi
                                is_connected: true,
                                joined_at: chrono::Utc::now().timestamp(),
                            };

                            updated_session
                                .players
                                .insert("mcts_ai".to_string(), mcts_player);
                            log::info!(
                                "🤖 MCTS automatiquement ajouté à la session {} (mode: {})",
                                updated_session.code,
                                updated_session.game_mode
                            );

                            // 🎮 EN MODE SOLO: Mettre automatiquement le joueur humain prêt aussi
                            if let Some(human_player) = updated_session.players.get_mut(&player_id)
                            {
                                human_player.is_ready = true;
                                log::info!(
                                    "🎮 Joueur humain {} automatiquement mis prêt en mode solo",
                                    player_id
                                );
                            }

                            // Vérifier si le jeu peut démarrer automatiquement
                            if all_players_ready(&updated_session)
                                && updated_session.players.len() >= 2
                            {
                                updated_session = start_game(updated_session);
                                log::info!("🚀 Jeu démarré automatiquement en mode solo !");
                            }
                        }
                        let player = updated_session
                            .players
                            .get(&player_id)
                            .cloned()
                            .ok_or_else(|| Status::internal("Player not found after creation"))?;

                        let session_id = updated_session.id.clone();

                        // Sauvegarder avec MCTS
                        update_session_with_manager(manager, updated_session)
                            .await
                            .map_err(Status::internal)?;

                        let response =
                            create_success_response(session_code, session_id, player_id, player);
                        Ok(Response::new(response))
                    }
                    Err(e) => {
                        log::error!("❌ Échec ajout joueur: {}", e);
                        let response =
                            create_error_response(e, "Failed to add player to session".to_string());
                        Ok(Response::new(response))
                    }
                }
            } else {
                log::error!("❌ Session introuvable après création: {}", session_code);
                Err(Status::internal("Failed to retrieve created session"))
            }
        }
        Err(e) => {
            log::error!("❌ Échec création session: {}", e);
            let response = create_error_response(
                "CREATION_FAILED".to_string(),
                "Failed to create session".to_string(),
            );
            Ok(Response::new(response))
        }
    }
}

// session_service.rs - dans join_session_logic
async fn join_session_logic(
    service: &SessionServiceImpl,
    session_code: String,
    player_name: String,
) -> Result<Response<JoinSessionResponse>, Status> {
    let manager = &service.session_manager;

    // 🎮 GESTION SPÉCIALE DU CODE 'AUTO' EN MODE SINGLE-PLAYER
    let session = if session_code == "AUTO" && service.single_player_mode {
        // Utiliser la première session disponible
        let store = get_store_from_manager(manager);
        let state = store.read().await;

        if let Some((_session_id, session)) = state.sessions.iter().next() {
            let session_clone = session.clone();
            let session_code_found = session.code.clone();
            drop(state);
            log::info!(
                "🔄 AUTO: connexion à la session single-player {}",
                session_code_found
            );
            session_clone
        } else {
            drop(state);
            log::error!("❌ Aucune session disponible pour AUTO");
            return Ok(Response::new(join_error_response(
                "NO_SESSION_AVAILABLE".to_string(),
                "No session available for auto-connection".to_string(),
            )));
        }
    } else {
        // Logique normale par code
        match get_session_by_code_with_manager(manager, &session_code).await {
            Some(session) => session,
            None => {
                log::error!("❌ Session introuvable avec code: {}", session_code);
                return Ok(Response::new(join_error_response(
                    "SESSION_NOT_FOUND".to_string(),
                    format!("Session with code {} not found", session_code),
                )));
            }
        }
    };

    // 🔧 GESTION SPÉCIALE DES VIEWERS - mode lecture seule
    if player_name.contains("Viewer") || player_name.contains("Observer") {
        log::info!("👁️ Viewer {} rejoint session {}", player_name, session_code);

        // ✅ Permettre les viewers pour TOUS les modes (solo ET multiplayer)
        if service.single_player_mode
            || session.game_mode.starts_with("single-player")
            || session.game_mode == "training"
            || session.game_mode == "multiplayer"
        {
            let viewer_id = format!("viewer_{}", &uuid::Uuid::new_v4().to_string()[0..8]);
            let viewer_player = Player {
                id: viewer_id.clone(),
                name: player_name.clone(),
                score: 0,
                is_ready: true,
                is_connected: true,
                joined_at: chrono::Utc::now().timestamp(),
            };

            // ✅ AJOUTER LE VIEWER À LA SESSION POUR QU'IL REÇOIVE LES MISES À JOUR
            let mut updated_session = session.clone();
            updated_session
                .players
                .insert(viewer_id.clone(), viewer_player);
            update_session_with_manager(manager, updated_session.clone())
                .await
                .map_err(Status::internal)?;

            let session_id = updated_session.id.clone();
            let game_state = session_to_game_state(&updated_session);
            let response = join_success_response(session_id, viewer_id, game_state);
            return Ok(Response::new(response));
        } else {
            // Mode multijoueur - rejeter les viewers
            let response = join_error_response(
                "VIEWER_NOT_ALLOWED".to_string(),
                "Viewers not allowed in multiplayer mode".to_string(),
            );
            return Ok(Response::new(response));
        }
    }

    match add_player_to_session(session, player_name.clone()) {
        Ok((mut updated_session, player_id)) => {
            // ✅ AJOUTER MCTS AUTOMATIQUEMENT EN MODE SOLO
            if updated_session.game_mode.starts_with("single-player")
                || updated_session.game_mode == "training"
            {
                let mcts_player = Player {
                    id: "mcts_ai".to_string(),
                    name: "🤖 MCTS IA".to_string(),
                    score: 0,
                    is_ready: true,
                    is_connected: true,
                    joined_at: chrono::Utc::now().timestamp(),
                };
                updated_session
                    .players
                    .insert("mcts_ai".to_string(), mcts_player);
                log::info!("🤖 MCTS IA automatiquement ajouté en mode solo");
            }

            // 🎮 EN MODE SINGLE-PLAYER: joueur humain automatiquement prêt + démarrage auto
            if service.single_player_mode
                || updated_session.game_mode.starts_with("single-player")
                || updated_session.game_mode == "training"
            {
                if let Some(player) = updated_session.players.get_mut(&player_id) {
                    player.is_ready = true;
                    log::info!(
                        "🎯 Joueur {} automatiquement prêt en mode single-player",
                        player_name
                    );

                    // Vérifier si tous les joueurs sont prêts pour démarrer automatiquement
                    let all_ready = updated_session
                        .players
                        .values()
                        .all(|p| p.is_ready && p.is_connected);
                    let enough_players = updated_session.players.len() >= 2; // Humain + MCTS

                    if all_ready && enough_players && updated_session.state == 0 {
                        updated_session.state = 1; // IN_PROGRESS
                        if let Some(first_player_id) = updated_session.players.keys().next() {
                            updated_session.current_player_id = Some(first_player_id.clone());
                        }
                        updated_session.turn_number = 1;

                        // ✅ CRÉER ET DÉMARRER LE PREMIER TOUR AUTOMATIQUEMENT
                        use crate::services::game_manager::{
                            create_take_it_easy_game, start_new_turn,
                        };
                        let player_ids: Vec<String> =
                            updated_session.players.keys().cloned().collect();
                        let game_state =
                            create_take_it_easy_game(updated_session.id.clone(), player_ids);

                        // Démarrer immédiatement le premier tour avec une tuile
                        match start_new_turn(game_state) {
                            Ok(started_game) => {
                                updated_session.board_state =
                                    serde_json::to_string(&started_game).unwrap_or_default();
                                log::info!("🎮 Jeu ET premier tour automatiquement démarrés pour session {}", updated_session.code);
                                log::info!("🎲 Tuile proposée: {:?}", started_game.current_tile);
                            }
                            Err(e) => {
                                log::error!("❌ Échec démarrage premier tour: {}", e);
                                updated_session.board_state =
                                    r#"{"tiles": [], "available_positions": []}"#.to_string();
                            }
                        }
                    }
                }
            }

            let session_id = updated_session.id.clone();
            let game_state = session_to_game_state(&updated_session);
            update_session_with_manager(manager, updated_session)
                .await
                .map_err(Status::internal)?;

            let response = join_success_response(session_id, player_id, game_state);
            Ok(Response::new(response))
        }
        Err(e) => {
            log::error!("❌ Échec join session: {}", e);
            let response = join_error_response(e, "Failed to join session".to_string());
            Ok(Response::new(response))
        }
    }
}

// 🔧 FONCTION SET_READY AVEC DEBUG ULTRA-DÉTAILLÉ
async fn set_ready_logic(
    service: &SessionServiceImpl,
    session_id: String,
    player_id: String,
    ready: bool,
) -> Result<Response<SetReadyResponse>, Status> {
    let manager = &service.session_manager;
    // 🔍 Étape 1: Vérifier l'existence de la session AVANT transform
    match get_session_by_id_with_manager(manager, &session_id).await {
        Some(session) => {
            // Vérifier si le joueur existe
            if let Some(_player) = session.players.get(&player_id) {
            } else {
                log::error!(
                    "❌ Joueur {} introuvable dans session {}",
                    player_id,
                    session_id
                );
                return Ok(Response::new(set_ready_error_response(
                    "PLAYER_NOT_FOUND".to_string(),
                    format!("Player {} not found in session {}", player_id, session_id),
                )));
            }
        }
        None => {
            log::error!("❌ Session {} introuvable lors de SET_READY", session_id);

            // 🔍 Debug: Lister toutes les sessions existantes
            let store = get_store_from_manager(manager);
            let state = store.read().await;
            log::error!("🔍 Sessions existantes ({} total):", state.sessions.len());
            for (sid, session) in &state.sessions {
                log::error!(
                    "  - id={}, code={}, players={}",
                    sid,
                    session.code,
                    session.players.len()
                );
            }
            drop(state);

            return Ok(Response::new(set_ready_error_response(
                "SESSION_NOT_FOUND".to_string(),
                format!("Session {} not found", session_id),
            )));
        }
    }

    // 🔧 Étape 2: Continuer avec la logique normale
    let store = get_store_from_manager(manager);

    // Utilisation directe de transform_session_in_store pour récupérer game_started
    let result = transform_session_in_store(store, &session_id, |session| {
        // En mode single-player, démarrer dès qu'il y a 1 humain + MCTS (fonction standard)
        set_player_ready_in_session_with_min(session, &player_id, ready, 2)
    })
    .await;

    match result {
        Ok(Some(game_started)) => Ok(Response::new(set_ready_success_response(game_started))),
        Ok(None) => {
            log::error!("❌ Session {} disparue pendant transform", session_id);
            Ok(Response::new(set_ready_error_response(
                "SESSION_NOT_FOUND".to_string(),
                "Session not found during update".to_string(),
            )))
        }
        Err(error_code) => {
            log::error!("❌ Erreur pendant SET_READY: {}", error_code);
            Ok(Response::new(set_ready_error_response(
                error_code,
                "Failed to set ready status".to_string(),
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
        create_session_logic_with_manager(self, req.player_name, req.max_players, req.game_mode)
            .await
    }

    async fn join_session(
        &self,
        request: Request<JoinSessionRequest>,
    ) -> Result<Response<JoinSessionResponse>, Status> {
        let req = request.into_inner();
        log::info!(
            "🔄 Tentative JOIN_SESSION: code='{}', joueur='{}'",
            req.session_code,
            req.player_name
        );
        join_session_logic(self, req.session_code, req.player_name).await
    }

    async fn set_ready(
        &self,
        request: Request<SetReadyRequest>,
    ) -> Result<Response<SetReadyResponse>, Status> {
        let req = request.into_inner();
        set_ready_logic(self, req.session_id, req.player_id, req.ready).await
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
                let proto_players: Vec<crate::generated::takeiteasygame::v1::Player> = session
                    .players
                    .values()
                    .map(|p| crate::generated::takeiteasygame::v1::Player {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        score: p.score,
                        is_ready: p.is_ready,
                        is_connected: p.is_connected,
                        joined_at: p.joined_at,
                    })
                    .collect();

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
                    game_mode: session.game_mode.clone(),
                };

                Ok(Response::new(GetSessionStateResponse {
                    game_state: Some(game_state),
                    error: None,
                }))
            }
            None => {
                log::error!(
                    "❌ GET_SESSION_STATE: Session {} introuvable",
                    req.session_id
                );

                // 🎮 EN MODE SINGLE-PLAYER: utiliser la première session disponible
                if self.single_player_mode {
                    let store = get_store_from_manager(&self.session_manager);
                    let state = store.read().await;

                    if let Some((session_id, session)) = state.sessions.iter().next() {
                        let session_id_clone = session_id.clone();
                        let session_code = session.code.clone();
                        drop(state);

                        log::info!(
                            "🔄 Mode single-player: redirection vers session {}",
                            session_code
                        );

                        // Récursion avec la bonne session
                        let new_request = GetSessionStateRequest {
                            session_id: session_id_clone,
                        };
                        return self.get_session_state(Request::new(new_request)).await;
                    }

                    log::error!("🔍 Sessions existantes ({} total):", state.sessions.len());
                    for (sid, session) in &state.sessions {
                        log::error!("  - id={}, code={}", sid, session.code);
                    }
                    drop(state);
                }

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
