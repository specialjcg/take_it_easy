// src/services/game_service/state_provider.rs - Fournisseur d'état de jeu

use std::sync::Arc;
use tonic::{Response, Status};

use crate::game::tile::Tile;
use crate::generated::takeiteasygame::v1::*;
use crate::services::game_manager::{
    get_all_players_status, is_game_finished, TakeItEasyGameState,
};
use crate::services::session_manager::{
    get_session_by_id_from_store, get_store_from_manager, update_session_in_store, SessionManager,
};
use crate::utils::image::generate_tile_image_names;

use super::response_builders::{game_state_error_response, game_state_success_response};

// ============================================================================
// LOGIQUE DE FOURNITURE D'ÉTAT
// ============================================================================

pub async fn get_game_state_logic(
    session_manager: &Arc<SessionManager>,
    session_id: String,
) -> Result<Response<GetGameStateResponse>, Status> {
    let store = get_store_from_manager(session_manager);

    // Récupérer la session
    let session = match get_session_by_id_from_store(store, &session_id).await {
        Some(session) => session,
        None => {
            return Ok(Response::new(game_state_error_response(
                "Session not found".to_string(),
            )));
        }
    };

    if session.board_state.is_empty() || session.board_state == "{}" {
        return Ok(Response::new(game_state_error_response(
            "Game not started yet".to_string(),
        )));
    }

    let game_state: TakeItEasyGameState = match serde_json::from_str(&session.board_state) {
        Ok(state) => state,
        Err(e) => {
            return Ok(Response::new(game_state_error_response(format!(
                "Failed to parse game state: {}",
                e
            ))))
        }
    };

    let current_tile_str = game_state
        .current_tile
        .map(|t| format!("{}-{}-{}", t.0, t.1, t.2))
        .unwrap_or_default();

    // ✅ CORRECTION: Gérer tuile vide (0,0,0)
    let current_tile_image = game_state
        .current_tile
        .filter(|tile| *tile != Tile(0, 0, 0)) // ✅ Filtrer les tuiles vides
        .map(|tile| {
            let tile_images = generate_tile_image_names(&[tile]);
            tile_images[0].clone()
        })
        .unwrap_or_default(); // ✅ Chaîne vide au lieu de "000.png"

    let final_scores_json = if is_game_finished(&game_state) {
        serde_json::to_string(&game_state.scores).unwrap_or_default()
    } else {
        "{}".to_string()
    };

    // ✅ CRITICAL: Synchroniser les scores avec la session avant réponse
    let mut updated_session = session.clone();
    for (player_id, score) in &game_state.scores {
        if let Some(player) = updated_session.players.get_mut(player_id) {
            player.score = *score;
        }
    }
    if updated_session.players != session.players {
        if let Err(e) = update_session_in_store(store, updated_session.clone()).await {
            log::error!("Failed to sync scores in GetGameState: {}", e);
        }
    }

    let current_turn = game_state.current_turn as i32;
    let waiting_for_players = game_state.waiting_for_players.clone();
    let is_finished = is_game_finished(&game_state);
    let game_state_json = serde_json::to_string(&game_state).unwrap_or_default();

    // ✅ Enrichir avec les images et statuts des joueurs
    let mut enhanced_game_state_json = enhance_game_state_with_images(&game_state_json);

    // Ajouter les statuts des joueurs pour le flow indépendant
    let players_status = get_all_players_status(&game_state);
    let mut enhanced_data: serde_json::Value =
        serde_json::from_str(&enhanced_game_state_json).unwrap_or_else(|_| serde_json::json!({}));

    enhanced_data["players_status"] = serde_json::to_value(&players_status).unwrap_or_default();
    enhanced_game_state_json = enhanced_data.to_string();

    let response = game_state_success_response(
        enhanced_game_state_json,
        current_tile_str,
        current_tile_image, // ✅ Sera vide si pas de tuile
        current_turn,
        waiting_for_players,
        is_finished,
        final_scores_json,
    );

    Ok(Response::new(response))
}

