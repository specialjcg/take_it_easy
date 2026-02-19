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

pub use game_record::PlayerType;
pub use game_recorder::{get_recorder, init_recorder};
