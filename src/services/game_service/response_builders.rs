// src/services/game_service/response_builders.rs - Constructeurs de réponses gRPC

use crate::generated::takeiteasygame::v1::*;
use crate::services::game_manager::{
    mcts_move_to_json, take_it_easy_state_to_protobuf, MoveResult,
};

// ============================================================================
// CONSTRUCTEURS DE RÉPONSES - FONCTIONS PURES
// ============================================================================

pub fn make_move_success_response(move_result: MoveResult, game_mode: &str) -> MakeMoveResponse {
    let mcts_response_json = move_result
        .mcts_response
        .as_ref()
        .and_then(|mcts| mcts_move_to_json(mcts).ok())
        .unwrap_or_default();

    MakeMoveResponse {
        result: Some(make_move_response::Result::Success(MakeMoveSuccess {
            new_game_state: Some(take_it_easy_state_to_protobuf(
                &move_result.new_game_state,
                game_mode,
            )),
            mcts_response: mcts_response_json,
            points_earned: move_result.points_earned,
            is_game_over: move_result.is_game_over,
        })),
    }
}

pub fn make_move_error_response(code: String, message: String) -> MakeMoveResponse {
    MakeMoveResponse {
        result: Some(make_move_response::Result::Error(Error {
            code,
            message,
            details: std::collections::HashMap::new(),
        })),
    }
}

pub fn available_moves_success_response(
    positions: Vec<usize>,
    current_tile: Option<crate::game::tile::Tile>,
) -> GetAvailableMovesResponse {
    let moves_json: Vec<String> = positions
        .iter()
        .map(|pos| {
            serde_json::json!({
                "position": pos,
                "tile": current_tile.map(|t| (t.0, t.1, t.2))
            })
            .to_string()
        })
        .collect();

    GetAvailableMovesResponse {
        available_moves: moves_json,
        error: None,
    }
}

pub fn available_moves_error_response(code: String, message: String) -> GetAvailableMovesResponse {
    GetAvailableMovesResponse {
        available_moves: vec![],
        error: Some(Error {
            code,
            message,
            details: std::collections::HashMap::new(),
        }),
    }
}

pub fn start_turn_success_response(
    announced_tile: String,
    tile_image: String,
    turn_number: i32,
    waiting_for_players: Vec<String>,
    game_state_json: String,
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

pub fn start_turn_error_response(message: String) -> StartTurnResponse {
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

pub fn game_state_success_response(
    game_state_json: String,
    current_tile: String,
    current_tile_image: String,
    current_turn: i32,
    waiting_for_players: Vec<String>,
    is_game_finished: bool,
    final_scores_json: String,
) -> GetGameStateResponse {
    GetGameStateResponse {
        success: true,
        game_state: game_state_json,
        current_tile,
        current_tile_image,
        current_turn,
        waiting_for_players,
        is_game_finished,
        final_scores: final_scores_json,
        error: None,
    }
}

pub fn game_state_error_response(message: String) -> GetGameStateResponse {
    GetGameStateResponse {
        success: false,
        game_state: String::new(),
        current_tile: String::new(),
        current_tile_image: String::new(),
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
