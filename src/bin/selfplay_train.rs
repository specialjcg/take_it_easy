//! AlphaZero Self-Play Training Loop for Take It Easy
//!
//! Subcommands:
//!   generate   - Self-play with current policy, save to CSV
//!   train      - Train policy + value net on self-play data (supports --min-score filtering)
//!   benchmark  - Compare two policies head-to-head
//!   ensemble   - Compare individual models and their weighted logit ensemble
//!   loop       - Full iterative pipeline (generate → train → benchmark → repeat)
//!   gridsearch - Grid search over strategy parameters
//!   expectimax - Compare policy-only vs value-guided vs expectimax search
//!
//! Usage:
//!   CARGO_TARGET_DIR=target2 cargo build --release --bin selfplay_train
//!   ./target2/release/selfplay_train generate --model-path model_weights/graph_transformer_policy.safetensors --num-games 10000
//!   ./target2/release/selfplay_train train --data-dir selfplay_data --generations 0 --init-policy model_weights/graph_transformer_policy.safetensors
//!   ./target2/release/selfplay_train benchmark --model-a <gen0> --model-b <gen1> --num-games 500
//!   ./target2/release/selfplay_train loop --init-policy model_weights/graph_transformer_policy.safetensors --max-generations 10

use clap::{Parser, Subcommand};
use rand::prelude::*;
use rand::rngs::StdRng;
use std::error::Error;
use std::fs;
use std::io::Write as IoWrite;
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::deck::Deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::{GraphTransformerPolicyNet, GraphTransformerValueNet};
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

// ============================================================
// CLI
// ============================================================

#[derive(Parser)]
#[command(name = "selfplay_train", about = "AlphaZero self-play training for Take It Easy")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate self-play games using current policy
    Generate {
        #[arg(long)]
        model_path: String,
        #[arg(long, default_value_t = 10000)]
        num_games: usize,
        #[arg(long, default_value_t = 1.0)]
        temperature: f64,
        #[arg(long, default_value_t = 8)]
        explore_turns: usize,
        #[arg(long, default_value = "selfplay_data")]
        data_dir: String,
        #[arg(long, default_value_t = 0)]
        generation: usize,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 0.0)]
        line_boost: f64,
        #[arg(long, default_value_t = 0.0)]
        row_boost: f64,
    },
    /// Train policy and value networks on self-play data
    Train {
        #[arg(long, default_value = "selfplay_data")]
        data_dir: String,
        #[arg(long, num_args = 1.., required = true)]
        generations: Vec<usize>,
        #[arg(long, num_args = 1..)]
        gen_weights: Option<Vec<f64>>,
        #[arg(long)]
        init_policy: Option<String>,
        #[arg(long)]
        init_value: Option<String>,
        #[arg(long, default_value_t = 50)]
        epochs: usize,
        #[arg(long, default_value_t = 128)]
        batch_size: usize,
        #[arg(long, default_value_t = 3e-4)]
        lr: f64,
        #[arg(long, default_value_t = 1e-4)]
        weight_decay: f64,
        #[arg(long, default_value = "model_weights")]
        output_dir: String,
        #[arg(long)]
        output_gen: Option<usize>,
        #[arg(long)]
        skip_value: bool,
        #[arg(long)]
        skip_policy: bool,
        #[arg(long)]
        min_score: Option<i32>,
        #[arg(long, default_value_t = 128)]
        embed_dim: i64,
        #[arg(long, default_value_t = 2)]
        num_layers: usize,
        #[arg(long, default_value_t = 4)]
        num_heads: i64,
    },
    /// Compare two policies head-to-head on identical tile sequences
    Benchmark {
        #[arg(long)]
        model_a: String,
        #[arg(long)]
        model_b: String,
        #[arg(long, default_value_t = 500)]
        num_games: usize,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 0.0)]
        line_boost_a: f64,
        #[arg(long, default_value_t = 0.0)]
        row_boost_a: f64,
        #[arg(long, default_value_t = 0.0)]
        line_boost_b: f64,
        #[arg(long, default_value_t = 0.0)]
        row_boost_b: f64,
        #[arg(long, default_value_t = 128)]
        embed_dim_a: i64,
        #[arg(long, default_value_t = 2)]
        num_layers_a: usize,
        #[arg(long, default_value_t = 4)]
        num_heads_a: i64,
        #[arg(long, default_value_t = 128)]
        embed_dim_b: i64,
        #[arg(long, default_value_t = 2)]
        num_layers_b: usize,
        #[arg(long, default_value_t = 4)]
        num_heads_b: i64,
    },
    /// Full iterative self-play loop (generate + train + benchmark, repeat)
    Loop {
        #[arg(long)]
        init_policy: String,
        #[arg(long)]
        init_value: Option<String>,
        #[arg(long, default_value_t = 10)]
        max_generations: usize,
        #[arg(long, default_value_t = 10000)]
        games_per_gen: usize,
        #[arg(long, default_value_t = 1.0)]
        temperature: f64,
        #[arg(long, default_value_t = 8)]
        explore_turns: usize,
        #[arg(long, default_value_t = 50)]
        epochs: usize,
        #[arg(long, default_value_t = 128)]
        batch_size: usize,
        #[arg(long, default_value_t = 3e-4)]
        lr: f64,
        #[arg(long, default_value_t = 1e-4)]
        weight_decay: f64,
        #[arg(long, default_value_t = 500)]
        benchmark_games: usize,
        #[arg(long, default_value_t = 1.0)]
        accept_threshold: f64,
        #[arg(long, default_value = "selfplay_data")]
        data_dir: String,
        #[arg(long, default_value = "model_weights")]
        output_dir: String,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 0.0)]
        line_boost: f64,
        #[arg(long, default_value_t = 0.0)]
        row_boost: f64,
    },
    /// Ensemble benchmark: compare individual models and their ensemble
    Ensemble {
        #[arg(long)]
        model_a: String,
        #[arg(long)]
        model_b: String,
        #[arg(long, default_value_t = 1000)]
        num_games: usize,
        #[arg(long, default_value_t = 1.0)]
        weight_b: f64,
        #[arg(long, default_value_t = 0.0)]
        line_boost: f64,
        #[arg(long, default_value_t = 0.0)]
        row_boost: f64,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Grid search over strategy parameters to find optimal line_boost/row_boost
    Gridsearch {
        #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
        model_path: String,
        #[arg(long, default_value_t = 500)]
        num_games: usize,
        #[arg(long, default_value = "0.0,0.5,1.0,1.5,2.0,2.5,3.0,4.0,5.0")]
        line_boosts: String,
        #[arg(long, default_value = "0.0,0.5,1.0,1.5,2.0,3.0")]
        row_boosts: String,
        #[arg(long, default_value = "gridsearch_results.csv")]
        output: String,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Expectimax search: compare policy-only vs value-guided vs expectimax
    Expectimax {
        #[arg(long)]
        model_policy: String,
        #[arg(long)]
        model_value: String,
        #[arg(long, default_value_t = 200)]
        num_games: usize,
        #[arg(long, default_value_t = 1)]
        depth: usize,
        #[arg(long, default_value_t = 0.0)]
        line_boost: f64,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long, default_value_t = 128)]
        embed_dim: i64,
        #[arg(long, default_value_t = 2)]
        num_layers: usize,
        #[arg(long, default_value_t = 4)]
        num_heads: i64,
    },
}

// ============================================================
// Data types
// ============================================================

struct TurnRecord {
    game_idx: usize,
    turn: usize,
    plateau: [i32; 19],        // encoded: 0=empty, v1*100+v2*10+v3
    tile: (i32, i32, i32),     // current tile values
    chosen_position: usize,
    final_score: i32,
}

// ============================================================
// Helpers
// ============================================================

fn encode_tile(t: &Tile) -> i32 {
    if *t == Tile(0, 0, 0) { 0 } else { t.0 * 100 + t.1 * 10 + t.2 }
}

fn decode_tile(v: i32) -> Tile {
    if v == 0 { Tile(0, 0, 0) } else { Tile(v / 100, (v / 10) % 10, v % 10) }
}

fn reconstruct_deck_from_plateau(plateau: &Plateau) -> Deck {
    let mut deck = create_deck();
    for tile in &plateau.tiles {
        if *tile != Tile(0, 0, 0) {
            deck = replace_tile_in_deck(&deck, tile);
        }
    }
    deck
}

