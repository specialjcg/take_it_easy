//! Benchmark Strategy Comparison for Take It Easy
//!
//! Compares strong strategies only:
//!   GT Direct | GT Beam | Hybrid Beam | Full Beam | AZ MCTS
//!
//! Usage: cargo run --release --bin benchmark_strategies [-- --beam-width 5 --beam-rollouts 30]

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;
use tch::{nn, Device, Kind, Tensor};

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
use take_it_easy::strategy::gt_boost::{self, gt_beam_rollout_select, gt_beam_v1_select, gt_boosted_select, gt_mcts_select};

#[derive(Parser)]
#[command(name = "benchmark_strategies", about = "Benchmark strategy comparison")]
struct Args {
    /// Directory containing recorded game CSVs
    #[arg(long, default_value = "data/recorded_games")]
    data_dir: String,

    /// Number of additional random-tile-sequence games to play
    #[arg(long, default_value_t = 100)]
    random_games: usize,

    /// Path to GT policy model weights
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    model_path: String,

    /// Beam search width (number of top-K GT candidates)
    #[arg(long, default_value_t = 5)]
    beam_width: usize,

    /// Number of rollouts per beam candidate (for GT Beam and Hybrid Beam)
    #[arg(long, default_value_t = 30)]
    beam_rollouts: usize,

    /// Number of rollouts per candidate for Full Beam
    #[arg(long, default_value_t = 10)]
    full_beam_rollouts: usize,

    /// Number of PUCT simulations per turn for AlphaZero MCTS
    #[arg(long, default_value_t = 150)]
    az_sims: usize,

    /// Line completion boost strength in logit units (max completion = boost × 1.0)
    #[arg(long, default_value_t = 3.0)]
    line_boost: f64,

    /// Row v1 affinity boost strength (encourages v1-homogeneous horizontal rows)
    #[arg(long, default_value_t = 2.0)]
    row_boost: f64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Number of boosted rollouts per candidate (0 = no rollouts, use direct)
    #[arg(long, default_value_t = 0)]
    boost_rollouts: usize,

    /// Beam width for boost rollouts (top-K candidates)
    #[arg(long, default_value_t = 3)]
    boost_beam_k: usize,

    /// Number of PUCT MCTS simulations per turn using GT+Boost (0 = disabled)
    #[arg(long, default_value_t = 0)]
    mcts_sims: usize,

    /// Number of rollouts for v1-beam strategy (0 = disabled)
    #[arg(long, default_value_t = 0)]
    v1_beam_rollouts: usize,

    /// Beam width for v1-beam strategy
    #[arg(long, default_value_t = 3)]
    v1_beam_k: usize,

    /// V1-row bonus strength for v1-aware rollouts (0.0 = no v1 bonus in rollouts)
    #[arg(long, default_value_t = 2.0)]
    v1_bonus: f64,

    /// Only run GT Direct (skip beam search for fast comparison)
    #[arg(long, default_value_t = false)]
    direct_only: bool,

    /// Graph Transformer embedding dimension
    #[arg(long, default_value_t = 128)]
    embed_dim: i64,

    /// Graph Transformer number of layers
    #[arg(long, default_value_t = 2)]
    num_layers: usize,

    /// Graph Transformer number of attention heads
    #[arg(long, default_value_t = 4)]
    num_heads: i64,
}

/// Re-export LINES from gt_boost module for local use.
const LINES: [(&[usize], usize); 15] = gt_boost::LINES;

// ─── Data structures ──────────────────────────────────────────────

#[derive(Debug, Clone)]
struct RecordedGame {
    game_id: String,
    tile_sequence: Vec<Tile>,
    human_score: i32,
    #[allow(dead_code)]
    ai_score: i32,
    #[allow(dead_code)]
    human_won: bool,
}

#[derive(Debug, Default)]
struct LineCompletions {
    v1_cols: usize,
    v2_diags: usize,
    v3_diags: usize,
}

impl LineCompletions {
    fn total(&self) -> usize {
        self.v1_cols + self.v2_diags + self.v3_diags
    }
}

struct StrategyStats {
    name: String,
    scores: Vec<i32>,
    completions: Vec<LineCompletions>,
}

impl StrategyStats {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            scores: Vec::new(),
            completions: Vec::new(),
        }
    }

    fn push(&mut self, score: i32, completions: LineCompletions) {
        self.scores.push(score);
        self.completions.push(completions);
    }
}

// ─── CSV loading ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CsvMove {
    game_id: String,
    turn: usize,
    player_type: String,
    tile: Tile,
    #[allow(dead_code)]
    position: usize,
    final_score: i32,
    human_won: bool,
}

