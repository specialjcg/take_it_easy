//! Game recording module for training data collection.
//!
//! This module provides functionality to record human vs AI games
//! for use in training improved models.
//!
//! # Components
//!
//! - `game_record`: Data structures for game records
//! - `game_recorder`: Thread-safe game recording service
//! - `csv_writer`: CSV output for training data

pub mod csv_writer;
pub mod game_record;
pub mod game_recorder;

pub use csv_writer::{load_games_from_csv, CsvWriter, LoadedMoveRecord};
pub use game_record::{
    decode_plateau_value, encode_plateau, GameRecord, MoveRecord, PlayerRecord, PlayerType,
};
pub use game_recorder::{
    determine_player_type, get_recorder, init_recorder, GameRecorder,
};