fn cosine_lr(base_lr: f64, epoch: usize, total_epochs: usize, min_ratio: f64) -> f64 {
    let min_lr = base_lr * min_ratio;
    let progress = epoch as f64 / total_epochs as f64;
    min_lr + 0.5 * (base_lr - min_lr) * (1.0 + (std::f64::consts::PI * progress).cos())
}

fn generate_tile_sequence(rng: &mut StdRng) -> Vec<Tile> {
    let mut deck = create_deck();
    let mut tiles = Vec::with_capacity(19);
    for _ in 0..19 {
        let available = get_available_tiles(&deck);
        if available.is_empty() { break; }
        let tile = *available.choose(rng).unwrap();
        tiles.push(tile);
        deck = replace_tile_in_deck(&deck, &tile);
    }
    tiles
}

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

fn load_policy_sized(
    path: &str,
    embed_dim: i64,
    num_layers: usize,
    num_heads: i64,
) -> Result<(nn::VarStore, GraphTransformerPolicyNet), Box<dyn Error>> {
    let mut vs = nn::VarStore::new(Device::Cpu);
    let net = GraphTransformerPolicyNet::new(&vs, 47, embed_dim, num_layers, num_heads, 0.1);
    load_varstore(&mut vs, path)?;
    Ok((vs, net))
}

fn load_policy(path: &str) -> Result<(nn::VarStore, GraphTransformerPolicyNet), Box<dyn Error>> {
    load_policy_sized(path, 128, 2, 4)
}

fn load_value_sized(
    path: &str,
    embed_dim: i64,
    num_layers: usize,
    num_heads: i64,
) -> Result<(nn::VarStore, GraphTransformerValueNet), Box<dyn Error>> {
    let mut vs = nn::VarStore::new(Device::Cpu);
    let net = GraphTransformerValueNet::new(&vs, 47, embed_dim, num_layers, num_heads, 0.1);
    load_varstore(&mut vs, path)?;
    Ok((vs, net))
}

fn decode_sample_state(sample: &TurnRecord) -> (Plateau, Tile) {
    let mut plateau = create_plateau_empty();
    for i in 0..19 {
        plateau.tiles[i] = decode_tile(sample.plateau[i]);
    }
    let tile = Tile(sample.tile.0, sample.tile.1, sample.tile.2);
    (plateau, tile)
}

fn compute_sample_weights(samples: &[TurnRecord], gen_weight: f64) -> Vec<f32> {
    let scores: Vec<f64> = samples.iter().map(|s| s.final_score as f64).collect();
    let mean = scores.iter().sum::<f64>() / scores.len() as f64;
    let std = (scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64)
        .sqrt()
        .max(1.0);

    scores
        .iter()
        .map(|&s| {
            let advantage = (s - mean) / std;
            let w = (1.0 + advantage.exp()).ln(); // softplus
            (w * gen_weight) as f32
        })
        .collect()
}

fn score_stats(scores: &[i32]) -> (f64, i32, i32, i32) {
    let avg = scores.iter().map(|&s| s as f64).sum::<f64>() / scores.len() as f64;
    let mut sorted = scores.to_vec();
    sorted.sort();
    let median = sorted[sorted.len() / 2];
    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    (avg, median, min, max)
}

// ============================================================
// Heuristic boosting (ported from benchmark_strategies.rs)
// ============================================================

/// Line definitions: (positions, direction_index)
/// direction 0 = tile.0 (horizontal), 1 = tile.1 (diag1), 2 = tile.2 (diag2)
const LINES: [(&[usize], usize); 15] = [
    (&[0, 1, 2], 0),
    (&[3, 4, 5, 6], 0),
    (&[7, 8, 9, 10, 11], 0),
    (&[12, 13, 14, 15], 0),
    (&[16, 17, 18], 0),
    (&[0, 3, 7], 1),
    (&[1, 4, 8, 12], 1),
    (&[2, 5, 9, 13, 16], 1),
    (&[6, 10, 14, 17], 1),
    (&[11, 15, 18], 1),
    (&[7, 12, 16], 2),
    (&[3, 8, 13, 17], 2),
    (&[0, 4, 9, 14, 18], 2),
    (&[1, 5, 10, 15], 2),
    (&[2, 6, 11], 2),
];

const ROWS: [&[usize]; 5] = [
    &[0, 1, 2],
    &[3, 4, 5, 6],
    &[7, 8, 9, 10, 11],
    &[12, 13, 14, 15],
    &[16, 17, 18],
];

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

fn line_boost_heuristic(plateau: &Plateau, tile: &Tile, position: usize, boost: f64) -> f64 {
    let mut total = 0.0;

    for &(line_positions, direction) in &LINES {
        if !line_positions.contains(&position) {
            continue;
        }

        let tile_val = match direction {
            0 => tile.0,
            1 => tile.1,
            _ => tile.2,
        };

        let line_len = line_positions.len();

        let mut same = 0;
        let mut diff = 0;
        let mut empty = 0;
        let mut diff_val = 0;
        let mut diff_homogeneous = true;

        for &pos in line_positions {
            if pos == position {
                continue;
            }
            let t = &plateau.tiles[pos];
            if *t == Tile(0, 0, 0) {
                empty += 1;
            } else {
                let v = match direction {
                    0 => t.0,
                    1 => t.1,
                    _ => t.2,
                };
                if v == tile_val {
                    same += 1;
                } else {
                    if diff_val == 0 {
                        diff_val = v;
                    } else if v != diff_val {
                        diff_homogeneous = false;
                    }
                    diff += 1;
                }
            }
        }

        let norm = 45.0;

        if diff == 0 {
            if empty == 0 {
                total += boost * tile_val as f64 * line_len as f64 / norm;
            } else if empty == 1 && same >= 1 {
                total += boost * 0.4 * tile_val as f64 * line_len as f64 / norm;
            } else if empty == 2 && same >= 1 {
                total += boost * 0.15 * tile_val as f64 * line_len as f64 / norm;
            }
        } else if diff_homogeneous && same == 0 && diff_val != 0 {
            let progress = diff as f64 / (line_len - 1) as f64;
            total -= boost * 0.3 * progress * diff_val as f64 * line_len as f64 / norm;
        }
    }

    total
}

