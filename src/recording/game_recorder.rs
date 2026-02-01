//! Game recorder for capturing human vs AI games.
//!
//! This module provides thread-safe game recording capabilities
//! for collecting training data from human play.

use crate::game::plateau::Plateau;
use crate::game::tile::Tile;
use crate::recording::csv_writer::CsvWriter;
use crate::recording::game_record::{encode_plateau, GameRecord, MoveRecord, PlayerType};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Thread-safe game recorder
pub struct GameRecorder {
    /// Active game records, indexed by session_id
    active_games: Mutex<HashMap<String, GameRecord>>,
    /// CSV writer for persisting completed games
    csv_writer: Mutex<CsvWriter>,
    /// Whether recording is enabled
    enabled: bool,
}

impl GameRecorder {
    /// Create a new game recorder
    pub fn new<P: AsRef<Path>>(output_dir: P) -> std::io::Result<Self> {
        let csv_writer = CsvWriter::new(output_dir)?;
        Ok(Self {
            active_games: Mutex::new(HashMap::new()),
            csv_writer: Mutex::new(csv_writer),
            enabled: true,
        })
    }

    /// Create a disabled recorder (for testing or when recording is not needed)
    pub fn disabled() -> Self {
        Self {
            active_games: Mutex::new(HashMap::new()),
            csv_writer: Mutex::new(CsvWriter::new("/dev/null").unwrap()),
            enabled: false,
        }
    }

    /// Check if recording is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Start recording a new game
    pub fn start_game(
        &self,
        session_id: &str,
        game_mode: &str,
        players: Vec<(String, PlayerType)>,
    ) {
        if !self.enabled {
            return;
        }

        let mut record = GameRecord::new(session_id.to_string(), game_mode.to_string());
        for (player_id, player_type) in players {
            record.add_player(player_id, player_type);
        }

        let mut games = self.active_games.lock().unwrap();
        games.insert(session_id.to_string(), record);

        log::info!("Started recording game: {}", session_id);
    }

    /// Record a move in an active game
    pub fn record_move(
        &self,
        session_id: &str,
        turn: usize,
        player_id: &str,
        player_type: PlayerType,
        plateau: &Plateau,
        tile: &Tile,
        position: usize,
        mcts_evaluation: Option<f32>,
    ) {
        if !self.enabled {
            return;
        }

        let move_record = MoveRecord {
            turn,
            player_id: player_id.to_string(),
            player_type,
            plateau_before: encode_plateau(&plateau.tiles),
            tile: (tile.0, tile.1, tile.2),
            position,
            mcts_evaluation,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let mut games = self.active_games.lock().unwrap();
        if let Some(record) = games.get_mut(session_id) {
            record.record_move(move_record);
            log::debug!(
                "Recorded move for game {}: turn={}, player={}, pos={}",
                session_id,
                turn,
                player_id,
                position
            );
        } else {
            log::warn!("Attempted to record move for unknown game: {}", session_id);
        }
    }

    /// Finalize and save a completed game
    pub fn finalize_game(
        &self,
        session_id: &str,
        final_scores: HashMap<String, i32>,
    ) -> std::io::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let record = {
            let mut games = self.active_games.lock().unwrap();
            games.remove(session_id)
        };

        if let Some(mut record) = record {
            record.finalize(final_scores);

            let human_won = record.human_won;
            let human_score = record.human_score();
            let ai_score = record.best_ai_score();

            {
                let mut writer = self.csv_writer.lock().unwrap();
                writer.write_game(&record)?;
            }

            log::info!(
                "Finalized game {}: human_won={}, human_score={:?}, ai_score={:?}",
                session_id,
                human_won,
                human_score,
                ai_score
            );
        } else {
            log::warn!("Attempted to finalize unknown game: {}", session_id);
        }

        Ok(())
    }

    /// Cancel a game without saving
    pub fn cancel_game(&self, session_id: &str) {
        if !self.enabled {
            return;
        }

        let mut games = self.active_games.lock().unwrap();
        if games.remove(session_id).is_some() {
            log::info!("Cancelled recording for game: {}", session_id);
        }
    }

    /// Get the number of active games being recorded
    pub fn active_game_count(&self) -> usize {
        self.active_games.lock().unwrap().len()
    }

    /// Flush any pending writes
    pub fn flush(&self) -> std::io::Result<()> {
        if self.enabled {
            let mut writer = self.csv_writer.lock().unwrap();
            writer.flush()?;
        }
        Ok(())
    }
}

/// Global singleton for the game recorder
static GAME_RECORDER: std::sync::OnceLock<Arc<GameRecorder>> = std::sync::OnceLock::new();

/// Initialize the global game recorder
pub fn init_recorder<P: AsRef<Path>>(output_dir: P) -> std::io::Result<()> {
    let recorder = GameRecorder::new(output_dir)?;
    GAME_RECORDER
        .set(Arc::new(recorder))
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Recorder already initialized"))
}

/// Get the global game recorder
pub fn get_recorder() -> Option<Arc<GameRecorder>> {
    GAME_RECORDER.get().cloned()
}

/// Helper to determine player type from player ID or configuration
pub fn determine_player_type(player_id: &str, is_mcts: bool, is_hybrid: bool) -> PlayerType {
    if is_hybrid {
        PlayerType::Hybrid
    } else if is_mcts || player_id.to_lowercase().contains("mcts") {
        PlayerType::Mcts
    } else if player_id.to_lowercase().contains("ai") || player_id.to_lowercase().contains("bot") {
        PlayerType::Mcts
    } else {
        PlayerType::Human
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::plateau::create_plateau_empty;
    use tempfile::tempdir;

    #[test]
    fn test_game_recorder() -> std::io::Result<()> {
        let dir = tempdir()?;
        let recorder = GameRecorder::new(dir.path())?;

        // Start a game
        recorder.start_game(
            "test-session",
            "human_vs_mcts",
            vec![
                ("human".to_string(), PlayerType::Human),
                ("mcts".to_string(), PlayerType::Mcts),
            ],
        );

        // Record some moves
        let plateau = create_plateau_empty();
        let tile = Tile(1, 2, 3);

        recorder.record_move(
            "test-session",
            0,
            "human",
            PlayerType::Human,
            &plateau,
            &tile,
            5,
            None,
        );

        recorder.record_move(
            "test-session",
            0,
            "mcts",
            PlayerType::Mcts,
            &plateau,
            &tile,
            10,
            Some(0.85),
        );

        // Finalize
        let mut scores = HashMap::new();
        scores.insert("human".to_string(), 150);
        scores.insert("mcts".to_string(), 120);

        recorder.finalize_game("test-session", scores)?;

        assert_eq!(recorder.active_game_count(), 0);

        Ok(())
    }

    #[test]
    fn test_disabled_recorder() {
        let recorder = GameRecorder::disabled();
        assert!(!recorder.is_enabled());

        // These should not panic even though disabled
        recorder.start_game(
            "test",
            "test",
            vec![("player".to_string(), PlayerType::Human)],
        );
        recorder.cancel_game("test");
    }
}
