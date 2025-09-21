// src/services/game_service/mod.rs - Interface principale du service de jeu modulaire

use tonic::{Request, Response, Status};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::generated::takeiteasygame::v1::*;
use crate::generated::takeiteasygame::v1::game_service_server::GameService;
use crate::services::session_manager::SessionManager;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};

// Modules internes
pub mod response_builders;
pub mod session_utils;
pub mod mcts_integration;
pub mod move_handler;
pub mod async_move_handler;
pub mod available_moves;
pub mod turn_manager;
pub mod state_provider;

// Réexports publics pour compatibilité

// ============================================================================
// STRUCTURE PRINCIPALE DU SERVICE
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
// IMPLÉMENTATION GRPC - ORCHESTRATION DES MODULES
// ============================================================================

#[tonic::async_trait]
impl GameService for GameServiceImpl {
    async fn make_move(
        &self,
        request: Request<MakeMoveRequest>,
    ) -> Result<Response<MakeMoveResponse>, Status> {
        let req = request.into_inner();

        // ✅ NOUVEAU: Utiliser le handler asynchrone pour feedback immédiat
        async_move_handler::make_move_async_logic(
            &self.session_manager,
            &self.policy_net,
            &self.value_net,
            self.num_simulations,
            async_move_handler::AsyncMoveRequest {
                session_id: req.session_id,
                player_id: req.player_id,
                move_data: req.move_data,
                timestamp: req.timestamp,
            },
        ).await
    }

    async fn get_available_moves(
        &self,
        request: Request<GetAvailableMovesRequest>,
    ) -> Result<Response<GetAvailableMovesResponse>, Status> {
        let req = request.into_inner();
        available_moves::get_available_moves_logic(
            &self.session_manager,
            req.session_id,
            req.player_id
        ).await
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
            self.num_simulations,
            req.session_id
        ).await
    }

    async fn get_game_state(
        &self,
        request: Request<GetGameStateRequest>,
    ) -> Result<Response<GetGameStateResponse>, Status> {
        let req = request.into_inner();
        state_provider::get_game_state_logic(
            &self.session_manager,
            req.session_id
        ).await
    }
}