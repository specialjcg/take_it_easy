//! Game recording data structures for training data collection.
//!
//! This module defines the structures used to record human vs AI games
//! for later use in training improved models.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of player in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerType {
    Human,
    Mcts,
    Hybrid,
    Pure,
}

impl std::fmt::Display for PlayerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerType::Human => write!(f, "Human"),
            PlayerType::Mcts => write!(f, "MCTS"),
            PlayerType::Hybrid => write!(f, "Hybrid"),
            PlayerType::Pure => write!(f, "Pure"),
        }
    }
}

impl PlayerType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "human" => PlayerType::Human,
            "mcts" => PlayerType::Mcts,
            "hybrid" => PlayerType::Hybrid,
            "pure" => PlayerType::Pure,
            _ => PlayerType::Human,
        }
    }
}

/// Record of a single player in the game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerRecord {
    pub player_id: String,
    pub player_type: PlayerType,
    pub final_score: i32,
}

/// Record of a single move in the game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRecord {
    /// Turn number (0-18)
    pub turn: usize,
    /// Player who made the move
    pub player_id: String,
    /// Type of player
    pub player_type: PlayerType,
    /// Plateau state before the move (19 positions encoded)
    pub plateau_before: Vec<i32>,
    /// The tile placed (3 values)
    pub tile: (i32, i32, i32),
    /// Position where tile was placed (0-18)
    pub position: usize,
    /// MCTS evaluation score if available
    pub mcts_evaluation: Option<f32>,
    /// Timestamp of the move
    pub timestamp: i64,
}

/// Complete record of a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRecord {
    /// Unique game identifier
    pub game_id: String,
    /// Game start timestamp
    pub timestamp: i64,
    /// Players in the game
    pub players: Vec<PlayerRecord>,
    /// All moves made in the game
    pub moves: Vec<MoveRecord>,
    /// Final scores for each player
    pub final_scores: HashMap<String, i32>,
    /// Whether a human won the game
    pub human_won: bool,
    /// Game mode (e.g., "human_vs_mcts", "ai_vs_ai")
    pub game_mode: String,
}

impl GameRecord {
    /// Create a new empty game record
    pub fn new(game_id: String, game_mode: String) -> Self {
        Self {
            game_id,
            timestamp: chrono::Utc::now().timestamp(),
            players: Vec::new(),
            moves: Vec::new(),
            final_scores: HashMap::new(),
            human_won: false,
            game_mode,
        }
    }

    /// Add a player to the game record
    pub fn add_player(&mut self, player_id: String, player_type: PlayerType) {
        self.players.push(PlayerRecord {
            player_id,
            player_type,
            final_score: 0,
        });
    }

    /// Record a move
    pub fn record_move(&mut self, move_record: MoveRecord) {
        self.moves.push(move_record);
    }

    /// Finalize the game with final scores
    pub fn finalize(&mut self, scores: HashMap<String, i32>) {
        self.final_scores = scores.clone();

        // Update player records with final scores
        for player in &mut self.players {
            if let Some(&score) = scores.get(&player.player_id) {
                player.final_score = score;
            }
        }

        // Determine if human won
        let mut human_score = i32::MIN;
        let mut max_ai_score = i32::MIN;

        for player in &self.players {
            match player.player_type {
                PlayerType::Human => {
                    if player.final_score > human_score {
                        human_score = player.final_score;
                    }
                }
                _ => {
                    if player.final_score > max_ai_score {
                        max_ai_score = player.final_score;
                    }
                }
            }
        }

        self.human_won = human_score > max_ai_score;
    }

    /// Get the human player's final score (if any)
    pub fn human_score(&self) -> Option<i32> {
        self.players
            .iter()
            .find(|p| p.player_type == PlayerType::Human)
            .map(|p| p.final_score)
    }

    /// Get the best AI score
    pub fn best_ai_score(&self) -> Option<i32> {
        self.players
            .iter()
            .filter(|p| p.player_type != PlayerType::Human)
            .map(|p| p.final_score)
            .max()
    }
}

/// Encodes a plateau state to a flat vector of i32
/// Each tile is encoded as a single value: tile.0 * 100 + tile.1 * 10 + tile.2
pub fn encode_plateau(tiles: &[crate::game::tile::Tile]) -> Vec<i32> {
    tiles
        .iter()
        .map(|t| t.0 * 100 + t.1 * 10 + t.2)
        .collect()
}

/// Decodes a flat vector back to tile values
pub fn decode_plateau_value(encoded: i32) -> (i32, i32, i32) {
    let v0 = encoded / 100;
    let v1 = (encoded % 100) / 10;
    let v2 = encoded % 10;
    (v0, v1, v2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_plateau() {
        let encoded = 123;
        let decoded = decode_plateau_value(encoded);
        assert_eq!(decoded, (1, 2, 3));
    }

    #[test]
    fn test_game_record_finalize() {
        let mut record = GameRecord::new("test-game".to_string(), "human_vs_mcts".to_string());
        record.add_player("human1".to_string(), PlayerType::Human);
        record.add_player("mcts1".to_string(), PlayerType::Mcts);

        let mut scores = HashMap::new();
        scores.insert("human1".to_string(), 150);
        scores.insert("mcts1".to_string(), 120);

        record.finalize(scores);

        assert!(record.human_won);
        assert_eq!(record.human_score(), Some(150));
        assert_eq!(record.best_ai_score(), Some(120));
    }
}
