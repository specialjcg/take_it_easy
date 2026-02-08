// src/services/game_service/mod.rs - Interface principale du service de jeu modulaire

use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use crate::generated::takeiteasygame::v1::game_service_server::GameService;
use crate::generated::takeiteasygame::v1::*;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::qvalue_net::QValueNet;
use crate::services::session_manager::SessionManager;

// Modules internes
pub mod async_move_handler;
pub mod available_moves;
pub mod mcts_integration;
pub mod move_handler;
pub mod response_builders;
pub mod session_utils;
pub mod state_provider;
pub mod turn_manager;

// Réexports publics pour compatibilité

// ============================================================================
// STRUCTURE PRINCIPALE DU SERVICE
// ============================================================================

#[derive(Clone)]
pub struct GameServiceImpl {
    session_manager: Arc<SessionManager>,
    policy_net: Arc<Mutex<PolicyNet>>,
    value_net: Arc<Mutex<ValueNet>>,
    qvalue_net: Option<Arc<Mutex<QValueNet>>>,
    num_simulations: usize,
    top_k: usize,
}

impl GameServiceImpl {
    pub fn new(
        session_manager: Arc<SessionManager>,
        policy_net: Arc<Mutex<PolicyNet>>,
        value_net: Arc<Mutex<ValueNet>>,
        num_simulations: usize,
    ) -> Self {
        GameServiceImpl {
            session_manager,
            policy_net,
            value_net,
            qvalue_net: None,
            num_simulations,
            top_k: 6,
        }
    }

    /// Create with Q-Net hybrid MCTS for optimal performance
    pub fn new_with_qnet(
        session_manager: Arc<SessionManager>,
        policy_net: Arc<Mutex<PolicyNet>>,
        value_net: Arc<Mutex<ValueNet>>,
        qvalue_net: Option<Arc<Mutex<QValueNet>>>,
        num_simulations: usize,
        top_k: usize,
    ) -> Self {
        GameServiceImpl {
            session_manager,
            policy_net,
            value_net,
            qvalue_net,
            num_simulations,
            top_k,
        }
    }
}

// ============================================================================
// IMPLÉMENTATION GRPC - ORCHESTRATION DES MODULES
// ============================================================================

#[tonic::async_trait]
impl GameService for GameServiceImpl {
    async fn make_move(
        &self,
        request: Request<MakeMoveRequest>,
    ) -> Result<Response<MakeMoveResponse>, Status> {
        let req = request.into_inner();

        // ✅ Utiliser le handler asynchrone avec support Q-Net hybrid
        async_move_handler::make_move_async_logic(
            &self.session_manager,
            &self.policy_net,
            &self.value_net,
            self.qvalue_net.clone(),
            self.num_simulations,
            self.top_k,
            async_move_handler::AsyncMoveRequest {
                session_id: req.session_id,
                player_id: req.player_id,
                move_data: req.move_data,
                timestamp: req.timestamp,
            },
        )
        .await
    }

    async fn get_available_moves(
        &self,
        request: Request<GetAvailableMovesRequest>,
    ) -> Result<Response<GetAvailableMovesResponse>, Status> {
        let req = request.into_inner();
        available_moves::get_available_moves_logic(
            &self.session_manager,
            req.session_id,
            req.player_id,
        )
        .await
    }

    async fn start_turn(
        &self,
        request: Request<StartTurnRequest>,
    ) -> Result<Response<StartTurnResponse>, Status> {
        let req = request.into_inner();
        turn_manager::start_turn_logic(
            &self.session_manager,
            &self.policy_net,
            &self.value_net,
            self.qvalue_net.clone(),
            self.num_simulations,
            self.top_k,
            req.session_id,
        )
        .await
    }

    async fn get_game_state(
        &self,
        request: Request<GetGameStateRequest>,
    ) -> Result<Response<GetGameStateResponse>, Status> {
        let req = request.into_inner();
        state_provider::get_game_state_logic(&self.session_manager, req.session_id).await
    }

