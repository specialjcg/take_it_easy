//! Generate strategic self-play games with v1-row-priority bias.
//!
//! Strategy: GT + line_boost + v1_row_priority
//!   - v1=9 tiles → center row (positions 7-11, len 5) → +bonus
//!   - v1=5 tiles → side rows (positions 3-6, 12-15, len 4) → +bonus
//!   - v1=1 tiles → edge rows (positions 0-2, 16-18, len 3) → +bonus
//!   - Mismatches (v1=9 on edge, v1=1 on center) → penalty
//!
//! Filters games by minimum score and deduplicates by final board state.
//! Output format is identical to selfplay_train CSV.
//!
//! Usage:
//!   cargo build --release --bin generate_v1_strategic --target-dir target2
//!   ./target2/release/generate_v1_strategic --num-games 50000 --min-score 170

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tch::{nn, Device, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::deck::Deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::load_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::gt_boost::line_boost;

// ============================================================
// CLI
// ============================================================

#[derive(Parser)]
#[command(
    name = "generate_v1_strategic",
    about = "Generate high-score games with v1-row-priority strategy for GT training"
)]
struct Cli {
    #[arg(long, default_value_t = 50000)]
    num_games: usize,

    #[arg(long, default_value_t = 170)]
    min_score: i32,

    #[arg(long, default_value_t = 8.0)]
    v1_bonus: f64,

    #[arg(long, default_value_t = 3.0)]
    line_boost: f64,

    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    model_path: String,

    #[arg(long, default_value = "data/v1_strategic.csv")]
    output: String,

    #[arg(long)]
    seed: Option<u64>,
}

// ============================================================
// Helpers (same encoding as selfplay_train)
// ============================================================

struct TurnRecord {
    game_idx: usize,
    turn: usize,
    plateau: [i32; 19],
    tile: (i32, i32, i32),
    chosen_position: usize,
    final_score: i32,
}

fn encode_tile(t: &Tile) -> i32 {
    if *t == Tile(0, 0, 0) {
        0
    } else {
        t.0 * 100 + t.1 * 10 + t.2
    }
}

fn generate_tile_sequence(rng: &mut StdRng) -> Vec<Tile> {
    let mut deck = create_deck();
    let mut tiles = Vec::with_capacity(19);
    for _ in 0..19 {
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }
        let tile = *available.choose(rng).unwrap();
        tiles.push(tile);
        deck = replace_tile_in_deck(&deck, &tile);
    }
    tiles
}

fn encode_board(plateau: &Plateau) -> [i32; 19] {
    let mut encoded = [0i32; 19];
    for i in 0..19 {
        encoded[i] = encode_tile(&plateau.tiles[i]);
    }
    encoded
}

