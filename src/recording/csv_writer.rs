//! CSV writer for game recordings.
//!
//! Writes game data in a format compatible with supervised_trainer_csv.rs
//! Format: game_id,turn,player_type,plateau_0-18,tile_0-2,position,final_score,human_won

use crate::recording::game_record::{GameRecord, MoveRecord, PlayerType};
use chrono::Utc;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// CSV writer for game recordings with daily rotation
pub struct CsvWriter {
    base_dir: PathBuf,
    current_file: Option<BufWriter<File>>,
    current_date: String,
}

impl CsvWriter {
    /// Create a new CSV writer
    pub fn new<P: AsRef<Path>>(base_dir: P) -> std::io::Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        fs::create_dir_all(&base_dir)?;

        Ok(Self {
            base_dir,
            current_file: None,
            current_date: String::new(),
        })
    }

    /// Get the current date string for file naming
    fn get_date_string() -> String {
        Utc::now().format("%Y%m%d").to_string()
    }

    /// Get the file path for a given date
    fn get_file_path(&self, date: &str) -> PathBuf {
        self.base_dir.join(format!("games_{}.csv", date))
    }

    /// Ensure the file is open for the current date, with rotation
    fn ensure_file_open(&mut self) -> std::io::Result<()> {
        let today = Self::get_date_string();

        if self.current_date != today || self.current_file.is_none() {
            // Close current file if open
            if let Some(mut file) = self.current_file.take() {
                file.flush()?;
            }

            let file_path = self.get_file_path(&today);
            let file_exists = file_path.exists();

            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)?;

            let mut writer = BufWriter::new(file);

            // Write header if new file
            if !file_exists {
                Self::write_header(&mut writer)?;
            }

            self.current_file = Some(writer);
            self.current_date = today;
        }

        Ok(())
    }

    /// Write the CSV header
    fn write_header<W: Write>(writer: &mut W) -> std::io::Result<()> {
        let mut header = String::from("game_id,turn,player_type");

        // Plateau columns (19 positions)
        for i in 0..19 {
            header.push_str(&format!(",plateau_{}", i));
        }

        // Tile columns
        header.push_str(",tile_0,tile_1,tile_2");

        // Position and scores
        header.push_str(",position,final_score,human_won");

        writeln!(writer, "{}", header)
    }

    /// Write a single move record
    fn write_move<W: Write>(
        writer: &mut W,
        game_id: &str,
        move_record: &MoveRecord,
        final_score: i32,
        human_won: bool,
    ) -> std::io::Result<()> {
        let player_type = match move_record.player_type {
            PlayerType::Human => "Human",
            PlayerType::Mcts => "MCTS",
            PlayerType::Hybrid => "Hybrid",
            PlayerType::Pure => "Pure",
        };

        let mut row = format!("{},{},{}", game_id, move_record.turn, player_type);

        // Plateau state (19 values)
        for encoded in &move_record.plateau_before {
            row.push_str(&format!(",{}", encoded));
        }

        // Tile values
        row.push_str(&format!(
            ",{},{},{}",
            move_record.tile.0, move_record.tile.1, move_record.tile.2
        ));

        // Position and scores
        row.push_str(&format!(
            ",{},{},{}",
            move_record.position,
            final_score,
            if human_won { 1 } else { 0 }
        ));

        writeln!(writer, "{}", row)
    }

    /// Write a complete game record
    pub fn write_game(&mut self, record: &GameRecord) -> std::io::Result<()> {
        self.ensure_file_open()?;

        if let Some(ref mut writer) = self.current_file {
            for move_record in &record.moves {
                // Find the player's final score
                let final_score = record
                    .final_scores
                    .get(&move_record.player_id)
                    .copied()
                    .unwrap_or(0);

                Self::write_move(
                    writer,
                    &record.game_id,
                    move_record,
                    final_score,
                    record.human_won,
                )?;
            }
            writer.flush()?;
        }

        Ok(())
    }

    /// Flush any buffered data
    pub fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut writer) = self.current_file {
            writer.flush()?;
        }
        Ok(())
    }

    /// Close the writer
    pub fn close(&mut self) -> std::io::Result<()> {
        if let Some(mut writer) = self.current_file.take() {
            writer.flush()?;
        }
        Ok(())
    }
}

impl Drop for CsvWriter {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Load recorded games from CSV files
pub fn load_games_from_csv<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<LoadedMoveRecord>, Box<dyn std::error::Error>> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut records = Vec::new();

    for result in reader.records() {
        let record = result?;

        // Parse the CSV row
        let game_id = record.get(0).unwrap_or("").to_string();
        let turn: usize = record.get(1).unwrap_or("0").parse().unwrap_or(0);
        let player_type = PlayerType::from_str(record.get(2).unwrap_or("Human"));

        // Parse plateau (columns 3-21)
        let mut plateau = Vec::with_capacity(19);
        for i in 3..22 {
            let value: i32 = record.get(i).unwrap_or("0").parse().unwrap_or(0);
            plateau.push(value);
        }

        // Parse tile (columns 22-24)
        let tile_0: i32 = record.get(22).unwrap_or("0").parse().unwrap_or(0);
        let tile_1: i32 = record.get(23).unwrap_or("0").parse().unwrap_or(0);
        let tile_2: i32 = record.get(24).unwrap_or("0").parse().unwrap_or(0);

        // Parse position and scores (columns 25-27)
        let position: usize = record.get(25).unwrap_or("0").parse().unwrap_or(0);
        let final_score: i32 = record.get(26).unwrap_or("0").parse().unwrap_or(0);
        let human_won: bool = record.get(27).unwrap_or("0") == "1";

        records.push(LoadedMoveRecord {
            game_id,
            turn,
            player_type,
            plateau,
            tile: (tile_0, tile_1, tile_2),
            position,
            final_score,
            human_won,
        });
    }

    Ok(records)
}

/// A move record loaded from CSV
#[derive(Debug, Clone)]
pub struct LoadedMoveRecord {
    pub game_id: String,
    pub turn: usize,
    pub player_type: PlayerType,
    pub plateau: Vec<i32>,
    pub tile: (i32, i32, i32),
    pub position: usize,
    pub final_score: i32,
    pub human_won: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_csv_writer() -> std::io::Result<()> {
        let dir = tempdir()?;
        let mut writer = CsvWriter::new(dir.path())?;

        let mut record = GameRecord::new("test-1".to_string(), "human_vs_mcts".to_string());
        record.add_player("human".to_string(), PlayerType::Human);

        let move_record = MoveRecord {
            turn: 0,
            player_id: "human".to_string(),
            player_type: PlayerType::Human,
            plateau_before: vec![0; 19],
            tile: (1, 2, 3),
            position: 5,
            mcts_evaluation: None,
            timestamp: 0,
        };
        record.record_move(move_record);

        let mut scores = HashMap::new();
        scores.insert("human".to_string(), 100);
        record.finalize(scores);

        writer.write_game(&record)?;
        writer.close()?;

        // Verify file exists
        let files: Vec<_> = fs::read_dir(dir.path())?
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1);

        Ok(())
    }
}