    /// Mode Jeu Réel: obtenir la recommandation IA pour une tuile donnée
    async fn get_ai_move(
        &self,
        request: Request<GetAiMoveRequest>,
    ) -> Result<Response<GetAiMoveResponse>, Status> {
        let req = request.into_inner();

        // Importer les modules nécessaires
        use crate::game::create_deck::create_deck;
        use crate::game::plateau::create_plateau_empty;
        use crate::game::remove_tile_from_deck::replace_tile_in_deck;
        use crate::game::tile::Tile;
        use crate::neural::tensor_conversion::convert_plateau_for_gat_47ch;
        use crate::neural::manager::NNArchitecture;

        // Parser le code de tuile (ex: "168" -> Tile)
        let _tile = match parse_tile_code(&req.tile_code) {
            Some(t) => t,
            None => {
                return Ok(Response::new(GetAiMoveResponse {
                    success: false,
                    recommended_position: -1,
                    error: Some(Error {
                        code: "INVALID_TILE".to_string(),
                        message: format!("Invalid tile code: {}", req.tile_code),
                        details: Default::default(),
                    }),
                }));
            }
        };

        // Créer le plateau à partir de l'état envoyé
        // Tile(0, 0, 0) représente une case vide
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        for (i, tile_str) in req.board_state.iter().enumerate() {
            if i < 19 && !tile_str.is_empty() {
                if let Some(t) = parse_tile_code(tile_str) {
                    plateau.tiles[i] = t;
                    // Remove placed tile from deck
                    deck = replace_tile_in_deck(&deck, &t);
                }
            }
        }
        // Also remove the current tile from deck
        deck = replace_tile_in_deck(&deck, &_tile);

        // Obtenir les positions disponibles (où Tile(0,0,0) = vide)
        let available: Vec<usize> = if req.available_positions.is_empty() {
            plateau.tiles.iter().enumerate()
                .filter(|(_, t)| t.0 == 0 && t.1 == 0 && t.2 == 0)
                .map(|(i, _)| i)
                .collect()
        } else {
            req.available_positions.iter().map(|&p| p as usize).collect()
        };

        if available.is_empty() {
            return Ok(Response::new(GetAiMoveResponse {
                success: false,
                recommended_position: -1,
                error: Some(Error {
                    code: "NO_POSITIONS".to_string(),
                    message: "No available positions".to_string(),
                    details: Default::default(),
                }),
            }));
        }

        // Utiliser le PolicyNet pour obtenir la meilleure position
        let policy_net = self.policy_net.lock().await;
        let arch = policy_net.arch;

        let current_turn = req.turn_number as usize;
        let total_turns = 19;

        // Convertir le plateau en tensor (47 features pour Graph Transformer)
        let input_tensor = match arch {
            NNArchitecture::GraphTransformer | NNArchitecture::Gnn => {
                convert_plateau_for_gat_47ch(&plateau, &_tile, &deck, current_turn, total_turns)
            }
            _ => {
                // Pour CNN, on utilise aussi la version 47ch
                convert_plateau_for_gat_47ch(&plateau, &_tile, &deck, current_turn, total_turns)
            }
        };

        // Forward pass
        let policy_output = policy_net.forward(&input_tensor, false);

        // Extraire les probabilités pour les positions disponibles
        let policy_vec: Vec<f32> = {
            let flat = policy_output.flatten(0, -1);
            let size = flat.size()[0] as usize;
            let mut buf = vec![0f32; size];
            flat.copy_data(&mut buf, size);
            buf
        };

        // Trouver la meilleure position parmi les disponibles
        let best_position: usize = available.iter()
            .filter(|&&pos| pos < policy_vec.len())
            .max_by(|&a, &b| {
                let val_a: f32 = policy_vec[*a];
                let val_b: f32 = policy_vec[*b];
                val_a.partial_cmp(&val_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
            .unwrap_or(available[0]);

        log::info!(
            "🎲 AI Move: tile={} position={} (from {} available)",
            req.tile_code, best_position, available.len()
        );

        Ok(Response::new(GetAiMoveResponse {
            success: true,
            recommended_position: best_position as i32,
            error: None,
        }))
    }
}

/// Parse un code de tuile (ex: "168") en Tile
fn parse_tile_code(code: &str) -> Option<crate::game::tile::Tile> {
    use crate::game::tile::Tile;

    // Nettoyer le code (enlever "image/" et ".png" si présent)
    let clean_code = code
        .replace("image/", "")
        .replace(".png", "")
        .replace("../", "");

    if clean_code.len() != 3 {
        return None;
    }

    let chars: Vec<char> = clean_code.chars().collect();
    let v1 = chars[0].to_digit(10)? as i32;
    let v2 = chars[1].to_digit(10)? as i32;
    let v3 = chars[2].to_digit(10)? as i32;

    // Valider les valeurs
    if ![1, 5, 9].contains(&v1) || ![2, 6, 7].contains(&v2) || ![3, 4, 8].contains(&v3) {
        return None;
    }

    Some(Tile(v1, v2, v3))
}