fn board_key(encoded: &[i32; 19]) -> String {
    encoded
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

// ============================================================
// v1-row priority (adaptive)
// ============================================================

/// Row definitions: positions belonging to each horizontal row.
const ROWS: [&[usize]; 5] = [
    &[0, 1, 2],        // row 0: edge (len 3) — target v1=1
    &[3, 4, 5, 6],     // row 1: side (len 4) — target v1=5
    &[7, 8, 9, 10, 11],// row 2: center (len 5) — target v1=9
    &[12, 13, 14, 15],  // row 3: side (len 4) — target v1=5
    &[16, 17, 18],      // row 4: edge (len 3) — target v1=1
];

/// Target v1 value for each row.
const ROW_TARGET_V1: [i32; 5] = [1, 5, 9, 5, 1];

fn pos_to_row(pos: usize) -> usize {
    match pos {
        0..=2 => 0,
        3..=6 => 1,
        7..=11 => 2,
        12..=15 => 3,
        16..=18 => 4,
        _ => unreachable!(),
    }
}

/// Check if a row is still viable for v1-homogeneity with the target value.
/// A row is viable if no tile with a DIFFERENT v1 has been placed in it.
fn row_viable(plateau: &Plateau, row_idx: usize) -> bool {
    let target = ROW_TARGET_V1[row_idx];
    for &pos in ROWS[row_idx] {
        let t = &plateau.tiles[pos];
        if *t != Tile(0, 0, 0) && t.0 != target {
            return false; // a conflicting v1 is already there
        }
    }
    true
}

/// Count how many positions in the row already have the correct v1.
fn row_progress(plateau: &Plateau, row_idx: usize) -> usize {
    let target = ROW_TARGET_V1[row_idx];
    ROWS[row_idx]
        .iter()
        .filter(|&&pos| {
            let t = &plateau.tiles[pos];
            *t != Tile(0, 0, 0) && t.0 == target
        })
        .count()
}

/// Adaptive v1-row priority.
///
/// Only applies bonus when:
///   1. The tile's v1 matches the row's target v1
///   2. The row is still viable (no conflicting v1 placed)
///
/// Bonus scales with progress: more tiles already matching → stronger signal.
/// Penalty for placing a mismatched v1 in a viable row (would ruin it).
fn v1_row_priority_adaptive(plateau: &Plateau, tile: &Tile, position: usize, bonus: f64) -> f64 {
    let row_idx = pos_to_row(position);
    let target_v1 = ROW_TARGET_V1[row_idx];
    let viable = row_viable(plateau, row_idx);

    if tile.0 == target_v1 && viable {
        // Correct v1 in a viable row — bonus scales with existing progress
        let progress = row_progress(plateau, row_idx);
        let row_len = ROWS[row_idx].len();
        // Base bonus + extra for rows with tiles already matching
        let scale = if progress == 0 {
            0.3 // light nudge for first tile
        } else {
            0.3 + 0.7 * (progress as f64 / (row_len - 1) as f64)
        };
        bonus * scale
    } else if tile.0 != target_v1 && viable && row_progress(plateau, row_idx) >= 1 {
        // Wrong v1 in a viable row that already has progress — penalty
        let progress = row_progress(plateau, row_idx);
        let row_len = ROWS[row_idx].len();
        let potential = target_v1 as f64 * row_len as f64 / 45.0;
        -bonus * 0.4 * (progress as f64 / (row_len - 1) as f64) * potential
    } else {
        0.0 // row already broken or no signal
    }
}

// ============================================================
// GT inference
// ============================================================

fn compute_masked_logits(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
) -> Tensor {
    let feat = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19).unsqueeze(0);
    let logits = policy_net.forward(&feat, false).squeeze_dim(0);
    let mut mask = [0.0f32; 19];
    for i in 0..19 {
        if plateau.tiles[i] != Tile(0, 0, 0) {
            mask[i] = f32::NEG_INFINITY;
        }
    }
    logits + Tensor::from_slice(&mask)
}

// ============================================================
// Play one game with GT + line_boost + v1_priority
// ============================================================

fn play_game_v1_strategic(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    lb: f64,
    v1_bonus: f64,
) -> (Plateau, Vec<(usize, [i32; 19], (i32, i32, i32), usize)>) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut turns = Vec::with_capacity(19);

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        // Record state BEFORE placement
        let encoded = encode_board(&plateau);

        let pos = if legal.len() == 1 {
            legal[0]
        } else {
            // GT logits
            let masked = compute_masked_logits(&plateau, tile, &deck, turn, policy_net);
            let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

            // argmax over (GT_logit + line_boost + v1_priority_adaptive)
            *legal
                .iter()
                .max_by(|&&a, &&b| {
                    let sa = logit_values[a]
                        + line_boost(&plateau, tile, a, lb)
                        + v1_row_priority_adaptive(&plateau, tile, a, v1_bonus);
                    let sb = logit_values[b]
                        + line_boost(&plateau, tile, b, lb)
                        + v1_row_priority_adaptive(&plateau, tile, b, v1_bonus);
                    sa.partial_cmp(&sb).unwrap()
                })
                .unwrap()
        };

        turns.push((turn, encoded, (tile.0, tile.1, tile.2), pos));
        plateau.tiles[pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    (plateau, turns)
}

// ============================================================
// CSV output (identical format to selfplay_train)
// ============================================================

fn save_csv(records: &[TurnRecord], path: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    let mut wtr = csv::Writer::from_path(path)?;
    let mut header: Vec<String> = vec!["game_idx".into(), "turn".into()];
    for i in 0..19 {
        header.push(format!("p{}", i));
    }
    header.extend(
        ["t0", "t1", "t2", "chosen", "score"]
            .iter()
            .map(|s| s.to_string()),
    );
    wtr.write_record(&header)?;

    for r in records {
        let mut row: Vec<String> = vec![r.game_idx.to_string(), r.turn.to_string()];
        for i in 0..19 {
            row.push(r.plateau[i].to_string());
        }
        row.push(r.tile.0.to_string());
        row.push(r.tile.1.to_string());
        row.push(r.tile.2.to_string());
        row.push(r.chosen_position.to_string());
        row.push(r.final_score.to_string());
        wtr.write_record(&row)?;
    }
    wtr.flush()?;
    Ok(())
}