fn load_csv(path: &Path) -> Vec<CsvMove> {
    let mut moves = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return moves,
    };

    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let _ = lines.next();

    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 28 {
            continue;
        }

        let tile_0: i32 = parts[22].parse().unwrap_or(0);
        let tile_1: i32 = parts[23].parse().unwrap_or(0);
        let tile_2: i32 = parts[24].parse().unwrap_or(0);

        moves.push(CsvMove {
            game_id: parts[0].to_string(),
            turn: parts[1].parse().unwrap_or(0),
            player_type: parts[2].to_string(),
            tile: Tile(tile_0, tile_1, tile_2),
            position: parts[25].parse().unwrap_or(0),
            final_score: parts[26].parse().unwrap_or(0),
            human_won: parts[27].parse::<i32>().unwrap_or(0) == 1,
        });
    }
    moves
}

fn load_all_games(dir: &str) -> Vec<RecordedGame> {
    let path = Path::new(dir);
    if !path.exists() {
        eprintln!("Data directory not found: {}", dir);
        return Vec::new();
    }

    let mut all_moves = Vec::new();
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let file_path = entry.path();
        if file_path.extension().map_or(false, |e| e == "csv") {
            let csv_moves = load_csv(&file_path);
            println!(
                "  {} : {} rows",
                file_path.file_name().unwrap().to_string_lossy(),
                csv_moves.len()
            );
            all_moves.extend(csv_moves);
        }
    }

    let mut by_game: HashMap<String, Vec<CsvMove>> = HashMap::new();
    for m in all_moves {
        by_game.entry(m.game_id.clone()).or_default().push(m);
    }

    let mut games = Vec::new();
    for (game_id, mut moves) in by_game {
        moves.sort_by_key(|m| (m.turn, m.player_type.clone()));

        let human_moves: Vec<&CsvMove> = moves
            .iter()
            .filter(|m| m.player_type == "Human")
            .collect();
        let ai_moves: Vec<&CsvMove> = moves
            .iter()
            .filter(|m| m.player_type != "Human")
            .collect();

        if human_moves.is_empty() {
            continue;
        }

        let mut tile_seq: Vec<(usize, Tile)> = human_moves
            .iter()
            .map(|m| (m.turn, m.tile))
            .collect();
        tile_seq.sort_by_key(|(t, _)| *t);
        let tile_sequence: Vec<Tile> = tile_seq.into_iter().map(|(_, t)| t).collect();

        if tile_sequence.len() != 19 {
            continue;
        }

        let human_score = human_moves[0].final_score;
        let ai_score = if !ai_moves.is_empty() {
            ai_moves[0].final_score
        } else {
            0
        };
        let human_won = human_moves[0].human_won;

        games.push(RecordedGame {
            game_id,
            tile_sequence,
            human_score,
            ai_score,
            human_won,
        });
    }

    games.sort_by(|a, b| a.game_id.cmp(&b.game_id));
    games
}

// ─── Strategy implementations ─────────────────────────────────────

