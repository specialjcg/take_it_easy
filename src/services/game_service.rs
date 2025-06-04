// src/services/game_service.rs - GameService étendu avec gameplay MCTS - VERSION CORRIGÉE

use tonic::{Request, Response, Status};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
// Import des types générés par tonic
use crate::generated::takeiteasygame::v1::*;
use crate::generated::takeiteasygame::v1::game_service_server::GameService;
use crate::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use crate::services::game_manager::{TakeItEasyGameState, MoveResult, create_take_it_easy_game, start_new_turn, process_player_move_with_mcts, get_available_positions, take_it_easy_state_to_protobuf, player_move_from_json, mcts_move_to_json, is_game_finished, ensure_current_tile, apply_player_move, MctsMove, PlayerMove, check_turn_completion};
use crate::services::session_manager::{get_store_from_manager, SessionManager, get_session_by_id_from_store, update_session_in_store, SessionStoreState, GameSession, get_session_by_code_from_store};
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::utils::image::generate_tile_image_names;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::tile::Tile;
// ============================================================================
// GAMESERVICE AVEC INTÉGRATION MCTS + NOUVELLES MÉTHODES GAMEPLAY
// ============================================================================

#[derive(Clone)]
pub struct GameServiceImpl {
    session_manager: Arc<SessionManager>,
    policy_net: Arc<Mutex<PolicyNet>>,
    value_net: Arc<Mutex<ValueNet>>,
    num_simulations: usize,
}

impl GameServiceImpl {
    pub fn new(
        session_manager: Arc<SessionManager>,
        policy_net: Arc<Mutex<PolicyNet>>,
        value_net: Arc<Mutex<ValueNet>>,
        num_simulations: usize
    ) -> Self {
        GameServiceImpl {
            session_manager,
            policy_net,
            value_net,
            num_simulations,
        }
    }
}

// ============================================================================
// FONCTIONS PURES - CRÉATION DE RÉPONSES (existantes + nouvelles)
// ============================================================================

fn make_move_success_response(move_result: MoveResult) -> MakeMoveResponse {
    let mcts_response_json = move_result.mcts_response
        .as_ref()
        .and_then(|mcts| mcts_move_to_json(mcts).ok())
        .unwrap_or_default();

    MakeMoveResponse {
        result: Some(make_move_response::Result::Success(
            MakeMoveSuccess {
                new_game_state: Some(take_it_easy_state_to_protobuf(&move_result.new_game_state)),
                mcts_response: mcts_response_json,
                points_earned: move_result.points_earned,
                is_game_over: move_result.is_game_over,
            }
        )),
    }
}

fn make_move_error_response(code: String, message: String) -> MakeMoveResponse {
    MakeMoveResponse {
        result: Some(make_move_response::Result::Error(Error {
            code,
            message,
            details: std::collections::HashMap::new(),
        })),
    }
}

fn available_moves_success_response(positions: Vec<usize>, current_tile: Option<crate::game::tile::Tile>) -> GetAvailableMovesResponse {
    let moves_json: Vec<String> = positions
        .iter()
        .map(|pos| {
            serde_json::json!({
                "position": pos,
                "tile": current_tile.map(|t| (t.0, t.1, t.2))
            }).to_string()
        })
        .collect();

    GetAvailableMovesResponse {
        available_moves: moves_json,
        error: None,
    }
}

fn available_moves_error_response(code: String, message: String) -> GetAvailableMovesResponse {
    GetAvailableMovesResponse {
        available_moves: vec![],
        error: Some(Error {
            code,
            message,
            details: std::collections::HashMap::new(),
        }),
    }
}

fn start_turn_success_response(
    announced_tile: String,
    tile_image: String,
    turn_number: i32,
    waiting_for_players: Vec<String>,
    game_state_json: String
) -> StartTurnResponse {
    StartTurnResponse {
        success: true,
        announced_tile,
        tile_image,
        turn_number,
        waiting_for_players,
        game_state: game_state_json,
        error: None,
    }
}