fn row_affinity_boost(plateau: &Plateau, tile: &Tile, position: usize, boost: f64) -> f64 {
    let row_idx = pos_to_row(position);
    let row = ROWS[row_idx];
    let row_len = row.len();
    let v1 = tile.0;

    let mut same = 0usize;
    let mut diff = 0usize;
    let mut existing_v1 = 0i32;
    let mut homogeneous = true;

    for &pos in row {
        if pos == position {
            continue;
        }
        let t = &plateau.tiles[pos];
        if *t == Tile(0, 0, 0) {
            continue;
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

    if homogeneous && filled >= 1 && diff == 0 && same == 0 {
        let existing_potential = existing_v1 as f64 * row_len as f64 / norm;
        let progress = filled as f64 / (row_len - 1) as f64;
        return -boost * 0.6 * progress * existing_potential;
    }

    if diff == 0 && same >= 2 {
        let potential = v1 as f64 * row_len as f64 / norm;
        let progress = same as f64 / (row_len - 1) as f64;
        return boost * 0.3 * progress * potential;
    }

    0.0
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

#[derive(Clone, Copy, Debug)]
struct StrategyParams {
    line_boost: f64,
    row_boost: f64,
}

impl StrategyParams {
    #[allow(dead_code)]
    fn none() -> Self {
        Self { line_boost: 0.0, row_boost: 0.0 }
    }
    fn is_active(&self) -> bool {
        self.line_boost.abs() > 1e-9 || self.row_boost.abs() > 1e-9
    }
}

fn compute_boosted_logits(
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    turn: usize,
    policy_net: &GraphTransformerPolicyNet,
    strategy: StrategyParams,
) -> Vec<f64> {
    let ml = compute_masked_logits(plateau, tile, deck, turn, policy_net);
    let logits: Vec<f64> = Vec::<f64>::try_from(&ml).unwrap();

    if !strategy.is_active() {
        return logits;
    }

    logits
        .iter()
        .enumerate()
        .map(|(pos, &l)| {
            if l.is_finite() {
                l + line_boost_heuristic(plateau, tile, pos, strategy.line_boost)
                    + row_affinity_boost(plateau, tile, pos, strategy.row_boost)
            } else {
                l
            }
        })
        .collect()
}

fn play_game_with_strategy_stats(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    strategy: StrategyParams,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let pos = if strategy.is_active() {
            let boosted =
                compute_boosted_logits(&plateau, tile, &deck, turn, policy_net, strategy);
            *legal
                .iter()
                .max_by(|&&a, &&b| boosted[a].partial_cmp(&boosted[b]).unwrap())
                .unwrap()
        } else {
            let ml = compute_masked_logits(&plateau, tile, &deck, turn, policy_net);
            ml.argmax(-1, false).int64_value(&[]) as usize
        };

        plateau.tiles[pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ============================================================
// CSV I/O
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
    header.extend(["t0", "t1", "t2", "chosen", "score"].iter().map(|s| s.to_string()));
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

fn load_csv(path: &str) -> Result<Vec<TurnRecord>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut records = Vec::new();
    for row in rdr.records() {
        let row = row?;
        let game_idx: usize = row[0].parse()?;
        let turn: usize = row[1].parse()?;
        let mut plateau = [0i32; 19];
        for i in 0..19 {
            plateau[i] = row[2 + i].parse()?;
        }
        let tile = (row[21].parse()?, row[22].parse()?, row[23].parse()?);
        let chosen: usize = row[24].parse()?;
        let score: i32 = row[25].parse()?;
        records.push(TurnRecord {
            game_idx,
            turn,
            plateau,
            tile,
            chosen_position: chosen,
            final_score: score,
        });
    }
    Ok(records)
}

// ============================================================
// Self-play generation
// ============================================================

fn run_generate(
    model_path: &str,
    num_games: usize,
    temperature: f64,
    explore_turns: usize,
    save_path: &str,
    seed: u64,
    strategy: StrategyParams,
) -> Result<Vec<i32>, Box<dyn Error>> {
    println!("Generating {} self-play games (temp={}, explore_turns={})", num_games, temperature, explore_turns);
    println!("Policy: {}", model_path);
    if strategy.is_active() {
        println!("Strategy: line_boost={:.1}, row_boost={:.1}", strategy.line_boost, strategy.row_boost);
    }

    let (_vs, policy_net) = load_policy(model_path)?;
    let _guard = tch::no_grad_guard();

    let mut rng = StdRng::seed_from_u64(seed);
    let mut all_records = Vec::with_capacity(num_games * 19);
    let mut scores = Vec::with_capacity(num_games);
    let start = Instant::now();

    for game_idx in 0..num_games {
        let tiles = generate_tile_sequence(&mut rng);
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut game_turns: Vec<(usize, [i32; 19], (i32, i32, i32), usize)> = Vec::with_capacity(19);

        for (turn, tile) in tiles.iter().enumerate() {
            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            let pos = if strategy.is_active() {
                let boosted = compute_boosted_logits(
                    &plateau, tile, &deck, turn, &policy_net, strategy,
                );
                if turn < explore_turns && temperature > 1e-6 {
                    let boosted_t = Tensor::from_slice(
                        &boosted.iter().map(|&x| x as f32).collect::<Vec<f32>>(),
                    );
                    let probs = (&boosted_t / temperature).softmax(-1, Kind::Float);
                    probs.multinomial(1, true).int64_value(&[0]) as usize
                } else {
                    *legal
                        .iter()
                        .max_by(|&&a, &&b| boosted[a].partial_cmp(&boosted[b]).unwrap())
                        .unwrap()
                }
            } else {
                let ml = compute_masked_logits(&plateau, tile, &deck, turn, &policy_net);
                if turn < explore_turns && temperature > 1e-6 {
                    let probs = (&ml / temperature).softmax(-1, Kind::Float);
                    probs.multinomial(1, true).int64_value(&[0]) as usize
                } else {
                    ml.argmax(-1, false).int64_value(&[]) as usize
                }
            };

            // Record state BEFORE placing tile
            let mut encoded = [0i32; 19];
            for i in 0..19 {
                encoded[i] = encode_tile(&plateau.tiles[i]);
            }
            game_turns.push((turn, encoded, (tile.0, tile.1, tile.2), pos));

            plateau.tiles[pos] = *tile;
            deck = replace_tile_in_deck(&deck, tile);
        }

        let score = result(&plateau);
        scores.push(score);

        for (turn, encoded, tile, chosen) in game_turns {
            all_records.push(TurnRecord {
                game_idx,
                turn,
                plateau: encoded,
                tile,
                chosen_position: chosen,
                final_score: score,
            });
        }

        if (game_idx + 1) % 1000 == 0 {
            let (avg, _, _, _) = score_stats(&scores);
            println!(
                "  {} games ({:.1}s) - avg score: {:.1}",
                game_idx + 1,
                start.elapsed().as_secs_f64(),
                avg
            );
        }
    }

    let (avg, median, min, max) = score_stats(&scores);
    println!(
        "Done: {} games, avg={:.1}, median={}, min={}, max={}, time={:.1}s",
        num_games,
        avg,
        median,
        min,
        max,
        start.elapsed().as_secs_f64()
    );
    println!("Records: {}", all_records.len());

    save_csv(&all_records, save_path)?;
    println!("Saved to {}", save_path);

    Ok(scores)
}

// ============================================================
// Training: Policy
// ============================================================

fn prepare_policy_batch(
    samples: &[TurnRecord],
    indices: &[usize],
    weights: &[f32],
) -> (Tensor, Tensor, Tensor, Tensor) {
    let mut features = Vec::with_capacity(indices.len());
    let mut masks = Vec::with_capacity(indices.len());
    let mut targets = Vec::with_capacity(indices.len());
    let mut w = Vec::with_capacity(indices.len());

    for &idx in indices {
        let s = &samples[idx];
        let (plateau, tile) = decode_sample_state(s);
        let deck = reconstruct_deck_from_plateau(&plateau);
        features.push(convert_plateau_for_gat_47ch(&plateau, &tile, &deck, s.turn, 19));

        let mut mask = [0.0f32; 19];
        for i in 0..19 {
            if plateau.tiles[i] != Tile(0, 0, 0) {
                mask[i] = f32::NEG_INFINITY;
            }
        }
        masks.push(Tensor::from_slice(&mask));
        targets.push(s.chosen_position as i64);
        w.push(weights[idx]);
    }

    (
        Tensor::stack(&features, 0),                    // [B, 19, 47]
        Tensor::stack(&masks, 0),                       // [B, 19]
        Tensor::from_slice(&targets).unsqueeze(1),      // [B, 1]
        Tensor::from_slice(&w),                         // [B]
    )
}

fn train_policy(
    samples: &[TurnRecord],
    sample_weights: &[f32],
    init_path: Option<&str>,
    save_path: &str,
    epochs: usize,
    batch_size: usize,
    lr: f64,
    weight_decay: f64,
    embed_dim: i64,
    num_layers: usize,
    num_heads: i64,
) -> Result<f64, Box<dyn Error>> {
    println!("\n=== Training Policy ({} samples, embed={}, layers={}, heads={}) ===",
        samples.len(), embed_dim, num_layers, num_heads);

    let mut vs = nn::VarStore::new(Device::Cpu);
    let policy_net = GraphTransformerPolicyNet::new(&vs, 47, embed_dim, num_layers, num_heads, 0.1);

    if let Some(path) = init_path {
        println!("Init weights: {}", path);
        load_varstore(&mut vs, path)?;
    }

    let mut opt = nn::Adam {
        wd: weight_decay,
        ..Default::default()
    }
    .build(&vs, lr)?;

    // Train/val split (90/10)
    let n = samples.len();
    let n_val = (n / 10).max(1);
    let n_train = n - n_val;
    let mut rng = StdRng::seed_from_u64(42);
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rng);
    let train_idx: Vec<usize> = indices[..n_train].to_vec();
    let val_idx: Vec<usize> = indices[n_train..].to_vec();

    let mut best_val_loss = f64::INFINITY;
    let start = Instant::now();

    for epoch in 0..epochs {
        let clr = cosine_lr(lr, epoch, epochs, 0.01);
        opt.set_lr(clr);

        let mut perm = train_idx.clone();
        perm.shuffle(&mut rng);

        let mut epoch_loss = 0.0;
        let mut num_batches = 0;

        for batch_start in (0..perm.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(perm.len());
            let batch = &perm[batch_start..batch_end];
            let (feat, mask, targets, weights) =
                prepare_policy_batch(samples, batch, sample_weights);

            let logits = policy_net.forward(&feat, true);
            let masked = logits + &mask;
            let log_probs = masked.log_softmax(-1, Kind::Float);
            let chosen = log_probs.gather(1, &targets, false).squeeze_dim(1);
            let loss = -(&weights * &chosen).mean(Kind::Float);

            opt.backward_step(&loss);
            epoch_loss += f64::try_from(&loss).unwrap();
            num_batches += 1;
        }

        // Validation
        let val_loss = tch::no_grad(|| {
            let mut total = 0.0;
            let mut count = 0;
            for batch_start in (0..val_idx.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(val_idx.len());
                let batch = &val_idx[batch_start..batch_end];
                let (feat, mask, targets, weights) =
                    prepare_policy_batch(samples, batch, sample_weights);
                let logits = policy_net.forward(&feat, false);
                let masked = logits + &mask;
                let log_probs = masked.log_softmax(-1, Kind::Float);
                let chosen = log_probs.gather(1, &targets, false).squeeze_dim(1);
                let loss = -(&weights * &chosen).mean(Kind::Float);
                total += f64::try_from(&loss).unwrap();
                count += 1;
            }
            if count > 0 { total / count as f64 } else { 0.0 }
        });

        if val_loss < best_val_loss {
            best_val_loss = val_loss;
            if let Err(e) = save_varstore(&vs, save_path) {
                eprintln!("Warning: failed to save policy: {}", e);
            }
        }

        if epoch % 10 == 0 || epoch == epochs - 1 {
            println!(
                "  Epoch {}/{}: train={:.4}, val={:.4}, lr={:.2e} ({:.0}s)",
                epoch + 1,
                epochs,
                epoch_loss / num_batches.max(1) as f64,
                val_loss,
                clr,
                start.elapsed().as_secs_f64()
            );
        }
    }

    println!("Policy best val loss: {:.4}, saved to {}", best_val_loss, save_path);
    Ok(best_val_loss)
}

// ============================================================
// Training: Value
// ============================================================

fn prepare_value_batch(
    samples: &[TurnRecord],
    indices: &[usize],
    score_mean: f64,
    score_std: f64,
) -> (Tensor, Tensor) {
    let mut features = Vec::with_capacity(indices.len());
    let mut targets = Vec::with_capacity(indices.len());

    for &idx in indices {
        let s = &samples[idx];
        let (plateau, tile) = decode_sample_state(s);
        let deck = reconstruct_deck_from_plateau(&plateau);
        features.push(convert_plateau_for_gat_47ch(&plateau, &tile, &deck, s.turn, 19));

        let norm = ((s.final_score as f64 - score_mean) / score_std).clamp(-1.0, 1.0) as f32;
        targets.push(norm);
    }

    (
        Tensor::stack(&features, 0),                     // [B, 19, 47]
        Tensor::from_slice(&targets).unsqueeze(1),       // [B, 1]
    )
}

fn train_value(
    samples: &[TurnRecord],
    init_path: Option<&str>,
    save_path: &str,
    epochs: usize,
    batch_size: usize,
    lr: f64,
    weight_decay: f64,
    embed_dim: i64,
    num_layers: usize,
    num_heads: i64,
) -> Result<f64, Box<dyn Error>> {
    println!("\n=== Training Value ({} samples, embed={}, layers={}, heads={}) ===",
        samples.len(), embed_dim, num_layers, num_heads);

    let mut vs = nn::VarStore::new(Device::Cpu);
    let value_net = GraphTransformerValueNet::new(&vs, 47, embed_dim, num_layers, num_heads, 0.1);

    if let Some(path) = init_path {
        println!("Init weights: {}", path);
        load_varstore(&mut vs, path)?;
    }

    let mut opt = nn::Adam {
        wd: weight_decay,
        ..Default::default()
    }
    .build(&vs, lr)?;

    let score_mean = 140.0;
    let score_std = 40.0;

    // Log actual score distribution
    let real_scores: Vec<f64> = samples.iter().map(|s| s.final_score as f64).collect();
    let real_mean = real_scores.iter().sum::<f64>() / real_scores.len() as f64;
    let real_std = (real_scores
        .iter()
        .map(|s| (s - real_mean).powi(2))
        .sum::<f64>()
        / real_scores.len() as f64)
        .sqrt();
    println!(
        "Score stats: actual mean={:.1}, std={:.1} | norm mean={}, std={}",
        real_mean, real_std, score_mean, score_std
    );

    // Train/val split (90/10)
    let n = samples.len();
    let n_val = (n / 10).max(1);
    let n_train = n - n_val;
    let mut rng = StdRng::seed_from_u64(42);
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rng);
    let train_idx: Vec<usize> = indices[..n_train].to_vec();
    let val_idx: Vec<usize> = indices[n_train..].to_vec();

    let mut best_val_loss = f64::INFINITY;
    let start = Instant::now();

    for epoch in 0..epochs {
        let clr = cosine_lr(lr, epoch, epochs, 0.01);
        opt.set_lr(clr);

        let mut perm = train_idx.clone();
        perm.shuffle(&mut rng);

        let mut epoch_loss = 0.0;
        let mut num_batches = 0;

        for batch_start in (0..perm.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(perm.len());
            let batch = &perm[batch_start..batch_end];
            let (feat, targets) = prepare_value_batch(samples, batch, score_mean, score_std);
            let pred = value_net.forward(&feat, true);
            let loss = pred.mse_loss(&targets, tch::Reduction::Mean);
            opt.backward_step(&loss);
            epoch_loss += f64::try_from(&loss).unwrap();
            num_batches += 1;
        }

        // Validation
        let val_loss = tch::no_grad(|| {
            let mut total = 0.0;
            let mut count = 0;
            for batch_start in (0..val_idx.len()).step_by(batch_size) {
                let batch_end = (batch_start + batch_size).min(val_idx.len());
                let batch = &val_idx[batch_start..batch_end];
                let (feat, targets) = prepare_value_batch(samples, batch, score_mean, score_std);
                let pred = value_net.forward(&feat, false);
                let loss = pred.mse_loss(&targets, tch::Reduction::Mean);
                total += f64::try_from(&loss).unwrap();
                count += 1;
            }
            if count > 0 { total / count as f64 } else { 0.0 }
        });

        if val_loss < best_val_loss {
            best_val_loss = val_loss;
            if let Err(e) = save_varstore(&vs, save_path) {
                eprintln!("Warning: failed to save value net: {}", e);
            }
        }

        if epoch % 10 == 0 || epoch == epochs - 1 {
            println!(
                "  Epoch {}/{}: train={:.4}, val={:.4}, lr={:.2e} ({:.0}s)",
                epoch + 1,
                epochs,
                epoch_loss / num_batches.max(1) as f64,
                val_loss,
                clr,
                start.elapsed().as_secs_f64()
            );
        }
    }

    println!("Value best val loss: {:.4}, saved to {}", best_val_loss, save_path);
    Ok(best_val_loss)
}

// ============================================================
// Benchmark
// ============================================================

fn play_game_with_policy(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    strategy: StrategyParams,
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let pos = if strategy.is_active() {
            let boosted =
                compute_boosted_logits(&plateau, tile, &deck, turn, policy_net, strategy);
            *legal
                .iter()
                .max_by(|&&a, &&b| boosted[a].partial_cmp(&boosted[b]).unwrap())
                .unwrap()
        } else {
            let ml = compute_masked_logits(&plateau, tile, &deck, turn, policy_net);
            ml.argmax(-1, false).int64_value(&[]) as usize
        };

        plateau.tiles[pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }
    result(&plateau)
}

fn run_benchmark(
    model_a_path: &str,
    model_b_path: &str,
    num_games: usize,
    seed: u64,
    strategy_a: StrategyParams,
    strategy_b: StrategyParams,
    size_a: (i64, usize, i64),
    size_b: (i64, usize, i64),
) -> Result<(f64, f64), Box<dyn Error>> {
    println!("\n=== Benchmark ({} games) ===", num_games);
    println!("  A: {} (embed={}, layers={}, heads={})", model_a_path, size_a.0, size_a.1, size_a.2);
    if strategy_a.is_active() {
        println!("    strategy: line={:.1}, row={:.1}", strategy_a.line_boost, strategy_a.row_boost);
    }
    println!("  B: {} (embed={}, layers={}, heads={})", model_b_path, size_b.0, size_b.1, size_b.2);
    if strategy_b.is_active() {
        println!("    strategy: line={:.1}, row={:.1}", strategy_b.line_boost, strategy_b.row_boost);
    }

    let (_vs_a, policy_a) = load_policy_sized(model_a_path, size_a.0, size_a.1, size_a.2)?;
    let (_vs_b, policy_b) = load_policy_sized(model_b_path, size_b.0, size_b.1, size_b.2)?;
    let _guard = tch::no_grad_guard();

    let mut rng = StdRng::seed_from_u64(seed);
    let mut scores_a = Vec::with_capacity(num_games);
    let mut scores_b = Vec::with_capacity(num_games);
    let start = Instant::now();

    for i in 0..num_games {
        let tiles = generate_tile_sequence(&mut rng);
        let sa = play_game_with_policy(&tiles, &policy_a, strategy_a);
        let sb = play_game_with_policy(&tiles, &policy_b, strategy_b);
        scores_a.push(sa);
        scores_b.push(sb);

        if (i + 1) % 100 == 0 {
            let avg_a: f64 = scores_a.iter().map(|&s| s as f64).sum::<f64>() / scores_a.len() as f64;
            let avg_b: f64 = scores_b.iter().map(|&s| s as f64).sum::<f64>() / scores_b.len() as f64;
            println!("  {} games: A={:.1}, B={:.1}", i + 1, avg_a, avg_b);
        }
    }

    let (avg_a, median_a, _, _) = score_stats(&scores_a);
    let (avg_b, median_b, _, _) = score_stats(&scores_b);

    let wins_a = scores_a.iter().zip(&scores_b).filter(|(&a, &b)| a > b).count();
    let wins_b = scores_a.iter().zip(&scores_b).filter(|(&a, &b)| b > a).count();
    let draws = num_games - wins_a - wins_b;

    println!("\nResults ({:.1}s):", start.elapsed().as_secs_f64());
    println!("  Model A: avg={:.1}, median={}", avg_a, median_a);
    println!("  Model B: avg={:.1}, median={}", avg_b, median_b);
    println!(
        "  Wins: A={} ({:.1}%), B={} ({:.1}%), draws={}",
        wins_a,
        wins_a as f64 / num_games as f64 * 100.0,
        wins_b,
        wins_b as f64 / num_games as f64 * 100.0,
        draws
    );

    Ok((avg_a, avg_b))
}

// ============================================================
// Loop
// ============================================================

fn run_loop(
    init_policy: &str,
    init_value: Option<&str>,
    max_generations: usize,
    games_per_gen: usize,
    temperature: f64,
    explore_turns: usize,
    epochs: usize,
    batch_size: usize,
    lr: f64,
    weight_decay: f64,
    benchmark_games: usize,
    accept_threshold: f64,
    data_dir: &str,
    output_dir: &str,
    seed: u64,
    strategy: StrategyParams,
) -> Result<(), Box<dyn Error>> {
    println!("{}", "=".repeat(60));
    println!("AlphaZero Self-Play Loop");
    println!("{}", "=".repeat(60));
    println!("Max generations: {}, Games/gen: {}", max_generations, games_per_gen);
    println!("Temperature: {}, Explore turns: {}", temperature, explore_turns);
    println!("Epochs: {}, Batch size: {}, LR: {}", epochs, batch_size, lr);
    println!("Benchmark games: {}, Accept threshold: {}", benchmark_games, accept_threshold);
    if strategy.is_active() {
        println!("Strategy: line_boost={:.1}, row_boost={:.1}", strategy.line_boost, strategy.row_boost);
    }

    fs::create_dir_all(data_dir)?;
    fs::create_dir_all(output_dir)?;

    let mut current_policy = init_policy.to_string();
    let mut current_value: Option<String> = init_value.map(|s| s.to_string());

    // History log
    let history_path = format!("{}/selfplay_history.csv", data_dir);
    let write_header = !Path::new(&history_path).exists();
    if write_header {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&history_path)?;
        writeln!(
            f,
            "generation,num_games,avg_score,policy_val_loss,value_val_loss,bench_avg_old,bench_avg_new,accepted"
        )?;
    }

    let loop_start = Instant::now();
    let gen_weight_schedule = [1.0, 0.5, 0.25];

    for gen in 0..max_generations {
        println!("\n{}", "=".repeat(60));
        println!("Generation {} / {}", gen + 1, max_generations);
        println!("{}", "=".repeat(60));

        // 1. Generate self-play data
        let data_path = format!("{}/gen{}.csv", data_dir, gen);
        let gen_seed = seed.wrapping_add(gen as u64 * 1_000_000);
        let scores = run_generate(
            &current_policy,
            games_per_gen,
            temperature,
            explore_turns,
            &data_path,
            gen_seed,
            strategy,
        )?;
        let avg_score: f64 = scores.iter().map(|&s| s as f64).sum::<f64>() / scores.len() as f64;

        // 2. Load training data (sliding window of last 3 generations)
        let mut all_samples: Vec<TurnRecord> = Vec::new();
        let mut all_weights: Vec<f32> = Vec::new();

        for (offset, &gw) in gen_weight_schedule.iter().enumerate() {
            if gen < offset {
                break;
            }
            let g = gen - offset;
            let path = format!("{}/gen{}.csv", data_dir, g);
            if !Path::new(&path).exists() {
                break;
            }
            let samples = load_csv(&path)?;
            let weights = compute_sample_weights(&samples, gw);
            let count = samples.len();
            all_samples.extend(samples);
            all_weights.extend(weights);
            println!("  Loaded gen {} ({} samples, weight={:.2})", g, count, gw);
        }
        println!("  Total training samples: {}", all_samples.len());

        // 3. Train policy
        let new_policy_path = format!(
            "{}/gt_selfplay_gen{}_policy.safetensors",
            output_dir,
            gen + 1
        );
        let policy_loss = train_policy(
            &all_samples,
            &all_weights,
            Some(&current_policy),
            &new_policy_path,
            epochs,
            batch_size,
            lr,
            weight_decay,
            128, 2, 4,
        )?;

        // 4. Train value
        let new_value_path = format!(
            "{}/gt_selfplay_gen{}_value.safetensors",
            output_dir,
            gen + 1
        );
        let value_loss = train_value(
            &all_samples,
            current_value.as_deref(),
            &new_value_path,
            epochs,
            batch_size,
            lr,
            weight_decay,
            128, 2, 4,
        )?;

        // 5. Benchmark new vs current
        let bench_seed = seed.wrapping_add(999_999 + gen as u64 * 1_000);
        let (avg_old, avg_new) =
            run_benchmark(&current_policy, &new_policy_path, benchmark_games, bench_seed, strategy, strategy, (128, 2, 4), (128, 2, 4))?;

        // 6. Accept/reject
        let accepted = avg_new > avg_old + accept_threshold;
        if accepted {
            println!(
                "\n>>> ACCEPTED gen {} ({:.1} > {:.1} + {:.1})",
                gen + 1,
                avg_new,
                avg_old,
                accept_threshold
            );
            current_policy = new_policy_path;
            current_value = Some(new_value_path);
        } else {
            println!(
                "\n>>> REJECTED gen {} ({:.1} <= {:.1} + {:.1})",
                gen + 1,
                avg_new,
                avg_old,
                accept_threshold
            );
        }

        // 7. Log to history
        {
            let mut f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&history_path)?;
            writeln!(
                f,
                "{},{},{:.1},{:.4},{:.4},{:.1},{:.1},{}",
                gen + 1,
                games_per_gen,
                avg_score,
                policy_loss,
                value_loss,
                avg_old,
                avg_new,
                accepted
            )?;
        }

        println!(
            "Elapsed: {:.0}s total",
            loop_start.elapsed().as_secs_f64()
        );
    }

    println!("\n{}", "=".repeat(60));
    println!("Loop complete!");
    println!("Final policy: {}", current_policy);
    if let Some(ref v) = current_value {
        println!("Final value:  {}", v);
    }
    println!(
        "Total time: {:.0}s",
        loop_start.elapsed().as_secs_f64()
    );

    Ok(())
}

