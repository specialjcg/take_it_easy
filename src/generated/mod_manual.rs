// src/generated/mod_manual.rs - Types temporaires avant génération complète

pub mod takeiteasygame {
    pub mod v1 {
        use std::collections::HashMap;
        use serde::{Deserialize, Serialize};
        use tonic::{Request, Response, Status};

        // ============================================================================
        // TYPES DE BASE - COMPATIBLES AVEC VOTRE SYSTÈME
        // ============================================================================

        #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
        pub struct Player {
            pub id: String,
            pub name: String,
            pub score: i32,
            pub is_ready: bool,
            pub is_connected: bool,
            pub joined_at: i64,
        }

        #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
        pub struct GameState {
            pub session_id: String,
            pub players: Vec<Player>,
            pub current_player_id: String,
            pub state: i32, // 0=WAITING, 1=IN_PROGRESS, 2=FINISHED
            pub board_state: String, // JSON de TakeItEasyGameState
            pub turn_number: i32,
        }

        #[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
        pub struct Error {
            pub code: String,
            pub message: String,
            pub details: HashMap<String, String>,
        }

        // ============================================================================
        // MESSAGES DE SESSION
        // ============================================================================

        #[derive(Clone, PartialEq, Debug)]
        pub struct CreateSessionRequest {
            pub player_name: String,
            pub max_players: i32,
            pub game_mode: String,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct CreateSessionSuccess {
            pub session_code: String,
            pub session_id: String,
            pub player_id: String,
            pub player: Option<Player>,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct CreateSessionResponse {
            pub result: Option<create_session_response::Result>,
        }

        pub mod create_session_response {
            use super::*;

            #[derive(Clone, PartialEq, Debug)]
            pub enum Result {
                Success(CreateSessionSuccess),
                Error(Error),
            }
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct JoinSessionRequest {
            pub session_code: String,
            pub player_name: String,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct JoinSessionSuccess {
            pub session_id: String,
            pub player_id: String,
            pub game_state: Option<GameState>,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct JoinSessionResponse {
            pub result: Option<join_session_response::Result>,
        }

        pub mod join_session_response {
            use super::*;

            #[derive(Clone, PartialEq, Debug)]
            pub enum Result {
                Success(JoinSessionSuccess),
                Error(Error),
            }
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct SetReadyRequest {
            pub session_id: String,
            pub player_id: String,
            pub ready: bool,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct SetReadyResponse {
            pub success: bool,
            pub error: Option<Error>,
            pub game_started: bool,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct GetSessionStateRequest {
            pub session_id: String,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct GetSessionStateResponse {
            pub game_state: Option<GameState>,
            pub error: Option<Error>,
        }

        // ============================================================================
        // MESSAGES DE JEU
        // ============================================================================

        #[derive(Clone, PartialEq, Debug)]
        pub struct MakeMoveRequest {
            pub session_id: String,
            pub player_id: String,
            pub move_data: String,
            pub timestamp: i64,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct MakeMoveSuccess {
            pub new_game_state: Option<GameState>,
            pub mcts_response: String,
            pub points_earned: i32,
            pub is_game_over: bool,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct MakeMoveResponse {
            pub result: Option<make_move_response::Result>,
        }

        pub mod make_move_response {
            use super::*;

            #[derive(Clone, PartialEq, Debug)]
            pub enum Result {
                Success(MakeMoveSuccess),
                Error(Error),
            }
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct GetAvailableMovesRequest {
            pub session_id: String,
            pub player_id: String,
        }

        #[derive(Clone, PartialEq, Debug)]
        pub struct GetAvailableMovesResponse {
            pub available_moves: Vec<String>,
            pub error: Option<Error>,
        }

        // ============================================================================
        // TRAITS DE SERVICE - DÉFINITIONS MINIMALES
        // ============================================================================

        #[tonic::async_trait]
        pub trait SessionService: Send + Sync + 'static {
            async fn create_session(
                &self,
                request: Request<CreateSessionRequest>,
            ) -> Result<Response<CreateSessionResponse>, Status>;

            async fn join_session(
                &self,
                request: Request<JoinSessionRequest>,
            ) -> Result<Response<JoinSessionResponse>, Status>;

            async fn set_ready(
                &self,
                request: Request<SetReadyRequest>,
            ) -> Result<Response<SetReadyResponse>, Status>;

            async fn get_session_state(
                &self,
                request: Request<GetSessionStateRequest>,
            ) -> Result<Response<GetSessionStateResponse>, Status>;
        }

        #[tonic::async_trait]
        pub trait GameService: Send + Sync + 'static {
            async fn make_move(
                &self,
                request: Request<MakeMoveRequest>,
            ) -> Result<Response<MakeMoveResponse>, Status>;

            async fn get_available_moves(
                &self,
                request: Request<GetAvailableMovesRequest>,
            ) -> Result<Response<GetAvailableMovesResponse>, Status>;
        }

        // ============================================================================
        // SERVEURS GRPC - IMPLÉMENTATION TEMPORAIRE SIMPLIFIÉE
        // ============================================================================

        pub mod session_service_server {
            use super::*;
            use std::sync::Arc;

            #[derive(Clone)]
            pub struct SessionServiceServer<T> {
                inner: Arc<T>,
            }

            impl<T> SessionServiceServer<T> {
                pub fn new(inner: T) -> Self {
                    Self {
                        inner: Arc::new(inner),
                    }
                }
            }

            // Implémentation minimale pour permettre la compilation
            // Sera remplacée par tonic-build
            use tonic::server::{NamedService, UnaryService};
            use tonic::codegen::*;

            impl<T: SessionService> NamedService for SessionServiceServer<T> {
                const NAME: &'static str = "takeiteasygame.v1.SessionService";
            }

            // Service trait sera implémenté automatiquement par tonic-build
        }

        pub mod game_service_server {
            use super::*;
            use std::sync::Arc;

            #[derive(Clone)]
            pub struct GameServiceServer<T> {
                inner: Arc<T>,
            }

            impl<T> GameServiceServer<T> {
                pub fn new(inner: T) -> Self {
                    Self {
                        inner: Arc::new(inner),
                    }
                }
            }

            use tonic::server::NamedService;

            impl<T: GameService> NamedService for GameServiceServer<T> {
                const NAME: &'static str = "takeiteasygame.v1.GameService";
            }
        }
    }
}