fn start_turn_error_response(message: String) -> StartTurnResponse {
    StartTurnResponse {
        success: false,
        announced_tile: String::new(),
        tile_image: String::new(),
        turn_number: 0,
        waiting_for_players: vec![],
        game_state: String::new(),
        error: Some(Error {
            code: "START_TURN_FAILED".to_string(),
            message,
            details: std::collections::HashMap::new(),
        }),
    }
}

fn game_state_success_response(
    game_state_json: String,
    current_tile: String,
    current_tile_image: String, // ✅ NOUVEAU PARAMÈTRE
    current_turn: i32,
    waiting_for_players: Vec<String>,
    is_game_finished: bool,
    final_scores_json: String
) -> GetGameStateResponse {
    GetGameStateResponse {
        success: true,
        game_state: game_state_json,
        current_tile,
        current_tile_image, // ✅ NOUVEAU CHAMP
        current_turn,
        waiting_for_players,
        is_game_finished,
        final_scores: final_scores_json,
        error: None,
    }
}

fn game_state_error_response(message: String) -> GetGameStateResponse {
    GetGameStateResponse {
        success: false,
        game_state: String::new(),
        current_tile: String::new(),
        current_tile_image: String::new(), // ✅ Chaîne vide (pas d'image)
        current_turn: 0,
        waiting_for_players: vec![],
        is_game_finished: false,
        final_scores: String::new(),
        error: Some(Error {
            code: "GET_GAME_STATE_FAILED".to_string(),
            message,
            details: std::collections::HashMap::new(),
        }),
    }
}

// ============================================================================
// FONCTION AUXILIAIRE : process_mcts_move_only
// ============================================================================