// ============================================================
// Ensemble
// ============================================================

fn play_game_ensemble(
    tiles: &[Tile],
    policy_a: &GraphTransformerPolicyNet,
    policy_b: &GraphTransformerPolicyNet,
    weight_b: f64,
    strategy: StrategyParams,
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        // Get logits from both models
        let logits_a = compute_masked_logits(&plateau, tile, &deck, turn, policy_a);
        let logits_b = compute_masked_logits(&plateau, tile, &deck, turn, policy_b);

        // Combined: logits_a + weight_b * logits_b
        let combined = &logits_a + &logits_b * weight_b;
        let combined_vec: Vec<f64> = Vec::<f64>::try_from(&combined).unwrap();

        let pos = if strategy.is_active() {
            // Add heuristic boosts
            let boosted: Vec<f64> = combined_vec
                .iter()
                .enumerate()
                .map(|(pos, &l)| {
                    if l.is_finite() {
                        l + line_boost_heuristic(&plateau, tile, pos, strategy.line_boost)
                            + row_affinity_boost(&plateau, tile, pos, strategy.row_boost)
                    } else {
                        l
                    }
                })
                .collect();
            *legal
                .iter()
                .max_by(|&&a, &&b| boosted[a].partial_cmp(&boosted[b]).unwrap())
                .unwrap()
        } else {
            combined.argmax(-1, false).int64_value(&[]) as usize
        };

        plateau.tiles[pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }
    result(&plateau)
}