// ============================================================
// Main
// ============================================================

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let seed = cli.seed.unwrap_or(42);

    println!("=== V1-Row-Priority Strategic Game Generator ===");
    println!("  Games to play:  {}", cli.num_games);
    println!("  Min score:      {}", cli.min_score);
    println!("  V1 bonus:       {:.1}", cli.v1_bonus);
    println!("  Line boost:     {:.1}", cli.line_boost);
    println!("  Model:          {}", cli.model_path);
    println!("  Output:         {}", cli.output);
    println!("  Seed:           {}", seed);
    println!();

    // Load model
    let mut vs = nn::VarStore::new(Device::Cpu);
    let policy_net = GraphTransformerPolicyNet::new(&vs, 47, 128, 2, 4, 0.1);
    load_varstore(&mut vs, &cli.model_path)?;
    let _guard = tch::no_grad_guard();

    let mut rng = StdRng::seed_from_u64(seed);
    let mut all_records: Vec<TurnRecord> = Vec::new();
    let mut seen_boards: HashSet<String> = HashSet::new();
    let mut kept_games = 0usize;
    let mut total_score: i64 = 0;
    let mut all_scores: Vec<i32> = Vec::with_capacity(cli.num_games);
    let start = Instant::now();

    for game_num in 0..cli.num_games {
        let tiles = generate_tile_sequence(&mut rng);
        let (plateau, turns) = play_game_v1_strategic(&tiles, &policy_net, cli.line_boost, cli.v1_bonus);
        let score = result(&plateau);
        all_scores.push(score);

        // Filter by min score
        if score < cli.min_score {
            if (game_num + 1) % 5000 == 0 {
                let avg_all =
                    all_scores.iter().map(|&s| s as f64).sum::<f64>() / all_scores.len() as f64;
                let kept_avg = if kept_games > 0 {
                    total_score as f64 / kept_games as f64
                } else {
                    0.0
                };
                println!(
                    "  {:>6} games ({:.1}s) | avg={:.1} | kept={} (avg={:.1})",
                    game_num + 1,
                    start.elapsed().as_secs_f64(),
                    avg_all,
                    kept_games,
                    kept_avg,
                );
            }
            continue;
        }

        // Dedup by final board state
        let final_encoded = encode_board(&plateau);
        let key = board_key(&final_encoded);
        if !seen_boards.insert(key) {
            continue; // duplicate board
        }

        // Record all turns for this game
        let game_idx = kept_games;
        for (turn, encoded, tile, chosen) in turns {
            all_records.push(TurnRecord {
                game_idx,
                turn,
                plateau: encoded,
                tile,
                chosen_position: chosen,
                final_score: score,
            });
        }
        kept_games += 1;
        total_score += score as i64;

        if (game_num + 1) % 5000 == 0 {
            let avg_all =
                all_scores.iter().map(|&s| s as f64).sum::<f64>() / all_scores.len() as f64;
            let kept_avg = total_score as f64 / kept_games as f64;
            println!(
                "  {:>6} games ({:.1}s) | avg={:.1} | kept={} (avg={:.1})",
                game_num + 1,
                start.elapsed().as_secs_f64(),
                avg_all,
                kept_games,
                kept_avg,
            );
        }
    }

    // Final stats
    let elapsed = start.elapsed().as_secs_f64();
    let avg_all = all_scores.iter().map(|&s| s as f64).sum::<f64>() / all_scores.len() as f64;
    let mut sorted = all_scores.clone();
    sorted.sort();
    let median = sorted[sorted.len() / 2];
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let above_threshold = all_scores.iter().filter(|&&s| s >= cli.min_score).count();
    let dup_count = above_threshold - kept_games;

    println!();
    println!("=== Results ===");
    println!(
        "  All games:   {} played, avg={:.1}, median={}, min={}, max={}",
        cli.num_games, avg_all, median, min, max
    );
    println!(
        "  Above {}:  {} ({:.1}%)",
        cli.min_score,
        above_threshold,
        above_threshold as f64 / cli.num_games as f64 * 100.0
    );
    println!("  Duplicates:  {}", dup_count);
    println!(
        "  Kept:        {} unique games, {} turn records",
        kept_games,
        all_records.len()
    );
    if kept_games > 0 {
        println!(
            "  Kept avg:    {:.1}",
            total_score as f64 / kept_games as f64
        );
    }
    println!("  Time:        {:.1}s", elapsed);

    // Save
    save_csv(&all_records, &cli.output)?;
    println!("\nSaved to {}", cli.output);

    Ok(())
}