pub fn process_mcts_move_only(
    game_state: TakeItEasyGameState,
    policy_net: &Mutex<PolicyNet>,
    value_net: &Mutex<ValueNet>,
    num_simulations: usize
) -> Result<(TakeItEasyGameState, MctsMove), String> {

    // ✅ VÉRIFICATION: MCTS doit être en attente
    if !game_state.waiting_for_players.contains(&"mcts_ai".to_string()) {
        return Err("MCTS_NOT_WAITING".to_string());
    }

    let current_tile = game_state.current_tile.ok_or("NO_CURRENT_TILE")?;

    // Récupérer le plateau MCTS
    let mcts_plateau = game_state.player_plateaus.get("mcts_ai")
        .ok_or("MCTS_PLAYER_NOT_FOUND")?
        .clone();

    // ✅ VÉRIFICATION: Mouvements légaux
    let legal_moves = get_legal_moves(mcts_plateau.clone());
    if legal_moves.is_empty() {
        return Err("NO_LEGAL_MOVES_FOR_MCTS".to_string());
    }


    let mut deck_clone = game_state.deck.clone();

    // Verrouiller les réseaux
    let policy_locked = policy_net.lock().map_err(|_| "Failed to lock policy net")?;
    let value_locked = value_net.lock().map_err(|_| "Failed to lock value net")?;

    // ✅ EXÉCUTION MCTS
    let mut mcts_plateau_mut = mcts_plateau.clone();
    let mcts_result = mcts_find_best_position_for_tile_with_nn(
        &mut mcts_plateau_mut,
        &mut deck_clone,
        current_tile,
        &*policy_locked,
        &*value_locked,
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

// ============================================================================
// FONCTION POUR GÉRER session par code OU ID
// ============================================================================

pub async fn get_session_by_code_or_id_from_store(
    store: &Arc<RwLock<SessionStoreState>>,
    identifier: &str
) -> Option<GameSession> {
    // Essayer d'abord par ID (UUID)
    if let Some(session) = get_session_by_id_from_store(store, identifier).await {
        return Some(session);
    }

    // Ensuite essayer par code (ex: "GTHG7Q")
    get_session_by_code_from_store(store, identifier).await
}

// ============================================================================
// FONCTIONS COMPOSABLES - LOGIQUE MÉTIER (existantes + nouvelles)
// ============================================================================

async fn make_move_logic(
    service: &GameServiceImpl,
    session_id: String,
    player_id: String,
    move_data: String,
    timestamp: i64
) -> Result<Response<MakeMoveResponse>, Status> {
    let store = get_store_from_manager(&service.session_manager);

    let session = match get_session_by_code_or_id_from_store(store, &session_id).await {
        Some(session) => session,
        None => {
            let response = make_move_error_response(
                "SESSION_NOT_FOUND".to_string(),
                "Session not found".to_string()
            );
            return Ok(Response::new(response));
        }
    };

    // Récupérer l'état de jeu
    let game_state: TakeItEasyGameState = if session.board_state.is_empty() || session.board_state == "{}" {
        let response = make_move_error_response(
            "GAME_NOT_STARTED".to_string(),
            "No game state found. Please start a turn first.".to_string()
        );
        return Ok(Response::new(response));
    } else {
        serde_json::from_str(&session.board_state)
            .map_err(|e| Status::internal(format!("Failed to parse game state: {}", e)))?
    };

    // Vérification: S'assurer qu'une tuile courante existe
    let game_state = match ensure_current_tile(game_state) {
        Ok(state) => {            state
        },
        Err(e) => {
            log::error!("❌ Échec garantie tuile: {}", e);
            return Ok(Response::new(make_move_error_response(
                "NO_CURRENT_TILE".to_string(),
                format!("No current tile available: {}", e)
            )));
        }
    };
    // Parser le mouvement du joueur
    let player_move = match player_move_from_json(&move_data, &player_id) {
        Ok(mv) => {
            let mut mv = mv;
            mv.timestamp = timestamp;
            if let Some(current_tile) = game_state.current_tile {
                mv.tile = current_tile;
            }
            mv
        },
        Err(e) => {
            let response = make_move_error_response(
                "INVALID_MOVE_FORMAT".to_string(),
                format!("Failed to parse move: {}", e)
            );
            return Ok(Response::new(response));
        }
    };

    // Traitement du mouvement avec MCTS
    match process_player_move_with_mcts(
        game_state,
        player_move,
        &*service.policy_net,
        &*service.value_net,
        service.num_simulations
    ) {
        Ok(move_result) => {
            let final_state = move_result.new_game_state.clone();

            // Sauvegarder l'état final
            let mut updated_session = session;
            updated_session.board_state = serde_json::to_string(&final_state).unwrap_or_default();

            // Mettre à jour les scores
            for (player_id, score) in &final_state.scores {
                if let Some(player) = updated_session.players.get_mut(player_id) {
                    player.score = *score;
                }
            }

            update_session_in_store(store, updated_session).await
                .map_err(|e| Status::internal(e))?;

            let response = make_move_success_response(move_result);
            Ok(Response::new(response))
        },
        Err(error_code) => {
            let response = make_move_error_response(
                error_code,
                "Failed to process move".to_string()
            );
            Ok(Response::new(response))
        }
    }
}

async fn get_available_moves_logic(
    service: &GameServiceImpl,
    session_id: String,
    player_id: String
) -> Result<Response<GetAvailableMovesResponse>, Status> {
    let store = get_store_from_manager(&service.session_manager);

    // Récupérer la session
    let session = match get_session_by_id_from_store(store, &session_id).await {
        Some(session) => session,
        None => {
            let response = available_moves_error_response(
                "SESSION_NOT_FOUND".to_string(),
                "Session not found".to_string()
            );
            return Ok(Response::new(response));
        }
    };

    // Récupérer l'état de jeu
    let game_state: TakeItEasyGameState = if session.board_state.is_empty() || session.board_state == "{}" {
        let response = available_moves_error_response(
            "GAME_NOT_STARTED".to_string(),
            "Game has not started yet".to_string()
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
            "Game is already finished".to_string()
        );
        return Ok(Response::new(response));
    }

    // Obtenir les positions disponibles
    let available_positions = get_available_positions(&game_state, &player_id);
    let current_tile = game_state.current_tile;

    let response = available_moves_success_response(available_positions, current_tile);
    Ok(Response::new(response))
}

async fn start_turn_logic(
    service: &GameServiceImpl,
    session_id: String
) -> Result<Response<StartTurnResponse>, Status> {
    let store = get_store_from_manager(&service.session_manager);
    let session = match get_session_by_code_or_id_from_store(store, &session_id).await {
        Some(session) => {            session
        },
        None => {
            return Ok(Response::new(start_turn_error_response("Session not found".to_string())));
        }
    };

    // Récupérer ou créer l'état de jeu
    let mut game_state: TakeItEasyGameState = if session.board_state.is_empty() || session.board_state == "{}" {
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
            Err(e) => {                let player_ids: Vec<String> = session.players.keys().cloned().collect();
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
            Ok(new_state) => {
                new_state
            },
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
            &*service.policy_net,
            &*service.value_net,
            service.num_simulations
        ) {
            Ok((updated_state, _mcts_move)) => {
                let update_state_clone = updated_state.clone();
                // Vérifier si le tour est terminé après que MCTS ait joué
                match check_turn_completion(update_state_clone) {
                    Ok(completed_state) => {
                        completed_state
                    },
                    Err(_e) => {
                        updated_state.clone()
                    }
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
fn enhance_game_state_with_images(board_state: &str) -> String {
    // Parser le JSON existant
    let mut game_data = serde_json::from_str::<serde_json::Value>(board_state).unwrap_or_else(|_| {
        // Si parsing échoue, créer structure minimale
        log::warn!("Parsing board_state échoué, création structure par défaut");
        serde_json::json!({
                "player_plateaus": {}
            })
    });

    // 🛡️ GARANTIR que player_plateaus existe
    if !game_data.get("player_plateaus").is_some() {
        game_data["player_plateaus"] = serde_json::json!({});
    }

    // 🛡️ GARANTIR que chaque plateau a tile_images et available_positions
    if let Some(player_plateaus) = game_data.get_mut("player_plateaus") {
        if let Some(plateaus_obj) = player_plateaus.as_object_mut() {

            for (player_id, plateau_data) in plateaus_obj.iter_mut() {

                // 🛡️ GARANTIR que tiles existe, sinon créer plateau vide
                let tiles_array = match plateau_data.get("tiles") {
                    Some(tiles) => tiles.clone(),
                    None => {
                        log::warn!("Plateau manquant pour {}, création plateau vide", player_id);
                        serde_json::json!(vec![[0, 0, 0]; 19]) // 19 positions vides
                    }
                };

                if let Some(tiles) = tiles_array.as_array() {
                    // Convertir JSON tiles vers Rust Tiles
                    let rust_tiles: Vec<Tile> = tiles
                        .iter()
                        .map(|tile_json| {
                            if let Some(tile_array) = tile_json.as_array() {
                                if tile_array.len() == 3 {
                                    Tile(
                                        tile_array[0].as_i64().unwrap_or(0) as i32,
                                        tile_array[1].as_i64().unwrap_or(0) as i32,
                                        tile_array[2].as_i64().unwrap_or(0) as i32,
                                    )
                                } else {
                                    log::warn!("Tuile malformée pour {}, utilisation (0,0,0)", player_id);
                                    Tile(0, 0, 0)
                                }
                            } else {
                                log::warn!("Format tuile invalide pour {}, utilisation (0,0,0)", player_id);
                                Tile(0, 0, 0)
                            }
                        })
                        .collect();

                    // 🚀 TOUJOURS GÉNÉRER tile_images (garantie 100%)
                    let tile_images = generate_tile_image_names(&rust_tiles);

                    // 🚀 TOUJOURS CALCULER available_positions (garantie 100%)
                    let available_positions: Vec<i32> = rust_tiles
                        .iter()
                        .enumerate()
                        .filter_map(|(index, tile)| {
                            if *tile == Tile(0, 0, 0) {
                                Some(index as i32)
                            } else {
                                None
                            }
                        })
                        .collect();

                    // 🛡️ GARANTIR l'ajout au JSON (même si plateau_data est malformé)
                    if let Some(plateau_obj) = plateau_data.as_object_mut() {
                        // Mettre à jour les données existantes
                        plateau_obj.insert("tiles".to_string(), tiles_array);
                        plateau_obj.insert(
                            "tile_images".to_string(),
                            serde_json::Value::Array(
                                tile_images.into_iter()
                                    .map(serde_json::Value::String)
                                    .collect()
                            )
                        );
                        plateau_obj.insert(
                            "available_positions".to_string(),
                            serde_json::Value::Array(
                                available_positions.into_iter()
                                    .map(|pos| serde_json::Value::Number(pos.into()))
                                    .collect()
                            )
                        );
                    } else {
                        // Si plateau_data n'est pas un objet, le recréer
                        *plateau_data = serde_json::json!({
                            "tiles": tiles_array,
                            "tile_images": tile_images,
                            "available_positions": available_positions
                        });
                    }
                } else {
                    // Si tiles n'est pas un array, créer plateau vide par défaut
                    log::warn!("tiles n'est pas un array pour {}, création plateau vide", player_id);
                    let empty_tiles = vec![Tile(0, 0, 0); 19];
                    let empty_images = generate_tile_image_names(&empty_tiles);
                    let all_positions: Vec<i32> = (0..19).collect();

                    *plateau_data = serde_json::json!({
                        "tiles": vec![[0, 0, 0]; 19],
                        "tile_images": empty_images,
                        "available_positions": all_positions
                    });
                }
            }
        }
    }

    // Retourner le JSON enrichi (garantie que tile_images existe)
    game_data.to_string()
}
// Dans game_service.rs - Ajouter ces lignes dans get_game_state_logic

async fn get_game_state_logic(
    service: &GameServiceImpl,
    session_id: String
) -> Result<Response<GetGameStateResponse>, Status> {
    let store = get_store_from_manager(&service.session_manager);

    // Récupérer la session
    let session = match get_session_by_id_from_store(store, &session_id).await {
        Some(session) => session,
        None => {
            return Ok(Response::new(game_state_error_response("Session not found".to_string())));
        }
    };

    if session.board_state.is_empty() || session.board_state == "{}" {
        return Ok(Response::new(game_state_error_response("Game not started yet".to_string())));
    }

    let game_state: TakeItEasyGameState = match serde_json::from_str(&session.board_state) {
        Ok(state) => state,
        Err(e) => return Ok(Response::new(game_state_error_response(format!("Failed to parse game state: {}", e)))),
    };

    let current_tile_str = game_state.current_tile
        .map(|t| format!("{}-{}-{}", t.0, t.1, t.2))
        .unwrap_or_default();

    // ✅ CORRECTION: Gérer tuile vide (0,0,0)
    let current_tile_image = game_state.current_tile
        .filter(|tile| *tile != Tile(0, 0, 0)) // ✅ Filtrer les tuiles vides
        .map(|tile| {
            let tile_images = generate_tile_image_names(&[tile]);
            tile_images[0].clone()
        })
        .unwrap_or_else(|| String::new()); // ✅ Chaîne vide au lieu de "000.png"

    let final_scores_json = if is_game_finished(&game_state) {
        serde_json::to_string(&game_state.scores).unwrap_or_default()
    } else {
        "{}".to_string()
    };

    let current_turn = game_state.current_turn as i32;
    let waiting_for_players = game_state.waiting_for_players.clone();
    let is_finished = is_game_finished(&game_state);
    let game_state_json = serde_json::to_string(&game_state).unwrap_or_default();

    // ✅ Enrichir avec les images
    let enhanced_game_state_json = enhance_game_state_with_images(&game_state_json);

    let response = game_state_success_response(
        enhanced_game_state_json,
        current_tile_str,
        current_tile_image, // ✅ Sera vide si pas de tuile
        current_turn,
        waiting_for_players,
        is_finished,
        final_scores_json
    );

    Ok(Response::new(response))
}

// ============================================================================
// IMPLÉMENTATION GRPC - TRAIT GÉNÉRÉ PAR TONIC
// ============================================================================

#[tonic::async_trait]
impl GameService for GameServiceImpl {
    async fn make_move(
        &self,
        request: Request<MakeMoveRequest>,
    ) -> Result<Response<MakeMoveResponse>, Status> {
        let req = request.into_inner();
        make_move_logic(
            self,
            req.session_id,
            req.player_id,
            req.move_data,
            req.timestamp
        ).await
    }

    async fn get_available_moves(
        &self,
        request: Request<GetAvailableMovesRequest>,
    ) -> Result<Response<GetAvailableMovesResponse>, Status> {
        let req = request.into_inner();
        get_available_moves_logic(
            self,
            req.session_id,
            req.player_id
        ).await
    }

    async fn start_turn(
        &self,
        request: Request<StartTurnRequest>,
    ) -> Result<Response<StartTurnResponse>, Status> {
        let req = request.into_inner();
        start_turn_logic(self, req.session_id).await
    }

    async fn get_game_state(
        &self,
        request: Request<GetGameStateRequest>,
    ) -> Result<Response<GetGameStateResponse>, Status> {
        let req = request.into_inner();
        get_game_state_logic(self, req.session_id).await
    }

}
// src/services/game_service.rs - TESTS TDD POUR enhance_game_state_with_images

// ... votre code existant de GameService ...

// À la fin du fichier, ajouter ces tests TDD :

#[cfg(test)]
mod image_enhancement_tests {
    use super::*;
    use crate::game::create_deck::create_deck;
    use crate::game::tile::Tile;

    // ========================================================================
    // TESTS TDD POUR enhance_game_state_with_images (fonction core)
    // ========================================================================

    #[test]
    fn test_enhance_game_state_basic_plateau() {
        // Arrange - Créer un JSON de base avec vraies tuiles du deck
        let deck = create_deck();
        let input_json = format!(r#"{{
            "player_plateaus": {{
                "player1": {{
                    "tiles": [[{},{},{}], [0,0,0], [{},{},{}]]
                }}
            }}
        }}"#,
                                 deck.tiles[0].0, deck.tiles[0].1, deck.tiles[0].2,
                                 deck.tiles[1].0, deck.tiles[1].1, deck.tiles[1].2
        );

        // Act
        let result = enhance_game_state_with_images(&input_json);

        // Assert
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let plateau = &parsed["player_plateaus"]["player1"];

        // Vérifier que tile_images a été ajouté
        assert!(plateau.get("tile_images").is_some());
        let images = plateau["tile_images"].as_array().unwrap();
        assert_eq!(images.len(), 3);

        // Vérifier le format des images
        assert!(images[0].as_str().unwrap().starts_with("../image/"));
        assert!(images[0].as_str().unwrap().ends_with(".png"));
        assert_eq!(images[1], "../image/000.png"); // Position vide

        // Vérifier que available_positions a été ajouté
        assert!(plateau.get("available_positions").is_some());
        let positions = plateau["available_positions"].as_array().unwrap();
        assert_eq!(positions.len(), 1); // Une seule position vide (index 1)
        assert_eq!(positions[0], 1);
    }

    #[test]
    fn test_enhance_game_state_empty_plateau() {
        // Test avec plateau entièrement vide (cas réaliste début de partie)
        let input_json = r#"{
            "player_plateaus": {
                "player1": {
                    "tiles": [[0,0,0], [0,0,0], [0,0,0], [0,0,0], [0,0,0]]
                }
            }
        }"#;

        let result = enhance_game_state_with_images(input_json);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let plateau = &parsed["player_plateaus"]["player1"];
        let images = plateau["tile_images"].as_array().unwrap();
        let positions = plateau["available_positions"].as_array().unwrap();

        // Toutes les images devraient être vides
        assert_eq!(images.len(), 5);
        for image in images {
            assert_eq!(image, "../image/000.png");
        }

        // Toutes les positions devraient être disponibles
        assert_eq!(positions.len(), 5);
        for (i, pos) in positions.iter().enumerate() {
            assert_eq!(pos, i);
        }
    }

    #[test]
    fn test_enhance_game_state_full_plateau() {
        // Test avec plateau entièrement rempli (cas réaliste fin de partie)
        let deck = create_deck();
        let input_json = format!(r#"{{
            "player_plateaus": {{
                "player1": {{
                    "tiles": [
                        [{},{},{}],
                        [{},{},{}],
                        [{},{},{}]
                    ]
                }}
            }}
        }}"#,
                                 deck.tiles[0].0, deck.tiles[0].1, deck.tiles[0].2,
                                 deck.tiles[1].0, deck.tiles[1].1, deck.tiles[1].2,
                                 deck.tiles[2].0, deck.tiles[2].1, deck.tiles[2].2
        );

        let result = enhance_game_state_with_images(&input_json);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let plateau = &parsed["player_plateaus"]["player1"];
        let images = plateau["tile_images"].as_array().unwrap();
        let positions = plateau["available_positions"].as_array().unwrap();

        // Toutes les images devraient être non-vides
        assert_eq!(images.len(), 3);
        for image in images {
            let img_str = image.as_str().unwrap();
            assert!(img_str.starts_with("../image/"));
            assert!(img_str.ends_with(".png"));
            assert_ne!(img_str, "../image/000.png"); // Aucune image vide
        }

        // Aucune position disponible
        assert_eq!(positions.len(), 0);
    }

    #[test]
    fn test_enhance_game_state_multiple_players() {
        // Test avec plusieurs joueurs (cas réaliste multijoueur)
        let deck = create_deck();
        let input_json = format!(r#"{{
            "player_plateaus": {{
                "player1": {{
                    "tiles": [[{},{},{}], [0,0,0]]
                }},
                "player2": {{
                    "tiles": [[0,0,0], [{},{},{}]]
                }},
                "mcts_ai": {{
                    "tiles": [[{},{},{}], [{},{},{}]]
                }}
            }}
        }}"#,
                                 deck.tiles[0].0, deck.tiles[0].1, deck.tiles[0].2,
                                 deck.tiles[1].0, deck.tiles[1].1, deck.tiles[1].2,
                                 deck.tiles[2].0, deck.tiles[2].1, deck.tiles[2].2,
                                 deck.tiles[3].0, deck.tiles[3].1, deck.tiles[3].2
        );

        let result = enhance_game_state_with_images(&input_json);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        // Vérifier chaque joueur
        for player_id in ["player1", "player2", "mcts_ai"] {
            let plateau = &parsed["player_plateaus"][player_id];

            assert!(plateau.get("tile_images").is_some());
            assert!(plateau.get("available_positions").is_some());

            let images = plateau["tile_images"].as_array().unwrap();
            let positions = plateau["available_positions"].as_array().unwrap();

            assert_eq!(images.len(), 2); // Chaque joueur a 2 positions

            // Vérifier cohérence images/positions
            for (index, image) in images.iter().enumerate() {
                let is_empty_image = image == "../image/000.png";
                let is_available_position = positions.contains(&serde_json::Value::Number((index as i32).into()));

                // Si image vide, position devrait être disponible
                assert_eq!(is_empty_image, is_available_position);
            }
        }
    }

    #[test]
    fn test_enhance_game_state_realistic_19_positions() {
        // Test avec 19 positions (taille réelle du plateau Take It Easy)
        let deck = create_deck();
        let mut tiles_array = vec![String::from("[0,0,0]"); 19]; // 19 positions vides

        // Placer quelques tuiles réalistes
        tiles_array[0] = format!("[{},{},{}]", deck.tiles[0].0, deck.tiles[0].1, deck.tiles[0].2);
        tiles_array[8] = format!("[{},{},{}]", deck.tiles[1].0, deck.tiles[1].1, deck.tiles[1].2); // Centre
        tiles_array[18] = format!("[{},{},{}]", deck.tiles[2].0, deck.tiles[2].1, deck.tiles[2].2); // Coin

        let input_json = format!(r#"{{
            "player_plateaus": {{
            
                "player1": {{
                    "tiles": [{}]
                }}
            }}
        }}"#, tiles_array.join(", "));

        let result = enhance_game_state_with_images(&input_json);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let plateau = &parsed["player_plateaus"]["player1"];
        let images = plateau["tile_images"].as_array().unwrap();
        let positions = plateau["available_positions"].as_array().unwrap();

        // Vérifier la taille réelle du plateau
        assert_eq!(images.len(), 19);

        // 3 tuiles placées, donc 16 positions disponibles
        assert_eq!(positions.len(), 16);

        // Vérifier les positions occupées
        assert_ne!(images[0], "../image/000.png");  // Position 0 occupée
        assert_ne!(images[8], "../image/000.png");  // Position 8 occupée (centre)
        assert_ne!(images[18], "../image/000.png"); // Position 18 occupée

        // Vérifier que les positions occupées ne sont pas dans available_positions
        assert!(!positions.contains(&serde_json::Value::Number(0.into())));
        assert!(!positions.contains(&serde_json::Value::Number(8.into())));
        assert!(!positions.contains(&serde_json::Value::Number(18.into())));
    }

    // ========================================================================
    // TESTS D'ERREUR ET CAS LIMITES (TDD robustesse)
    // ========================================================================

    #[test]
    fn test_enhance_game_state_invalid_json() {
        // Test avec JSON invalide
        let result = enhance_game_state_with_images("invalid json");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        // Devrait créer structure par défaut
        assert!(parsed.get("player_plateaus").is_some());
    }

    #[test]
    fn test_enhance_game_state_missing_player_plateaus() {
        // Test sans player_plateaus
        let input_json = r#"{"some_other_field": "value"}"#;

        let result = enhance_game_state_with_images(input_json);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        // Devrait ajouter player_plateaus
        assert!(parsed.get("player_plateaus").is_some());
    }

    #[test]
    fn test_enhance_game_state_malformed_tiles() {
        // Test avec tuiles malformées
        let input_json = r#"{
            "player_plateaus": {
                "player1": {
                    "tiles": ["invalid", [1,2], [1,2,3,4]]
                }
            }
        }"#;

        let result = enhance_game_state_with_images(input_json);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let plateau = &parsed["player_plateaus"]["player1"];
        let images = plateau["tile_images"].as_array().unwrap();

        // Devrait gérer les tuiles malformées en les remplaçant par (0,0,0)
        assert_eq!(images.len(), 3);
        for image in images {
            assert_eq!(image, "../image/000.png"); // Tuiles malformées → vides
        }
    }

    // ========================================================================
    // TESTS DE PERFORMANCE (TDD performance)
    // ========================================================================

    #[test]
    fn test_enhance_game_state_performance_large_game() {
        // Test avec plusieurs joueurs et plateaux complets
        let deck = create_deck();
        let mut large_input = String::from(r#"{"player_plateaus": {"#);

        // Créer 4 joueurs avec plateaux de 19 positions chacun
        for player_num in 1..=4 {
            if player_num > 1 { large_input.push_str(", "); }
            large_input.push_str(&format!(r#""player{}": {{"tiles": ["#, player_num));

            for pos in 0..19 {
                if pos > 0 { large_input.push_str(", "); }
                let tile = &deck.tiles[pos % deck.tiles.len()];
                large_input.push_str(&format!("[{},{},{}]", tile.0, tile.1, tile.2));
            }

            large_input.push_str("]}");
        }
        large_input.push_str("}}");

        // Mesurer le temps
        let start = std::time::Instant::now();
        let result = enhance_game_state_with_images(&large_input);
        let duration = start.elapsed();

        // Performance : ne devrait pas prendre plus de 10ms pour 4 joueurs × 19 positions
        assert!(duration.as_millis() < 10, "Performance trop lente: {:?}", duration);

        // Vérifier que le résultat est correct
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        for player_num in 1..=4 {
            let plateau = &parsed["player_plateaus"][&format!("player{}", player_num)];
            let images = plateau["tile_images"].as_array().unwrap();
            assert_eq!(images.len(), 19);
        }
    }

    // ========================================================================
    // TESTS D'INTÉGRATION AVEC generate_tile_image_names
    // ========================================================================

    #[test]
    fn test_enhance_uses_generate_tile_image_names_correctly() {
        // Test que enhance_game_state_with_images utilise bien generate_tile_image_names
        let deck = create_deck();
        let test_tile = &deck.tiles[0];

        // Générer l'image attendue avec la fonction directe
        let expected_image =generate_tile_image_names(&[*test_tile])[0].clone();

        // Tester via enhance_game_state_with_images
        let input_json = format!(r#"{{
            "player_plateaus": {{
                "player1": {{
                    "tiles": [[{},{},{}]]
                }}
            }}
        }}"#, test_tile.0, test_tile.1, test_tile.2);

        let result = enhance_game_state_with_images(&input_json);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let actual_image = parsed["player_plateaus"]["player1"]["tile_images"][0].as_str().unwrap();

        // Les deux méthodes devraient donner le même résultat
        assert_eq!(actual_image, expected_image);
    }
}