fn run_ensemble(
    model_a_path: &str,
    model_b_path: &str,
    num_games: usize,
    weight_b: f64,
    line_boost: f64,
    row_boost: f64,
    seed: u64,
) -> Result<(), Box<dyn Error>> {
    println!("{}", "=".repeat(60));
    println!("Ensemble Benchmark ({} games)", num_games);
    println!("{}", "=".repeat(60));
    println!("  Model A: {}", model_a_path);
    println!("  Model B: {}", model_b_path);
    println!("  weight_b: {:.2}", weight_b);
    if line_boost.abs() > 1e-9 || row_boost.abs() > 1e-9 {
        println!("  line_boost: {:.1}, row_boost: {:.1}", line_boost, row_boost);
    }
    println!();

    let (_vs_a, policy_a) = load_policy(model_a_path)?;
    let (_vs_b, policy_b) = load_policy(model_b_path)?;
    let _guard = tch::no_grad_guard();

    // Pre-generate tile sequences
    let mut rng = StdRng::seed_from_u64(seed);
    let tile_sequences: Vec<Vec<Tile>> = (0..num_games)
        .map(|_| generate_tile_sequence(&mut rng))
        .collect();

    let no_strategy = StrategyParams { line_boost: 0.0, row_boost: 0.0 };
    let boost_strategy = StrategyParams { line_boost, row_boost };

    let mut scores_a = Vec::with_capacity(num_games);
    let mut scores_b = Vec::with_capacity(num_games);
    let mut scores_ensemble = Vec::with_capacity(num_games);
    let mut scores_ensemble_boost = Vec::with_capacity(num_games);

    let start = Instant::now();

    for (i, tiles) in tile_sequences.iter().enumerate() {
        // Model A alone (no boost)
        let sa = play_game_with_policy(tiles, &policy_a, no_strategy);
        scores_a.push(sa);

        // Model B alone (no boost)
        let sb = play_game_with_policy(tiles, &policy_b, no_strategy);
        scores_b.push(sb);

        // Ensemble (no boost)
        let se = play_game_ensemble(tiles, &policy_a, &policy_b, weight_b, no_strategy);
        scores_ensemble.push(se);

        // Ensemble + boost
        let seb = if boost_strategy.is_active() {
            play_game_ensemble(tiles, &policy_a, &policy_b, weight_b, boost_strategy)
        } else {
            se // same as ensemble when no boost
        };
        scores_ensemble_boost.push(seb);

        if (i + 1) % 100 == 0 {
            let avg_a: f64 = scores_a.iter().map(|&s| s as f64).sum::<f64>() / scores_a.len() as f64;
            let avg_e: f64 = scores_ensemble.iter().map(|&s| s as f64).sum::<f64>() / scores_ensemble.len() as f64;
            let avg_eb: f64 = scores_ensemble_boost.iter().map(|&s| s as f64).sum::<f64>() / scores_ensemble_boost.len() as f64;
            println!(
                "  {} games ({:.1}s): A={:.1}, Ens={:.1}, Ens+boost={:.1}",
                i + 1,
                start.elapsed().as_secs_f64(),
                avg_a, avg_e, avg_eb
            );
        }
    }

    // Compute stats
    let (avg_a, median_a, min_a, max_a) = score_stats(&scores_a);
    let (avg_b, median_b, min_b, max_b) = score_stats(&scores_b);
    let (avg_e, median_e, min_e, max_e) = score_stats(&scores_ensemble);
    let (avg_eb, median_eb, min_eb, max_eb) = score_stats(&scores_ensemble_boost);

    // Wins vs model A
    let wins_e_vs_a = scores_ensemble.iter().zip(&scores_a).filter(|(&e, &a)| e > a).count();
    let wins_eb_vs_a = scores_ensemble_boost.iter().zip(&scores_a).filter(|(&e, &a)| e > a).count();

    println!("\n{}", "=".repeat(70));
    println!("Results ({} games, {:.1}s):", num_games, start.elapsed().as_secs_f64());
    println!("{}", "=".repeat(70));
    println!(
        "{:<25} {:>8} {:>8} {:>6} {:>6} {:>10}",
        "Strategy", "Avg", "Median", "Min", "Max", "Wins vs A"
    );
    println!("{}", "-".repeat(70));
    println!(
        "{:<25} {:>8.1} {:>8} {:>6} {:>6} {:>10}",
        "Model A (pure)", avg_a, median_a, min_a, max_a, "-"
    );
    println!(
        "{:<25} {:>8.1} {:>8} {:>6} {:>6} {:>10}",
        "Model B (pure)", avg_b, median_b, min_b, max_b, "-"
    );
    println!(
        "{:<25} {:>8.1} {:>8} {:>6} {:>6} {:>8} ({:.1}%)",
        format!("Ensemble (wb={:.1})", weight_b),
        avg_e, median_e, min_e, max_e,
        wins_e_vs_a,
        wins_e_vs_a as f64 / num_games as f64 * 100.0
    );
    if boost_strategy.is_active() {
        println!(
            "{:<25} {:>8.1} {:>8} {:>6} {:>6} {:>8} ({:.1}%)",
            format!("Ens+lb={:.0},rb={:.0}", line_boost, row_boost),
            avg_eb, median_eb, min_eb, max_eb,
            wins_eb_vs_a,
            wins_eb_vs_a as f64 / num_games as f64 * 100.0
        );
    }

    // Also show Model A + boost for reference
    if boost_strategy.is_active() {
        let mut scores_a_boost = Vec::with_capacity(num_games);
        for tiles in &tile_sequences {
            scores_a_boost.push(play_game_with_policy(tiles, &policy_a, boost_strategy));
        }
        let (avg_ab, median_ab, min_ab, max_ab) = score_stats(&scores_a_boost);
        println!(
            "{:<25} {:>8.1} {:>8} {:>6} {:>6} {:>10}",
            format!("A+lb={:.0},rb={:.0}", line_boost, row_boost),
            avg_ab, median_ab, min_ab, max_ab, "-"
        );
    }

    println!("{}", "=".repeat(70));

    Ok(())
}