/// Helper: compute masked GT logits for a position.
fn gt_masked_logits(
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

/// Find positions that would complete a scoring line for this tile.
fn find_line_completing_positions(plateau: &Plateau, tile: &Tile) -> Vec<usize> {
    let mut positions = Vec::new();
    for &(line_positions, direction) in &LINES {
        let get_value = |t: &Tile| match direction {
            0 => t.0,
            1 => t.1,
            _ => t.2,
        };
        let tile_value = get_value(tile);
        if tile_value == 0 {
            continue;
        }

        let mut empty_pos = None;
        let mut all_match = true;
        let mut empty_count = 0;

        for &pos in line_positions {
            let t = &plateau.tiles[pos];
            if *t == Tile(0, 0, 0) {
                empty_count += 1;
                empty_pos = Some(pos);
            } else if get_value(t) != tile_value {
                all_match = false;
                break;
            }
        }

        if all_match && empty_count == 1 {
            if let Some(pos) = empty_pos {
                if !positions.contains(&pos) {
                    positions.push(pos);
                }
            }
        }
    }
    positions
}

// ─── GT Direct ────────────────────────────────────────────────────

fn play_gt_direct(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }
        let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
        let best_pos = masked.argmax(-1, false).int64_value(&[]) as usize;
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── Line completion boost ───────────────────────────────────────
// Uses gt_boost::line_boost from the shared module.
use gt_boost::line_boost;

// ─── Row definitions (v1 horizontal rows) ────────────────────────

const ROWS: [&[usize]; 5] = [
    &[0, 1, 2],
    &[3, 4, 5, 6],
    &[7, 8, 9, 10, 11],
    &[12, 13, 14, 15],
    &[16, 17, 18],
];

/// Maps position → row index for O(1) lookup
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

/// Row affinity boost — defensive approach.
///
/// Main signal: PENALIZE contaminating a clean row (placing a tile whose v1
/// doesn't match an existing homogeneous row). This mimics the human instinct
/// to avoid ruining a row they've been building.
///
/// Secondary signal: small bonus for reinforcing a row with 2+ matching tiles.
/// No bonus for empty rows or rows with only 1 tile (let GT decide freely).
fn row_affinity_boost(plateau: &Plateau, tile: &Tile, position: usize, boost: f64) -> f64 {
    let row_idx = pos_to_row(position);
    let row = ROWS[row_idx];
    let row_len = row.len();
    let v1 = tile.0;

    let mut same = 0usize;
    let mut diff = 0usize;
    let mut existing_v1 = 0i32; // v1 of existing tiles (if homogeneous)
    let mut homogeneous = true;

    for &pos in row {
        if pos == position {
            continue;
        }
        let t = &plateau.tiles[pos];
        if *t == Tile(0, 0, 0) {
            continue; // empty, skip
        }
        if existing_v1 == 0 {
            existing_v1 = t.0;
        } else if t.0 != existing_v1 {
            homogeneous = false;
        }
        if t.0 == v1 {
            same += 1;
        } else {
            diff += 1;
        }
    }

    let filled = same + diff;
    let norm = 45.0;

    // === PENALTY: contaminating a clean row ===
    // Row has 1+ tiles, all same v1, and we'd be the first different one
    if homogeneous && filled >= 1 && diff == 0 && same == 0 {
        // We're about to break a clean row. Penalty scales with:
        // - how many tiles are already there (more = bigger loss)
        // - the existing v1 value × row length (what they'd score if completed)
        let existing_potential = existing_v1 as f64 * row_len as f64 / norm;
        let progress = filled as f64 / (row_len - 1) as f64;
        return -boost * 0.6 * progress * existing_potential;
    }

    // === BONUS: reinforcing a strong row (2+ matching, no conflicts) ===
    if diff == 0 && same >= 2 {
        let potential = v1 as f64 * row_len as f64 / norm;
        let progress = same as f64 / (row_len - 1) as f64;
        return boost * 0.3 * progress * potential;
    }

    // All other cases: no signal (let GT decide)
    0.0
}

// ─── GT + Line Boost ─────────────────────────────────────────────

fn play_gt_line_heuristic(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    deck_init: &Deck,
    boost: f64,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = deck_init.clone();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        if legal.len() == 1 {
            plateau.tiles[legal[0]] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
            continue;
        }

        // Get raw GT logits (not softmax — we add boost directly)
        let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

        // Pick position with highest (GT logit + line boost)
        let best_pos = *legal
            .iter()
            .max_by(|&&pos_a, &&pos_b| {
                let sa = logit_values[pos_a] + line_boost(&plateau, tile, pos_a, boost);
                let sb = logit_values[pos_b] + line_boost(&plateau, tile, pos_b, boost);
                sa.partial_cmp(&sb).unwrap()
            })
            .unwrap();

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── GT + Lines + Row Affinity ────────────────────────────────────

fn play_gt_lines_rows(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    deck_init: &Deck,
    line_boost_val: f64,
    row_boost_val: f64,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = deck_init.clone();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        if legal.len() == 1 {
            plateau.tiles[legal[0]] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
            continue;
        }

        // Get raw GT logits
        let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();

        // Pick position with highest (GT logit + line boost + row affinity)
        let best_pos = *legal
            .iter()
            .max_by(|&&a, &&b| {
                let sa = logit_values[a]
                    + line_boost(&plateau, tile, a, line_boost_val)
                    + row_affinity_boost(&plateau, tile, a, row_boost_val);
                let sb = logit_values[b]
                    + line_boost(&plateau, tile, b, line_boost_val)
                    + row_affinity_boost(&plateau, tile, b, row_boost_val);
                sa.partial_cmp(&sb).unwrap()
            })
            .unwrap();

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── Rollout helper ───────────────────────────────────────────────

/// Simulate a game from a partial state using GT Direct for remaining turns.
fn gt_rollout_from_state(
    plateau: &Plateau,
    deck: &Deck,
    current_turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    rng: &mut StdRng,
) -> i32 {
    let mut plateau = plateau.clone();
    let mut deck = deck.clone();

    for turn in current_turn..19 {
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }
        let tile = *available.choose(rng).unwrap();

        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let masked = gt_masked_logits(&plateau, &tile, &deck, turn, policy_net);
        let best_pos = masked.argmax(-1, false).int64_value(&[]) as usize;
        plateau.tiles[best_pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    result(&plateau)
}

/// Evaluate a set of candidate positions with rollouts and return the best one.
fn evaluate_candidates_with_rollouts(
    candidates: &[usize],
    plateau: &Plateau,
    deck: &Deck,
    tile: &Tile,
    turn: usize,
    num_rollouts: usize,
    policy_net: &GraphTransformerPolicyNet,
    rng: &mut StdRng,
) -> usize {
    let mut best_pos = candidates[0];
    let mut best_avg = f64::NEG_INFINITY;

    for &pos in candidates {
        let mut test_plateau = plateau.clone();
        test_plateau.tiles[pos] = *tile;
        let test_deck = replace_tile_in_deck(deck, tile);

        let mut total: i64 = 0;
        for _ in 0..num_rollouts {
            total += gt_rollout_from_state(&test_plateau, &test_deck, turn + 1, policy_net, rng)
                as i64;
        }
        let avg = total as f64 / num_rollouts as f64;

        if avg > best_avg {
            best_avg = avg;
            best_pos = pos;
        }
    }
    best_pos
}

// ─── GT Beam Search (top-K only) ─────────────────────────────────

fn play_gt_beam_search(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    beam_width: usize,
    num_rollouts: usize,
    rng: &mut StdRng,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        if legal.len() == 1 || turn == 18 {
            let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
            let best = masked.argmax(-1, false).int64_value(&[]) as usize;
            plateau.tiles[best] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
            continue;
        }

        let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();
        let mut gt_ranked: Vec<(usize, f64)> = legal
            .iter()
            .map(|&pos| (pos, logit_values[pos]))
            .collect();
        gt_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        gt_ranked.truncate(beam_width);

        let candidates: Vec<usize> = gt_ranked.iter().map(|&(pos, _)| pos).collect();

        let best_pos = evaluate_candidates_with_rollouts(
            &candidates, &plateau, &deck, tile, turn, num_rollouts, policy_net, rng,
        );

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── Beam + Line Boost ───────────────────────────────────────────

// gt_rollout_boosted is now imported from gt_boost module.

fn play_gt_beam_line_boost(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    beam_width: usize,
    num_rollouts: usize,
    boost: f64,
    rng: &mut StdRng,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        if legal.len() == 1 || turn == 18 {
            // Use boosted logits even for single-choice turns (consistent)
            let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
            let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();
            let best = *legal
                .iter()
                .max_by(|&&a, &&b| {
                    let sa = logit_values[a] + line_boost(&plateau, tile, a, boost);
                    let sb = logit_values[b] + line_boost(&plateau, tile, b, boost);
                    sa.partial_cmp(&sb).unwrap()
                })
                .unwrap();
            plateau.tiles[best] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
            continue;
        }

        // Candidate selection: GT logits + line boost → top-K
        let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();
        let mut boosted_ranked: Vec<(usize, f64)> = legal
            .iter()
            .map(|&pos| (pos, logit_values[pos] + line_boost(&plateau, tile, pos, boost)))
            .collect();
        boosted_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        boosted_ranked.truncate(beam_width);

        // Also add line-completing positions not already in candidates
        let completing = find_line_completing_positions(&plateau, tile);
        let mut candidates: Vec<usize> = boosted_ranked.iter().map(|&(pos, _)| pos).collect();
        for pos in completing {
            if legal.contains(&pos) && !candidates.contains(&pos) {
                candidates.push(pos);
            }
        }

        // Evaluate each candidate with plain GT rollouts (unbiased evaluation)
        let best_pos = evaluate_candidates_with_rollouts(
            &candidates, &plateau, &deck, tile, turn, num_rollouts, policy_net, rng,
        );

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── Hybrid Beam (GT top-K + greedy + line-completing) ────────────

fn play_gt_hybrid_beam(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    beam_width: usize,
    num_rollouts: usize,
    rng: &mut StdRng,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        if legal.len() == 1 || turn == 18 {
            let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
            let best = masked.argmax(-1, false).int64_value(&[]) as usize;
            plateau.tiles[best] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
            continue;
        }

        // GT top-K candidates
        let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&masked).unwrap();
        let mut gt_ranked: Vec<(usize, f64)> = legal
            .iter()
            .map(|&pos| (pos, logit_values[pos]))
            .collect();
        gt_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        gt_ranked.truncate(beam_width);

        let mut candidates: Vec<usize> = gt_ranked.iter().map(|&(pos, _)| pos).collect();

        // Add greedy position (max immediate score)
        let greedy_pos = *legal
            .iter()
            .max_by_key(|&&pos| {
                let mut test = plateau.clone();
                test.tiles[pos] = *tile;
                result(&test)
            })
            .unwrap();
        if !candidates.contains(&greedy_pos) {
            candidates.push(greedy_pos);
        }

        // Add line-completing positions
        for pos in find_line_completing_positions(&plateau, tile) {
            if legal.contains(&pos) && !candidates.contains(&pos) {
                candidates.push(pos);
            }
        }

        let best_pos = evaluate_candidates_with_rollouts(
            &candidates, &plateau, &deck, tile, turn, num_rollouts, policy_net, rng,
        );

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── GT + Boost + Rollouts ────────────────────────────────────────

fn play_gt_boost_rollout(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
    beam_k: usize,
    num_rollouts: usize,
    rng: &mut StdRng,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        if legal.len() == 1 || turn == 18 {
            // Use boosted select for trivial turns
            let best = gt_boosted_select(&plateau, tile, &deck, turn, policy_net, boost);
            plateau.tiles[best] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
            continue;
        }

        let best = gt_beam_rollout_select(
            &plateau, tile, &deck, turn, policy_net, boost, beam_k, num_rollouts, rng,
        );
        plateau.tiles[best] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── GT + Boost + V1 Beam (inject v1-ideal candidate) ─────────────

fn play_gt_v1_beam(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
    beam_k: usize,
    num_rollouts: usize,
    v1_bonus: f64,
    rng: &mut StdRng,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let best = gt_beam_v1_select(
            &plateau, tile, &deck, turn, policy_net, boost, beam_k, num_rollouts, v1_bonus, rng,
        );
        plateau.tiles[best] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── GT + Boost + MCTS (PUCT) ─────────────────────────────────────

fn play_gt_mcts_boosted(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    boost: f64,
    num_sims: usize,
    rng: &mut StdRng,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let best = gt_mcts_select(
            &plateau, tile, &deck, turn, policy_net, boost, num_sims, rng,
        );
        plateau.tiles[best] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── Full Beam (all legal positions) ──────────────────────────────

fn play_gt_full_beam(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    num_rollouts: usize,
    rng: &mut StdRng,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        if legal.len() == 1 || turn == 18 {
            let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
            let best = masked.argmax(-1, false).int64_value(&[]) as usize;
            plateau.tiles[best] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
            continue;
        }

        let best_pos = evaluate_candidates_with_rollouts(
            &legal, &plateau, &deck, tile, turn, num_rollouts, policy_net, rng,
        );

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── AlphaZero MCTS (PUCT + GT rollouts) ─────────────────────────

fn play_alphazero_mcts(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    num_sims: usize,
    rng: &mut StdRng,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    const C_PUCT: f64 = 2.5;
    const SCORE_NORM: f64 = 300.0;

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        if legal.len() == 1 || turn == 18 {
            let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
            let best = masked.argmax(-1, false).int64_value(&[]) as usize;
            plateau.tiles[best] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
            continue;
        }

        // GT policy prior (softmax)
        let masked = gt_masked_logits(&plateau, tile, &deck, turn, policy_net);
        let probs = masked.softmax(-1, Kind::Float);
        let prob_values: Vec<f64> = Vec::<f64>::try_from(&probs).unwrap();
        let priors: Vec<f64> = legal.iter().map(|&pos| prob_values[pos]).collect();

        let n = legal.len();
        let mut visit_counts = vec![0usize; n];
        let mut total_values = vec![0.0f64; n];

        for _ in 0..num_sims {
            let total_visits: usize = visit_counts.iter().sum();
            let sqrt_total = (total_visits as f64).sqrt();

            // PUCT selection
            let best_idx = (0..n)
                .max_by(|&a, &b| {
                    let q_a = if visit_counts[a] > 0 {
                        total_values[a] / (visit_counts[a] as f64 * SCORE_NORM)
                    } else {
                        0.5
                    };
                    let q_b = if visit_counts[b] > 0 {
                        total_values[b] / (visit_counts[b] as f64 * SCORE_NORM)
                    } else {
                        0.5
                    };
                    let u_a = C_PUCT * priors[a] * sqrt_total / (1.0 + visit_counts[a] as f64);
                    let u_b = C_PUCT * priors[b] * sqrt_total / (1.0 + visit_counts[b] as f64);
                    (q_a + u_a).partial_cmp(&(q_b + u_b)).unwrap()
                })
                .unwrap();

            let pos = legal[best_idx];
            let mut test_plateau = plateau.clone();
            test_plateau.tiles[pos] = *tile;
            let test_deck = replace_tile_in_deck(&deck, tile);
            let score =
                gt_rollout_from_state(&test_plateau, &test_deck, turn + 1, policy_net, rng);

            visit_counts[best_idx] += 1;
            total_values[best_idx] += score as f64;
        }

        // Pick action with highest average value
        let best_idx = (0..n)
            .max_by(|&a, &b| {
                let avg_a = if visit_counts[a] > 0 {
                    total_values[a] / visit_counts[a] as f64
                } else {
                    0.0
                };
                let avg_b = if visit_counts[b] > 0 {
                    total_values[b] / visit_counts[b] as f64
                } else {
                    0.0
                };
                avg_a.partial_cmp(&avg_b).unwrap()
            })
            .unwrap();

        plateau.tiles[legal[best_idx]] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── Line completion analysis ─────────────────────────────────────

fn count_line_completions(plateau: &Plateau) -> LineCompletions {
    let mut lc = LineCompletions::default();

    for &(positions, direction) in &LINES {
        let get_value = |tile: &Tile| match direction {
            0 => tile.0,
            1 => tile.1,
            _ => tile.2,
        };

        let first = &plateau.tiles[positions[0]];
        if *first == Tile(0, 0, 0) {
            continue;
        }
        let target = get_value(first);
        if target == 0 {
            continue;
        }

        let all_match = positions.iter().all(|&i| {
            let t = &plateau.tiles[i];
            *t != Tile(0, 0, 0) && get_value(t) == target
        });

        if all_match {
            match direction {
                0 => lc.v1_cols += 1,
                1 => lc.v2_diags += 1,
                _ => lc.v3_diags += 1,
            }
        }
    }

    lc
}

// ─── Random game generation ───────────────────────────────────────

fn generate_random_tile_sequence(rng: &mut StdRng) -> Vec<Tile> {
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

// ─── Main ─────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();
    let mut rng = StdRng::seed_from_u64(args.seed);

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Strategy Benchmark — Take It Easy                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let device = Device::Cpu;
    let mut vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(&vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.1);

    print!("Loading GT policy model from {}... ", &args.model_path);
    if let Err(e) = load_varstore(&mut vs, &args.model_path) {
        eprintln!("FAILED: {}", e);
        return;
    }
    println!("OK");

    // Strategy labels
    let line_label = format!("GT+Lines (b={:.1})", args.line_boost);
    let rows_label = format!("GT+L+R (b={:.1},r={:.1})", args.line_boost, args.row_boost);
    let beam_label = format!("GT Beam ({},{})", args.beam_width, args.beam_rollouts);
    let beam_lines_label = format!("Beam+Lines ({},{},b={:.1})", args.beam_width, args.beam_rollouts, args.line_boost);
    let boost_rollout_label = format!("B+Roll ({},{},b={:.1})", args.boost_beam_k, args.boost_rollouts, args.line_boost);
    let use_boost_rollouts = args.boost_rollouts > 0;
    let mcts_label = format!("MCTS ({},b={:.1})", args.mcts_sims, args.line_boost);
    let use_mcts = args.mcts_sims > 0;
    let v1_beam_label = format!("V1Beam ({},{},b={:.1},v={:.1})", args.v1_beam_k, args.v1_beam_rollouts, args.line_boost, args.v1_bonus);
    let use_v1_beam = args.v1_beam_rollouts > 0;

    // Load recorded games
    println!("\nLoading recorded games from {}:", &args.data_dir);
    let recorded_games = load_all_games(&args.data_dir);
    println!("  Total: {} complete games\n", recorded_games.len());

    if recorded_games.is_empty() && args.random_games == 0 {
        eprintln!("No games to benchmark. Exiting.");
        return;
    }

    // ─── Benchmark on recorded games ──────────────────────────────

    println!(
        "Running benchmark on {} recorded games...\n  Beam: K={} M={} | B+L boost={:.1}\n",
        recorded_games.len(),
        args.beam_width, args.beam_rollouts,
        args.line_boost,
    );

    let mut strategies: Vec<StrategyStats> = {
        let mut v = vec![
            StrategyStats::new("GT Direct"),
            StrategyStats::new(&line_label),
            StrategyStats::new(&rows_label),
        ];
        if use_boost_rollouts {
            v.push(StrategyStats::new(&boost_rollout_label));
        }
        if use_mcts {
            v.push(StrategyStats::new(&mcts_label));
        }
        if use_v1_beam {
            v.push(StrategyStats::new(&v1_beam_label));
        }
        if !args.direct_only {
            v.push(StrategyStats::new(&beam_label));
            v.push(StrategyStats::new(&beam_lines_label));
        }
        v
    };
    // Strategy index helpers
    let idx_direct = 0;
    let idx_lines = 1;
    let idx_rows = 2;
    let mut next_idx = 3;
    let idx_boost_roll = if use_boost_rollouts { let i = next_idx; next_idx += 1; Some(i) } else { None };
    let idx_mcts = if use_mcts { let i = next_idx; next_idx += 1; Some(i) } else { None };
    let idx_v1_beam = if use_v1_beam { let i = next_idx; next_idx += 1; Some(i) } else { None };
    let idx_beam = if !args.direct_only { let i = next_idx; next_idx += 1; Some(i) } else { None };
    let idx_beam_lines = if !args.direct_only { let i = next_idx; next_idx += 1; Some(i) } else { None };
    let _ = next_idx;

    let mut game_ids: Vec<String> = Vec::new();

    let total = recorded_games.len();
    let start = Instant::now();

    for (i, game) in recorded_games.iter().enumerate() {
        let game_start = Instant::now();
        let tiles = &game.tile_sequence;

        let (s, c) = play_gt_direct(tiles, &policy_net);
        strategies[idx_direct].push(s, c);
        let gt_s = s;

        let (s, c) = play_gt_line_heuristic(tiles, &policy_net, &create_deck(), args.line_boost);
        strategies[idx_lines].push(s, c);
        let line_s = s;

        let (s, c) = play_gt_lines_rows(tiles, &policy_net, &create_deck(), args.line_boost, args.row_boost);
        strategies[idx_rows].push(s, c);
        let row_s = s;

        let mut br_s = 0;
        if let Some(idx) = idx_boost_roll {
            let (s, c) = play_gt_boost_rollout(tiles, &policy_net, args.line_boost, args.boost_beam_k, args.boost_rollouts, &mut rng);
            strategies[idx].push(s, c);
            br_s = s;
        }

        let mut mcts_s = 0;
        if let Some(idx) = idx_mcts {
            let (s, c) = play_gt_mcts_boosted(tiles, &policy_net, args.line_boost, args.mcts_sims, &mut rng);
            strategies[idx].push(s, c);
            mcts_s = s;
        }

        let mut v1b_s = 0;
        if let Some(idx) = idx_v1_beam {
            let (s, c) = play_gt_v1_beam(tiles, &policy_net, args.line_boost, args.v1_beam_k, args.v1_beam_rollouts, args.v1_bonus, &mut rng);
            strategies[idx].push(s, c);
            v1b_s = s;
        }

        let mut beam_s = 0;
        let mut bl_s = 0;
        if let Some(idx) = idx_beam {
            let (s, c) = play_gt_beam_search(tiles, &policy_net, args.beam_width, args.beam_rollouts, &mut rng);
            strategies[idx].push(s, c);
            beam_s = s;
        }
        if let Some(idx) = idx_beam_lines {
            let (s, c) = play_gt_beam_line_boost(tiles, &policy_net, args.beam_width, args.beam_rollouts, args.line_boost, &mut rng);
            strategies[idx].push(s, c);
            bl_s = s;
        }

        // Progress line
        let id_short = if game.game_id.len() >= 8 { &game.game_id[..8] } else { &game.game_id };
        let mut prog = format!("\r  [{}/{}] {} : GT={} Line={} L+R={}", i+1, total, id_short, gt_s, line_s, row_s);
        if use_boost_rollouts { prog.push_str(&format!(" B+R={}", br_s)); }
        if use_mcts { prog.push_str(&format!(" MCTS={}", mcts_s)); }
        if use_v1_beam { prog.push_str(&format!(" V1B={}", v1b_s)); }
        if !args.direct_only { prog.push_str(&format!(" Beam={} B+L={}", beam_s, bl_s)); }
        prog.push_str(&format!(" ({:.1}s)    ", game_start.elapsed().as_secs_f64()));
        print!("{}", prog);

        game_ids.push(game.game_id.clone());
    }
    if !recorded_games.is_empty() {
        println!("\n\nRecorded games completed in {:.1}s\n", start.elapsed().as_secs_f64());
    }

    // ─── Random games benchmark ───────────────────────────────────

    let mut rand_strategies: Vec<StrategyStats> = {
        let mut v = vec![
            StrategyStats::new("GT Direct"),
            StrategyStats::new(&line_label),
            StrategyStats::new(&rows_label),
        ];
        if use_boost_rollouts {
            v.push(StrategyStats::new(&boost_rollout_label));
        }
        if use_mcts {
            v.push(StrategyStats::new(&mcts_label));
        }
        if use_v1_beam {
            v.push(StrategyStats::new(&v1_beam_label));
        }
        if !args.direct_only {
            v.push(StrategyStats::new(&beam_label));
            v.push(StrategyStats::new(&beam_lines_label));
        }
        v
    };

    if args.random_games > 0 {
        println!(
            "Running benchmark on {} random tile sequences...\n  line={:.1} row={:.1}{}\n",
            args.random_games,
            args.line_boost, args.row_boost,
            if use_boost_rollouts { format!(" | boost-rollouts: K={} M={}", args.boost_beam_k, args.boost_rollouts) } else { String::new() },
        );
        let rg_start = Instant::now();

        for i in 0..args.random_games {
            let tiles = generate_random_tile_sequence(&mut rng);

            let (s, c) = play_gt_direct(&tiles, &policy_net);
            rand_strategies[idx_direct].push(s, c);
            let gt_s = s;

            let (s, c) = play_gt_line_heuristic(&tiles, &policy_net, &create_deck(), args.line_boost);
            rand_strategies[idx_lines].push(s, c);
            let line_s = s;

            let (s, c) = play_gt_lines_rows(&tiles, &policy_net, &create_deck(), args.line_boost, args.row_boost);
            rand_strategies[idx_rows].push(s, c);
            let row_s = s;

            let mut br_s = 0;
            if let Some(idx) = idx_boost_roll {
                let (s, c) = play_gt_boost_rollout(&tiles, &policy_net, args.line_boost, args.boost_beam_k, args.boost_rollouts, &mut rng);
                rand_strategies[idx].push(s, c);
                br_s = s;
            }

            let mut mcts_s = 0;
            if let Some(idx) = idx_mcts {
                let (s, c) = play_gt_mcts_boosted(&tiles, &policy_net, args.line_boost, args.mcts_sims, &mut rng);
                rand_strategies[idx].push(s, c);
                mcts_s = s;
            }

            let mut v1b_s = 0;
            if let Some(idx) = idx_v1_beam {
                let (s, c) = play_gt_v1_beam(&tiles, &policy_net, args.line_boost, args.v1_beam_k, args.v1_beam_rollouts, args.v1_bonus, &mut rng);
                rand_strategies[idx].push(s, c);
                v1b_s = s;
            }

            let mut beam_s = 0;
            let mut bl_s = 0;
            if let Some(idx) = idx_beam {
                let (s, c) = play_gt_beam_search(&tiles, &policy_net, args.beam_width, args.beam_rollouts, &mut rng);
                rand_strategies[idx].push(s, c);
                beam_s = s;
            }
            if let Some(idx) = idx_beam_lines {
                let (s, c) = play_gt_beam_line_boost(&tiles, &policy_net, args.beam_width, args.beam_rollouts, args.line_boost, &mut rng);
                rand_strategies[idx].push(s, c);
                bl_s = s;
            }

            let mut prog = format!("\r  [{}/{}] GT={} Line={} L+R={}", i+1, args.random_games, gt_s, line_s, row_s);
            if use_boost_rollouts { prog.push_str(&format!(" B+R={}", br_s)); }
            if use_mcts { prog.push_str(&format!(" MCTS={}", mcts_s)); }
            if use_v1_beam { prog.push_str(&format!(" V1B={}", v1b_s)); }
            if !args.direct_only { prog.push_str(&format!(" Beam={} B+L={}", beam_s, bl_s)); }
            prog.push_str("    ");
            print!("{}", prog);
        }
        println!(
            "\n\nRandom games completed in {:.1}s\n",
            rg_start.elapsed().as_secs_f64()
        );
    }

    // ─── Print results ────────────────────────────────────────────

    print_summary_table("Recorded Games", &strategies);

    if !rand_strategies[0].scores.is_empty() {
        print_summary_table("Random Games", &rand_strategies);
    }

    // ─── Save CSV ─────────────────────────────────────────────────

    save_results_csv(&game_ids, &strategies);
}

// ─── Display ─────────────────────────────────────────────────────

fn print_summary_table(title: &str, strategies: &[StrategyStats]) {
    if strategies.is_empty() || strategies[0].scores.is_empty() {
        return;
    }
    let n = strategies[0].scores.len();
    let baseline = &strategies[0].scores;

    println!("╔══════════════════════════════════════════════════════════════════════╗");
    println!(
        "║  {} ({} games){}║",
        title, n,
        " ".repeat(68usize.saturating_sub(5 + title.len() + format!("{}", n).len()))
    );
    println!("╚══════════════════════════════════════════════════════════════════════╝\n");

    println!(
        "{:<20} | {:>9} | {:>6} | {:>5} | {:>5} | {:>10}",
        "Strategy", "Avg Score", "Median", "Min", "Max", "> GT Direct"
    );
    println!(
        "{:-<20}-+-{:-<9}-+-{:-<6}-+-{:-<5}-+-{:-<5}-+-{:-<10}",
        "", "", "", "", "", ""
    );

    for (i, strat) in strategies.iter().enumerate() {
        let beats = if i == 0 {
            None
        } else {
            let count = strat.scores.iter().zip(baseline.iter())
                .filter(|(&s, &b)| s > b).count();
            Some((count, n))
        };
        print_strategy_row(&strat.name, &strat.scores, beats);
    }

    println!();

    println!(
        "Line completions (avg per game):\n{:<20} | {:>7} | {:>8} | {:>8} | {:>11}",
        "Strategy", "v1 cols", "v2 diags", "v3 diags", "Total lines"
    );
    println!(
        "{:-<20}-+-{:-<7}-+-{:-<8}-+-{:-<8}-+-{:-<11}",
        "", "", "", "", ""
    );
    for strat in strategies {
        print_lc_row(&strat.name, &strat.completions);
    }
    println!();
}

fn print_strategy_row(name: &str, scores: &[i32], beats_gt: Option<(usize, usize)>) {
    let beats_str = match beats_gt {
        Some((count, total)) => format!("{:.0}%", count as f64 / total as f64 * 100.0),
        None => "—".to_string(),
    };
    println!(
        "{:<20} | {:>9.1} | {:>6} | {:>5} | {:>5} | {:>10}",
        name, avg(scores), median(scores),
        scores.iter().min().unwrap_or(&0),
        scores.iter().max().unwrap_or(&0),
        beats_str
    );
}

fn print_lc_row(name: &str, completions: &[LineCompletions]) {
    let n = completions.len() as f64;
    if n == 0.0 { return; }
    let v1 = completions.iter().map(|c| c.v1_cols).sum::<usize>() as f64 / n;
    let v2 = completions.iter().map(|c| c.v2_diags).sum::<usize>() as f64 / n;
    let v3 = completions.iter().map(|c| c.v3_diags).sum::<usize>() as f64 / n;
    let tot = completions.iter().map(|c| c.total()).sum::<usize>() as f64 / n;
    println!("{:<20} | {:>7.1} | {:>8.1} | {:>8.1} | {:>11.1}", name, v1, v2, v3, tot);
}

fn avg(scores: &[i32]) -> f64 {
    if scores.is_empty() { return 0.0; }
    scores.iter().sum::<i32>() as f64 / scores.len() as f64
}

fn median(scores: &[i32]) -> i32 {
    if scores.is_empty() { return 0; }
    let mut sorted = scores.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}

fn save_results_csv(game_ids: &[String], strategies: &[StrategyStats]) {
    let path = "benchmark_results.csv";
    let mut file = match File::create(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create {}: {}", path, e);
            return;
        }
    };

    let mut header = "game_id".to_string();
    for strat in strategies {
        let col = strat.name.to_lowercase().replace(' ', "_").replace(['(', ')', ','], "");
        header.push(',');
        header.push_str(&col);
    }
    writeln!(file, "{}", header).unwrap();

    for i in 0..game_ids.len() {
        let mut row = game_ids[i].clone();
        for strat in strategies {
            row.push(',');
            row.push_str(&strat.scores[i].to_string());
        }
        writeln!(file, "{}", row).unwrap();
    }

    println!("Per-game details saved to: {}\n", path);
}