// ============================================================================
// UTILITAIRE D'AMÉLIORATION D'ÉTAT AVEC IMAGES
// ============================================================================

type TileCache = std::collections::HashMap<usize, (Vec<String>, Vec<i32>)>;

pub fn enhance_game_state_with_images(board_state: &str) -> String {
    use std::sync::OnceLock;

    static EMPTY_TILE_CACHE: OnceLock<TileCache> = OnceLock::new();

    let cache = EMPTY_TILE_CACHE.get_or_init(|| {
        let mut cache = TileCache::new();
        for size in [19, 20, 25] {
            let empty_tiles = vec![Tile(0, 0, 0); size];
            let empty_images = generate_tile_image_names(&empty_tiles);
            let all_positions: Vec<i32> = (0..size as i32).collect();
            cache.insert(size, (empty_images, all_positions));
        }
        cache
    });

    let mut game_data =
        serde_json::from_str::<serde_json::Value>(board_state).unwrap_or_else(|_| {
            log::warn!("Parsing board_state échoué, création structure par défaut");
            serde_json::json!({"player_plateaus": {}})
        });

    if game_data.get("player_plateaus").is_none() {
        game_data["player_plateaus"] = serde_json::json!({});
    }

    if let Some(player_plateaus) = game_data.get_mut("player_plateaus") {
        if let Some(plateaus_obj) = player_plateaus.as_object_mut() {
            for (player_id, plateau_data) in plateaus_obj.iter_mut() {
                let tiles_array = match plateau_data.get("tiles") {
                    Some(tiles) => tiles.clone(),
                    None => {
                        log::warn!("Plateau manquant pour {}, création plateau vide", player_id);
                        if let Some((_, default_positions)) = cache.get(&19) {
                            *plateau_data = serde_json::json!({
                                "tiles": vec![[0, 0, 0]; 19],
                                "tile_images": vec!["../image/000.png"; 19],
                                "available_positions": default_positions
                            });
                        }
                        continue;
                    }
                };

                if let Some(tiles) = tiles_array.as_array() {
                    let tiles_len = tiles.len();
                    let mut tile_images = Vec::with_capacity(tiles_len);
                    let mut available_positions = Vec::new();

                    for (index, tile_json) in tiles.iter().enumerate() {
                        let tile = if let Some(tile_array) = tile_json.as_array() {
                            if tile_array.len() == 3 {
                                Tile(
                                    tile_array[0].as_i64().unwrap_or(0) as i32,
                                    tile_array[1].as_i64().unwrap_or(0) as i32,
                                    tile_array[2].as_i64().unwrap_or(0) as i32,
                                )
                            } else {
                                Tile(0, 0, 0)
                            }
                        } else {
                            Tile(0, 0, 0)
                        };

                        if tile == Tile(0, 0, 0) {
                            tile_images.push("../image/000.png".to_string());
                            available_positions.push(index as i32);
                        } else {
                            let tile_image = generate_tile_image_names(&[tile])[0].clone();
                            tile_images.push(tile_image);
                        }
                    }

                    if let Some(plateau_obj) = plateau_data.as_object_mut() {
                        plateau_obj.insert("tiles".to_string(), tiles_array);
                        plateau_obj.insert(
                            "tile_images".to_string(),
                            serde_json::Value::Array(
                                tile_images
                                    .into_iter()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        );
                        plateau_obj.insert(
                            "available_positions".to_string(),
                            serde_json::Value::Array(
                                available_positions
                                    .into_iter()
                                    .map(|pos| serde_json::Value::Number(pos.into()))
                                    .collect(),
                            ),
                        );
                    }
                } else if let Some((cached_images, cached_positions)) = cache.get(&19) {
                    *plateau_data = serde_json::json!({
                        "tiles": vec![[0, 0, 0]; 19],
                        "tile_images": cached_images,
                        "available_positions": cached_positions
                    });
                }
            }
        }
    }

    game_data.to_string()
}