// ============================================================
// Grid Search
// ============================================================

fn run_gridsearch(
    model_path: &str,
    num_games: usize,
    line_boosts: &[f64],
    row_boosts: &[f64],
    output_path: &str,
    seed: u64,
) -> Result<(), Box<dyn Error>> {
    let total_combos = line_boosts.len() * row_boosts.len();
    println!("=== Grid Search ({} combos x {} games) ===", total_combos, num_games);
    println!("Policy: {}", model_path);
    println!("Line boosts: {:?}", line_boosts);
    println!("Row boosts:  {:?}", row_boosts);
    println!();

    let (_vs, policy_net) = load_policy(model_path)?;
    let _guard = tch::no_grad_guard();

    // Pre-generate all tile sequences for fair comparison
    let mut rng = StdRng::seed_from_u64(seed);
    let tile_sequences: Vec<Vec<Tile>> = (0..num_games)
        .map(|_| generate_tile_sequence(&mut rng))
        .collect();

    struct GridResult {
        line_b: f64,
        row_b: f64,
        avg: f64,
        median: i32,
        std: f64,
        min: i32,
        max: i32,
        avg_completions: f64,
    }

    let mut results: Vec<GridResult> = Vec::with_capacity(total_combos);
    let start = Instant::now();

    for (combo_idx, &lb) in line_boosts.iter().enumerate() {
        for &rb in row_boosts.iter() {
            let strategy = StrategyParams {
                line_boost: lb,
                row_boost: rb,
            };

            let mut scores = Vec::with_capacity(num_games);
            let mut total_completions = 0usize;

            for tiles in &tile_sequences {
                let (score, lc) = play_game_with_strategy_stats(tiles, &policy_net, strategy);
                scores.push(score);
                total_completions += lc.total();
            }

            let (avg, median, min, max) = score_stats(&scores);
            let mean = avg;
            let variance = scores.iter().map(|&s| (s as f64 - mean).powi(2)).sum::<f64>()
                / scores.len() as f64;
            let std = variance.sqrt();
            let avg_completions = total_completions as f64 / num_games as f64;

            let combo_num = combo_idx * row_boosts.len()
                + row_boosts.iter().position(|&r| (r - rb).abs() < 1e-9).unwrap()
                + 1;
            println!(
                "  [{:>3}/{}] line={:.1} row={:.1} => avg={:.1} med={} std={:.1} min={} max={} lines={:.1} ({:.1}s)",
                combo_num, total_combos, lb, rb, avg, median, std, min, max, avg_completions,
                start.elapsed().as_secs_f64()
            );

            results.push(GridResult {
                line_b: lb,
                row_b: rb,
                avg,
                median,
                std,
                min,
                max,
                avg_completions,
            });
        }
    }

    // Sort by avg score descending
    results.sort_by(|a, b| b.avg.partial_cmp(&a.avg).unwrap());

    // Print top results
    println!("\n{}", "=".repeat(80));
    println!("Top 10 configurations (sorted by avg score):");
    println!("{}", "=".repeat(80));
    println!(
        "{:>6} {:>6} | {:>9} {:>6} {:>6} {:>5} {:>5} | {:>5}",
        "line_b", "row_b", "avg", "median", "std", "min", "max", "lines"
    );
    println!(
        "{:-<6} {:-<6}-+-{:-<9}-{:-<6}-{:-<6}-{:-<5}-{:-<5}-+-{:-<5}",
        "", "", "", "", "", "", "", ""
    );
    for (i, r) in results.iter().enumerate().take(10) {
        println!(
            "{:>6.1} {:>6.1} | {:>9.2} {:>6} {:>6.1} {:>5} {:>5} | {:>5.1}{}",
            r.line_b, r.row_b, r.avg, r.median, r.std, r.min, r.max, r.avg_completions,
            if i == 0 { "  <-- BEST" } else { "" }
        );
    }

    // Save CSV
    {
        let mut f = fs::File::create(output_path)?;
        writeln!(f, "line_boost,row_boost,avg,median,std,min,max,avg_completions")?;
        for r in &results {
            writeln!(
                f,
                "{:.1},{:.1},{:.2},{},{:.1},{},{},{:.2}",
                r.line_b, r.row_b, r.avg, r.median, r.std, r.min, r.max, r.avg_completions
            )?;
        }
    }
    println!("\nResults saved to {}", output_path);
    println!(
        "Total time: {:.1}s ({:.1}s per combo)",
        start.elapsed().as_secs_f64(),
        start.elapsed().as_secs_f64() / total_combos as f64
    );

    // Print recommendation
    let best = &results[0];
    println!(
        "\nRecommended: --line-boost {:.1} --row-boost {:.1} (avg={:.2})",
        best.line_b, best.row_b, best.avg
    );

    Ok(())
}

// ============================================================
// Expectimax
// ============================================================

/// Evaluate a board state using the value net.
/// Averages value_net over all remaining tiles in the deck as "current tile"
/// to stay in-distribution (the value net was trained with real tiles).
fn evaluate_board(
    plateau: &Plateau,
    deck: &Deck,
    turn: usize,
    value_net: &GraphTransformerValueNet,
) -> f64 {
    let remaining = get_available_tiles(deck);
    if remaining.is_empty() {
        // Terminal state: return normalized actual score
        let score = result(plateau) as f64;
        return ((score - 140.0) / 40.0).clamp(-1.0, 1.0);
    }

    // Batch all remaining tiles into one forward pass
    let features: Vec<Tensor> = remaining.iter()
        .map(|tile| convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19))
        .collect();
    let batch = Tensor::stack(&features, 0); // [N, 19, 47]
    let values = value_net.forward(&batch, false); // [N, 1]
    f64::try_from(&values.mean(Kind::Float)).unwrap()
}

/// Play a game using expectimax search.
/// depth=0: value-guided (evaluate each legal placement with value_net)
/// depth=1: for each legal placement, average value over all possible next tiles
///          (using policy to choose placement for each next tile)
fn play_game_expectimax(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    depth: usize,
    line_boost: f64,
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let best_pos = if legal.len() == 1 {
            legal[0]
        } else {
            let mut best_eval = f64::NEG_INFINITY;
            let mut best = legal[0];

            for &pos in &legal {
                // Temporarily place tile
                plateau.tiles[pos] = *tile;
                let new_deck = replace_tile_in_deck(&deck, tile);

                let eval = if depth == 0 {
                    // 0-ply: just evaluate the board after placement
                    evaluate_board(&plateau, &new_deck, turn + 1, value_net)
                } else {
                    // 1-ply: average over all remaining tiles
                    let remaining = get_available_tiles(&new_deck);
                    if remaining.is_empty() {
                        // No tiles left, this is the final state
                        result(&plateau) as f64
                    } else {
                        let mut sum_eval = 0.0;
                        for next_tile in &remaining {
                            let next_legal = get_legal_moves(&plateau);
                            if next_legal.is_empty() {
                                sum_eval += result(&plateau) as f64;
                                continue;
                            }

                            // Find best position for next_tile using policy
                            let next_pos = if line_boost > 0.0 {
                                let strategy = StrategyParams { line_boost, row_boost: 0.0 };
                                let boosted = compute_boosted_logits(
                                    &plateau, next_tile, &new_deck, turn + 1,
                                    policy_net, strategy,
                                );
                                *next_legal.iter()
                                    .max_by(|&&a, &&b| boosted[a].partial_cmp(&boosted[b]).unwrap())
                                    .unwrap()
                            } else {
                                let ml = compute_masked_logits(
                                    &plateau, next_tile, &new_deck, turn + 1, policy_net,
                                );
                                ml.argmax(-1, false).int64_value(&[]) as usize
                            };

                            // Place next tile, evaluate, unplace
                            plateau.tiles[next_pos] = *next_tile;
                            let deck2 = replace_tile_in_deck(&new_deck, next_tile);
                            let v = evaluate_board(&plateau, &deck2, turn + 2, value_net);
                            sum_eval += v;
                            plateau.tiles[next_pos] = Tile(0, 0, 0);
                        }
                        sum_eval / remaining.len() as f64
                    }
                };

                // Unplace tile
                plateau.tiles[pos] = Tile(0, 0, 0);

                if eval > best_eval {
                    best_eval = eval;
                    best = pos;
                }
            }
            best
        };

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn run_expectimax(
    model_policy_path: &str,
    model_value_path: &str,
    num_games: usize,
    depth: usize,
    line_boost: f64,
    seed: u64,
    embed_dim: i64,
    num_layers: usize,
    num_heads: i64,
) -> Result<(), Box<dyn Error>> {
    println!("{}", "=".repeat(60));
    println!("Expectimax Benchmark ({} games, depth={})", num_games, depth);
    println!("{}", "=".repeat(60));
    println!("Policy: {}", model_policy_path);
    println!("Value:  {}", model_value_path);
    if line_boost > 0.0 {
        println!("Line boost: {:.1}", line_boost);
    }
    println!("Architecture: embed={}, layers={}, heads={}", embed_dim, num_layers, num_heads);
    println!();

    let (_vs_p, policy_net) = load_policy_sized(model_policy_path, embed_dim, num_layers, num_heads)?;
    let (_vs_v, value_net) = load_value_sized(model_value_path, embed_dim, num_layers, num_heads)?;
    let _guard = tch::no_grad_guard();

    let mut rng = StdRng::seed_from_u64(seed);
    let tile_sequences: Vec<Vec<Tile>> = (0..num_games)
        .map(|_| generate_tile_sequence(&mut rng))
        .collect();

    let no_strategy = StrategyParams { line_boost: 0.0, row_boost: 0.0 };
    let boost_strategy = StrategyParams { line_boost, row_boost: 0.0 };

    let mut scores_pure = Vec::with_capacity(num_games);
    let mut scores_boost = Vec::with_capacity(num_games);
    let mut scores_value = Vec::with_capacity(num_games);
    let mut scores_expectimax = Vec::with_capacity(num_games);

    let start = Instant::now();

    for (i, tiles) in tile_sequences.iter().enumerate() {
        // 1. Policy pure
        scores_pure.push(play_game_with_policy(tiles, &policy_net, no_strategy));

        // 2. Policy + line_boost
        if line_boost > 0.0 {
            scores_boost.push(play_game_with_policy(tiles, &policy_net, boost_strategy));
        }

        // 3. Value-guided (0-ply)
        scores_value.push(play_game_expectimax(tiles, &policy_net, &value_net, 0, line_boost));

        // 4. Expectimax (requested depth) - only if depth >= 1
        if depth >= 1 {
            scores_expectimax.push(play_game_expectimax(
                tiles, &policy_net, &value_net, depth, line_boost,
            ));
        }

        if (i + 1) % 10 == 0 {
            let avg_p = scores_pure.iter().map(|&s| s as f64).sum::<f64>() / scores_pure.len() as f64;
            let avg_v = scores_value.iter().map(|&s| s as f64).sum::<f64>() / scores_value.len() as f64;
            print!(
                "  {} games ({:.1}s): pure={:.1}, value-0ply={:.1}",
                i + 1, start.elapsed().as_secs_f64(), avg_p, avg_v,
            );
            if depth >= 1 && !scores_expectimax.is_empty() {
                let avg_e = scores_expectimax.iter().map(|&s| s as f64).sum::<f64>()
                    / scores_expectimax.len() as f64;
                print!(", expect-{}ply={:.1}", depth, avg_e);
            }
            println!();
        }
    }

    // Print results table
    println!("\n{}", "=".repeat(70));
    println!(
        "Results ({} games, {:.1}s):",
        num_games,
        start.elapsed().as_secs_f64()
    );
    println!("{}", "=".repeat(70));
    println!(
        "{:<30} {:>8} {:>8} {:>6} {:>6}",
        "Strategy", "Avg", "Median", "Min", "Max"
    );
    println!("{}", "-".repeat(70));

    let (avg, med, min, max) = score_stats(&scores_pure);
    println!("{:<30} {:>8.1} {:>8} {:>6} {:>6}", "Policy pure", avg, med, min, max);

    if line_boost > 0.0 {
        let (avg, med, min, max) = score_stats(&scores_boost);
        println!(
            "{:<30} {:>8.1} {:>8} {:>6} {:>6}",
            format!("Policy + lb={:.0}", line_boost), avg, med, min, max
        );
    }

    let (avg, med, min, max) = score_stats(&scores_value);
    println!("{:<30} {:>8.1} {:>8} {:>6} {:>6}", "Value-guided (0-ply)", avg, med, min, max);

    if depth >= 1 {
        let (avg, med, min, max) = score_stats(&scores_expectimax);
        println!(
            "{:<30} {:>8.1} {:>8} {:>6} {:>6}",
            format!("Expectimax ({}-ply)", depth), avg, med, min, max
        );
    }

    println!("{}", "=".repeat(70));

    Ok(())
}

// ============================================================
// Main
// ============================================================

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            model_path,
            num_games,
            temperature,
            explore_turns,
            data_dir,
            generation,
            seed,
            line_boost,
            row_boost,
        } => {
            let save_path = format!("{}/gen{}.csv", data_dir, generation);
            let strategy = StrategyParams { line_boost, row_boost };
            run_generate(
                &model_path,
                num_games,
                temperature,
                explore_turns,
                &save_path,
                seed.unwrap_or(42),
                strategy,
            )?;
        }

        Commands::Train {
            data_dir,
            generations,
            gen_weights,
            init_policy,
            init_value,
            epochs,
            batch_size,
            lr,
            weight_decay,
            output_dir,
            output_gen,
            skip_value,
            skip_policy,
            min_score,
            embed_dim,
            num_layers,
            num_heads,
        } => {
            let output_gen = output_gen.unwrap_or_else(|| generations.iter().max().unwrap() + 1);
            let gw = gen_weights.unwrap_or_else(|| vec![1.0; generations.len()]);
            assert_eq!(
                gw.len(),
                generations.len(),
                "--gen-weights length must match --generations length"
            );

            let mut all_samples = Vec::new();
            let mut all_weights = Vec::new();

            for (i, &gen) in generations.iter().enumerate() {
                let path = format!("{}/gen{}.csv", data_dir, gen);
                println!("Loading {} (weight={:.2})", path, gw[i]);
                let samples = load_csv(&path)?;
                let weights = compute_sample_weights(&samples, gw[i]);
                all_samples.extend(samples);
                all_weights.extend(weights);
            }
            println!("Total samples: {}", all_samples.len());

            if let Some(ms) = min_score {
                let before = all_samples.len();
                all_samples.retain(|s| s.final_score >= ms);
                // Recompute weights based on filtered set
                all_weights = compute_sample_weights(&all_samples, 1.0);
                println!(
                    "Filtered by min_score >= {}: {} -> {} samples ({} games removed)",
                    ms,
                    before,
                    all_samples.len(),
                    (before - all_samples.len()) / 19
                );
            }

            fs::create_dir_all(&output_dir)?;

            if !skip_policy {
                let save = format!(
                    "{}/gt_selfplay_gen{}_policy.safetensors",
                    output_dir, output_gen
                );
                train_policy(
                    &all_samples,
                    &all_weights,
                    init_policy.as_deref(),
                    &save,
                    epochs,
                    batch_size,
                    lr,
                    weight_decay,
                    embed_dim,
                    num_layers,
                    num_heads,
                )?;
            }

            if !skip_value {
                let save = format!(
                    "{}/gt_selfplay_gen{}_value.safetensors",
                    output_dir, output_gen
                );
                train_value(
                    &all_samples,
                    init_value.as_deref(),
                    &save,
                    epochs,
                    batch_size,
                    lr,
                    weight_decay,
                    embed_dim,
                    num_layers,
                    num_heads,
                )?;
            }
        }

        Commands::Benchmark {
            model_a,
            model_b,
            num_games,
            seed,
            line_boost_a,
            row_boost_a,
            line_boost_b,
            row_boost_b,
            embed_dim_a,
            num_layers_a,
            num_heads_a,
            embed_dim_b,
            num_layers_b,
            num_heads_b,
        } => {
            let strategy_a = StrategyParams { line_boost: line_boost_a, row_boost: row_boost_a };
            let strategy_b = StrategyParams { line_boost: line_boost_b, row_boost: row_boost_b };
            run_benchmark(
                &model_a, &model_b, num_games, seed.unwrap_or(42),
                strategy_a, strategy_b,
                (embed_dim_a, num_layers_a, num_heads_a),
                (embed_dim_b, num_layers_b, num_heads_b),
            )?;
        }

        Commands::Loop {
            init_policy,
            init_value,
            max_generations,
            games_per_gen,
            temperature,
            explore_turns,
            epochs,
            batch_size,
            lr,
            weight_decay,
            benchmark_games,
            accept_threshold,
            data_dir,
            output_dir,
            seed,
            line_boost,
            row_boost,
        } => {
            let strategy = StrategyParams { line_boost, row_boost };
            run_loop(
                &init_policy,
                init_value.as_deref(),
                max_generations,
                games_per_gen,
                temperature,
                explore_turns,
                epochs,
                batch_size,
                lr,
                weight_decay,
                benchmark_games,
                accept_threshold,
                &data_dir,
                &output_dir,
                seed.unwrap_or(42),
                strategy,
            )?;
        }

        Commands::Ensemble {
            model_a,
            model_b,
            num_games,
            weight_b,
            line_boost,
            row_boost,
            seed,
        } => {
            run_ensemble(
                &model_a,
                &model_b,
                num_games,
                weight_b,
                line_boost,
                row_boost,
                seed.unwrap_or(42),
            )?;
        }

        Commands::Gridsearch {
            model_path,
            num_games,
            line_boosts,
            row_boosts,
            output,
            seed,
        } => {
            let lb: Vec<f64> = line_boosts
                .split(',')
                .map(|s| s.trim().parse::<f64>().expect("Invalid line_boost value"))
                .collect();
            let rb: Vec<f64> = row_boosts
                .split(',')
                .map(|s| s.trim().parse::<f64>().expect("Invalid row_boost value"))
                .collect();
            run_gridsearch(&model_path, num_games, &lb, &rb, &output, seed.unwrap_or(42))?;
        }

        Commands::Expectimax {
            model_policy,
            model_value,
            num_games,
            depth,
            line_boost,
            seed,
            embed_dim,
            num_layers,
            num_heads,
        } => {
            run_expectimax(
                &model_policy,
                &model_value,
                num_games,
                depth,
                line_boost,
                seed.unwrap_or(42),
                embed_dim,
                num_layers,
                num_heads,
            )?;
        }
    }

    Ok(())
}
