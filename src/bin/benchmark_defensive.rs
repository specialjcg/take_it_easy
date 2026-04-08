//! Benchmark: V2 Distilled vs V2 + Defensive Block Penalty
//!
//! Tests whether penalizing line-blocking placements reduces catastrophic games.
//! Can replay specific tile sequences (from recorded losses) and run random games.

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::io::Write;
use tch::{nn, Device, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::device_util::parse_device;
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::load_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::gt_boost::LINES;

#[derive(Parser, Debug)]
#[command(name = "benchmark_defensive")]
#[command(about = "Benchmark V2 + defensive strategies including ensemble")]
struct Args {
    #[arg(long, default_value = "cpu")]
    device: String,

    #[arg(long, default_value = "model_weights/distilled_v2_policy.safetensors")]
    policy_path: String,

    /// Path to defensive/V3 model for ensemble mode
    #[arg(long, default_value = "/tmp/distilled_v3_boosted")]
    defensive_path: String,

    #[arg(long, default_value_t = 128)]
    embed_dim: i64,

    #[arg(long, default_value_t = 2)]
    num_layers: usize,

    #[arg(long, default_value_t = 4)]
    num_heads: i64,

    /// Number of random games
    #[arg(long, default_value_t = 500)]
    num_games: usize,

    /// Lambda values to test (comma-separated)
    #[arg(long, default_value = "0.0,0.5,1.0,2.0,3.0,5.0")]
    lambdas: String,

    #[arg(long, default_value_t = 42)]
    seed: u64,
}

/// Count total blocked lines on current plateau (not new blocks, total).
fn count_total_blocked_lines(plateau: &Plateau) -> usize {
    let mut blocked = 0;
    for &(line_positions, _direction) in &LINES {
        let mut filled: Vec<[i32; 3]> = Vec::new();
        for &pos in line_positions {
            let t = &plateau.tiles[pos];
            if t.0 != 0 || t.1 != 0 || t.2 != 0 {
                filled.push([t.0, t.1, t.2]);
            }
        }
        if filled.len() < 2 {
            continue;
        }
        let all_blocked = (0..3usize).all(|face| {
            let vals: Vec<i32> = filled.iter().map(|t| t[face]).collect();
            vals.iter().any(|v| *v != vals[0])
        });
        if all_blocked {
            blocked += 1;
        }
    }
    blocked
}

/// Expected blocked lines threshold per turn (from analysis of normal games).
/// If actual blocked > expected + margin → switch to defensive.
fn blocked_threshold(turn: usize, margin: f64) -> f64 {
    // Approximate average blocked lines per turn from normal games
    let expected: [f64; 19] = [
        0.0, 0.0, 0.0, 0.1, 0.2, 0.4, 0.6, 0.9,  // turns 0-7
        1.2, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5,   // turns 8-15
        5.0, 5.5, 6.0,                               // turns 16-18
    ];
    expected[turn.min(18)] + margin
}

/// Play one game with ensemble: V2 by default, switch to defensive when in danger.
fn play_ensemble(
    v2_net: &GraphTransformerPolicyNet,
    v3_net: &GraphTransformerPolicyNet,
    device: Device,
    tile_sequence: &[Tile],
    margin: f64,
    min_switch_turn: usize,
) -> (i32, usize) {  // (score, number of turns using defensive)
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut defensive_turns = 0;

    for (turn, &tile) in tile_sequence.iter().enumerate() {
        deck = replace_tile_in_deck(&deck, &tile);
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        // Decide which model to use
        let use_defensive = turn >= min_switch_turn
            && count_total_blocked_lines(&plateau) as f64 > blocked_threshold(turn, margin);

        let net = if use_defensive {
            defensive_turns += 1;
            v3_net
        } else {
            v2_net
        };

        let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19)
            .unsqueeze(0)
            .to_device(device);
        let logits = tch::no_grad(|| net.forward(&feat, false))
            .squeeze_dim(0)
            .to_device(Device::Cpu);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&logits).unwrap();

        let pos = *legal
            .iter()
            .max_by(|&&a, &&b| logit_values[a].partial_cmp(&logit_values[b]).unwrap())
            .unwrap();

        plateau.tiles[pos] = tile;
    }

    (result(&plateau), defensive_turns)
}

/// Simple block count (used for basic penalty).
fn count_new_blocked_lines(plateau: &Plateau, tile: &Tile, position: usize) -> usize {
    let tile_faces = [tile.0, tile.1, tile.2];
    let mut new_blocked = 0;

    for &(line_positions, _direction) in &LINES {
        if !line_positions.contains(&position) {
            continue;
        }

        let mut filled_before: Vec<[i32; 3]> = Vec::new();
        for &pos in line_positions {
            if pos == position {
                continue;
            }
            let t = &plateau.tiles[pos];
            if t.0 != 0 || t.1 != 0 || t.2 != 0 {
                filled_before.push([t.0, t.1, t.2]);
            }
        }

        if filled_before.is_empty() {
            continue;
        }

        let was_blocked = (0..3usize).all(|face| {
            let vals: Vec<i32> = filled_before.iter().map(|t| t[face]).collect();
            vals.iter().any(|v| *v != vals[0])
        });

        if was_blocked {
            continue;
        }

        let mut all_filled: Vec<[i32; 3]> = filled_before;
        all_filled.push(tile_faces);

        let now_blocked = (0..3usize).all(|face| {
            let vals: Vec<i32> = all_filled.iter().map(|t| t[face]).collect();
            vals.iter().any(|v| *v != vals[0])
        });

        if now_blocked {
            new_blocked += 1;
        }
    }

    new_blocked
}

/// Weighted penalty: considers how promising the killed line was.
/// penalty = Σ (coherent_filled / line_length) * (best_face_value * line_length / 45)
/// This heavily penalizes killing a 4/5 line worth 9×5=45 pts,
/// but barely penalizes killing a 1/3 line worth 1×3=3 pts.
fn weighted_block_penalty(plateau: &Plateau, tile: &Tile, position: usize) -> f64 {
    let tile_faces = [tile.0, tile.1, tile.2];
    let mut total_penalty = 0.0;

    for &(line_positions, _direction) in &LINES {
        if !line_positions.contains(&position) {
            continue;
        }

        let line_len = line_positions.len() as f64;

        // Collect filled tiles BEFORE placement
        let mut filled_before: Vec<[i32; 3]> = Vec::new();
        for &pos in line_positions {
            if pos == position {
                continue;
            }
            let t = &plateau.tiles[pos];
            if t.0 != 0 || t.1 != 0 || t.2 != 0 {
                filled_before.push([t.0, t.1, t.2]);
            }
        }

        if filled_before.is_empty() {
            continue;
        }

        // Find which face(s) were still consistent, and their value
        let mut best_consistent_value = 0i32;
        let mut was_any_consistent = false;
        for face in 0..3usize {
            let vals: Vec<i32> = filled_before.iter().map(|t| t[face]).collect();
            if vals.iter().all(|v| *v == vals[0]) {
                was_any_consistent = true;
                best_consistent_value = best_consistent_value.max(vals[0]);
            }
        }

        if !was_any_consistent {
            continue; // Already blocked
        }

        // Check if this tile kills ALL consistent faces
        let mut all_filled: Vec<[i32; 3]> = filled_before.clone();
        all_filled.push(tile_faces);

        let now_blocked = (0..3usize).all(|face| {
            let vals: Vec<i32> = all_filled.iter().map(|t| t[face]).collect();
            vals.iter().any(|v| *v != vals[0])
        });

        if now_blocked {
            let progress = filled_before.len() as f64 / line_len;
            let potential_score = best_consistent_value as f64 * line_len / 45.0;
            // penalty = progress^2 * potential_score
            // squaring progress makes it much heavier for near-complete lines
            total_penalty += progress * progress * potential_score;
        }
    }

    total_penalty
}

/// Flexibility score: count how many lines through this position remain open after placement.
/// Higher = more future options preserved.
fn flexibility_score(plateau: &Plateau, tile: &Tile, position: usize) -> f64 {
    let tile_faces = [tile.0, tile.1, tile.2];
    let mut open_lines = 0.0;

    for &(line_positions, _direction) in &LINES {
        if !line_positions.contains(&position) {
            continue;
        }

        // After placing: check if at least one face is still consistent
        let mut all_filled: Vec<[i32; 3]> = Vec::new();
        for &pos in line_positions {
            let t = if pos == position {
                tile_faces
            } else {
                let pt = &plateau.tiles[pos];
                if pt.0 == 0 && pt.1 == 0 && pt.2 == 0 {
                    continue;
                }
                [pt.0, pt.1, pt.2]
            };
            all_filled.push(t);
        }

        for face in 0..3usize {
            let vals: Vec<i32> = all_filled.iter().map(|t| t[face]).collect();
            if vals.iter().all(|v| *v == vals[0]) {
                let potential = vals[0] as f64 * line_positions.len() as f64 / 45.0;
                open_lines += potential;
                break;
            }
        }
    }

    open_lines
}

#[derive(Clone, Copy)]
enum PenaltyMode {
    None,
    Simple(f64),       // lambda * count_new_blocked
    Weighted(f64),     // lambda * weighted_block_penalty (progress^2 * value)
    Flexibility(f64),  // lambda * flexibility_score (reward open lines)
    WeightedFlex(f64, f64), // penalty_lambda * weighted + flex_lambda * flexibility
    /// Turn-dependent: (w_lambda, f_lambda, start_turn, end_turn)
    /// Only active between start_turn..end_turn, linearly fades
    TurnDependent(f64, f64, usize, usize),
}

fn compute_bonus(plateau: &Plateau, tile: &Tile, pos: usize, turn: usize, mode: PenaltyMode) -> f64 {
    match mode {
        PenaltyMode::None => 0.0,
        PenaltyMode::Simple(l) => -l * count_new_blocked_lines(plateau, tile, pos) as f64,
        PenaltyMode::Weighted(l) => -l * weighted_block_penalty(plateau, tile, pos),
        PenaltyMode::Flexibility(l) => l * flexibility_score(plateau, tile, pos),
        PenaltyMode::WeightedFlex(lp, lf) => {
            -lp * weighted_block_penalty(plateau, tile, pos)
            + lf * flexibility_score(plateau, tile, pos)
        }
        PenaltyMode::TurnDependent(lp, lf, start, end) => {
            if turn < start || turn > end {
                return 0.0;
            }
            // Linear fade: full strength at start, zero at end
            let progress = (end - turn) as f64 / (end - start) as f64;
            let scale = progress;
            scale * (
                -lp * weighted_block_penalty(plateau, tile, pos)
                + lf * flexibility_score(plateau, tile, pos)
            )
        }
    }
}

/// Play one game with V2 + optional defensive strategy.
fn play_with_mode(
    policy_net: &GraphTransformerPolicyNet,
    device: Device,
    tile_sequence: &[Tile],
    mode: PenaltyMode,
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, &tile) in tile_sequence.iter().enumerate() {
        deck = replace_tile_in_deck(&deck, &tile);
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19)
            .unsqueeze(0)
            .to_device(device);
        let logits = tch::no_grad(|| policy_net.forward(&feat, false))
            .squeeze_dim(0)
            .to_device(Device::Cpu);
        let logit_values: Vec<f64> = Vec::<f64>::try_from(&logits).unwrap();

        let pos = *legal
            .iter()
            .max_by(|&&a, &&b| {
                let bonus_a = compute_bonus(&plateau, &tile, a, turn, mode);
                let bonus_b = compute_bonus(&plateau, &tile, b, turn, mode);
                let sa = logit_values[a] + bonus_a;
                let sb = logit_values[b] + bonus_b;
                sa.partial_cmp(&sb).unwrap()
            })
            .unwrap();

        plateau.tiles[pos] = tile;
    }

    result(&plateau)
}

fn random_tile_sequence(rng: &mut StdRng) -> Vec<Tile> {
    let deck = create_deck();
    let mut available: Vec<Tile> = deck
        .tiles()
        .iter()
        .copied()
        .filter(|t| *t != Tile(0, 0, 0))
        .collect();
    let mut seq = Vec::with_capacity(19);
    for _ in 0..19 {
        if available.is_empty() {
            break;
        }
        let idx = rng.gen_range(0..available.len());
        seq.push(available.remove(idx));
    }
    seq
}

fn avg(scores: &[i32]) -> f64 {
    scores.iter().sum::<i32>() as f64 / scores.len() as f64
}

fn std_dev(scores: &[i32]) -> f64 {
    let m = avg(scores);
    let var = scores.iter().map(|&s| (s as f64 - m).powi(2)).sum::<f64>() / scores.len() as f64;
    var.sqrt()
}

fn main() {
    let args = Args::parse();
    let device = parse_device(&args.device).unwrap_or(Device::Cpu);

    let lambdas: Vec<f64> = args.lambdas.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  V2 + Defensive Block Penalty Benchmark                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Load policy
    let mut vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(
        &vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
    );
    print!("Loading V2 policy... ");
    std::io::stdout().flush().ok();
    if let Err(e) = load_varstore(&mut vs, &args.policy_path) {
        eprintln!("FAILED: {}", e);
        return;
    }
    println!("OK");

    // Load defensive/V3 model for ensemble
    let mut v3_vs = nn::VarStore::new(device);
    let v3_net = GraphTransformerPolicyNet::new(
        &v3_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
    );
    let has_v3 = if std::path::Path::new(&args.defensive_path).exists() {
        print!("Loading V3 defensive... ");
        std::io::stdout().flush().ok();
        match load_varstore(&mut v3_vs, &args.defensive_path) {
            Ok(_) => { println!("OK"); true }
            Err(e) => { println!("FAILED: {} (ensemble disabled)", e); false }
        }
    } else {
        println!("No defensive model found at {} (ensemble disabled)", args.defensive_path);
        false
    };

    // Generate random sequences
    let mut rng = StdRng::seed_from_u64(args.seed);
    let sequences: Vec<Vec<Tile>> = (0..args.num_games)
        .map(|_| random_tile_sequence(&mut rng))
        .collect();

    // Define strategies to test
    let strategies: Vec<(&str, PenaltyMode)> = vec![
        ("V2 Pure", PenaltyMode::None),
        // --- Fine grid around W8+F2 sweet spot ---
        ("W6+F1", PenaltyMode::WeightedFlex(6.0, 1.0)),
        ("W6+F2", PenaltyMode::WeightedFlex(6.0, 2.0)),
        ("W8+F1", PenaltyMode::WeightedFlex(8.0, 1.0)),
        ("W8+F2", PenaltyMode::WeightedFlex(8.0, 2.0)),
        ("W8+F3", PenaltyMode::WeightedFlex(8.0, 3.0)),
        ("W10+F2", PenaltyMode::WeightedFlex(10.0, 2.0)),
        ("W10+F3", PenaltyMode::WeightedFlex(10.0, 3.0)),
        ("W12+F2", PenaltyMode::WeightedFlex(12.0, 2.0)),
        ("W12+F3", PenaltyMode::WeightedFlex(12.0, 3.0)),
        // --- Turn-dependent: active only turns 0-12 (fades out) ---
        ("TD W8F2 0-12", PenaltyMode::TurnDependent(8.0, 2.0, 0, 12)),
        ("TD W8F2 0-14", PenaltyMode::TurnDependent(8.0, 2.0, 0, 14)),
        ("TD W10F2 0-12", PenaltyMode::TurnDependent(10.0, 2.0, 0, 12)),
        ("TD W10F3 0-12", PenaltyMode::TurnDependent(10.0, 3.0, 0, 12)),
        ("TD W12F3 0-12", PenaltyMode::TurnDependent(12.0, 3.0, 0, 12)),
        // --- Turn-dependent: active only turns 3-14 (skip first 3) ---
        ("TD W8F2 3-14", PenaltyMode::TurnDependent(8.0, 2.0, 3, 14)),
        ("TD W10F3 3-14", PenaltyMode::TurnDependent(10.0, 3.0, 3, 14)),
    ];

    // Benchmark
    println!("\n=== RANDOM GAMES ({}) ===\n", args.num_games);
    println!(
        "{:<20} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Strategy", "Avg", "Std", "Min", "Max", "<110", "Delta"
    );
    println!("{}", "-".repeat(76));

    let mut baseline_avg = 0.0;

    for (label, mode) in &strategies {
        print!("  {}...", label);
        std::io::stdout().flush().ok();

        let scores: Vec<i32> = sequences
            .iter()
            .map(|seq| play_with_mode(&policy_net, device, seq, *mode))
            .collect();

        let a = avg(&scores);
        let s = std_dev(&scores);
        let mn = *scores.iter().min().unwrap();
        let mx = *scores.iter().max().unwrap();
        let below_110 = scores.iter().filter(|&&s| s < 110).count();

        if matches!(mode, PenaltyMode::None) {
            baseline_avg = a;
            println!(
                "\r{:<20} {:>8.1} {:>8.1} {:>8} {:>8} {:>8}",
                label, a, s, mn, mx, below_110
            );
        } else {
            println!(
                "\r{:<20} {:>8.1} {:>8.1} {:>8} {:>8} {:>8} {:>+8.1}",
                label, a, s, mn, mx, below_110, a - baseline_avg
            );
        }
    }

    println!("{}", "=".repeat(76));

    // Now replay the known AI loss sequences (hardcoded from analysis)
    println!("\n=== REPLAY AI LOSSES (tile sequences from recorded games) ===\n");

    // These are the tile sequences from the 14 AI collapses (< 110)
    let loss_sequences: Vec<(&str, Vec<Tile>)> = vec![
        ("AI:65 H:153", vec![
            Tile(9,2,4), Tile(5,2,4), Tile(5,6,4), Tile(9,6,3), Tile(1,6,8), Tile(5,2,8),
            Tile(1,6,4), Tile(1,6,3), Tile(9,7,4), Tile(9,7,8), Tile(1,7,8), Tile(5,2,3),
            Tile(5,7,8), Tile(1,2,8), Tile(1,7,4), Tile(1,7,3), Tile(5,6,8), Tile(1,2,4),
            Tile(5,7,4),
        ]),
        ("AI:78 H:169", vec![
            Tile(1,2,3), Tile(5,7,3), Tile(1,2,4), Tile(5,6,4), Tile(9,6,8), Tile(9,7,3),
            Tile(5,7,4), Tile(1,6,8), Tile(5,6,8), Tile(9,7,8), Tile(1,6,4), Tile(5,2,8),
            Tile(5,6,3), Tile(9,2,3), Tile(5,2,4), Tile(5,7,8), Tile(5,2,3), Tile(1,7,3),
        ]),
        ("AI:85 H:148", vec![
            Tile(9,7,3), Tile(5,7,3), Tile(5,2,3), Tile(1,6,3), Tile(9,2,3), Tile(1,7,4),
            Tile(1,7,8), Tile(1,2,3), Tile(5,6,4), Tile(1,6,8), Tile(1,2,4), Tile(5,6,8),
            Tile(9,6,4), Tile(1,2,8), Tile(9,2,8), Tile(9,6,8), Tile(9,6,3), Tile(1,6,4),
            Tile(5,2,4),
        ]),
        ("AI:87 H:91", vec![
            Tile(5,6,4), Tile(9,6,8), Tile(1,6,8), Tile(5,6,8), Tile(5,6,3), Tile(1,7,4),
            Tile(5,7,3), Tile(1,6,3), Tile(5,2,4), Tile(9,6,3), Tile(9,7,8), Tile(1,6,4),
            Tile(1,7,3), Tile(9,7,3), Tile(1,2,8), Tile(5,7,8), Tile(1,2,4), Tile(5,7,4),
        ]),
        ("AI:95 H:107", vec![
            Tile(5,7,3), Tile(1,2,4), Tile(1,7,8), Tile(5,7,8), Tile(1,6,8), Tile(9,7,4),
            Tile(9,6,8), Tile(9,7,3), Tile(5,2,3), Tile(1,7,3), Tile(1,6,4), Tile(5,6,3),
            Tile(1,2,8), Tile(9,2,3), Tile(5,6,8), Tile(9,2,4), Tile(9,6,3), Tile(5,2,4),
            Tile(1,2,3),
        ]),
        ("AI:95 H:149", vec![
            Tile(5,2,8), Tile(1,2,8), Tile(5,6,3), Tile(1,6,3), Tile(5,2,3), Tile(5,6,8),
            Tile(9,2,3), Tile(9,2,8), Tile(1,6,8), Tile(9,7,3), Tile(9,2,4), Tile(5,2,4),
            Tile(1,7,8), Tile(1,2,4), Tile(1,7,4), Tile(5,7,3), Tile(5,6,4), Tile(9,6,8),
            Tile(9,6,3),
        ]),
        ("AI:96 H:171", vec![
            Tile(5,2,4), Tile(9,2,3), Tile(9,6,3), Tile(9,2,4), Tile(9,7,4), Tile(5,2,3),
            Tile(1,7,8), Tile(1,7,3), Tile(1,7,4), Tile(9,6,8), Tile(9,6,4), Tile(5,7,4),
            Tile(9,7,3), Tile(5,7,8), Tile(1,2,8), Tile(5,6,8), Tile(9,7,8), Tile(1,6,3),
        ]),
        ("AI:98 H:129", vec![
            Tile(1,7,4), Tile(9,6,3), Tile(5,7,3), Tile(1,6,3), Tile(1,2,4), Tile(1,6,8),
            Tile(9,7,4), Tile(1,2,3), Tile(5,7,8), Tile(5,6,4), Tile(1,7,3), Tile(9,2,3),
            Tile(9,7,8), Tile(9,2,4), Tile(5,2,4), Tile(1,7,8), Tile(1,2,8), Tile(9,2,8),
        ]),
        ("AI:100 H:133", vec![
            Tile(9,6,3), Tile(1,2,8), Tile(5,6,4), Tile(9,7,3), Tile(1,7,3), Tile(5,2,3),
            Tile(5,2,8), Tile(9,2,3), Tile(9,7,4), Tile(9,7,8), Tile(5,2,4), Tile(1,6,4),
            Tile(1,6,8), Tile(9,6,8), Tile(1,2,4), Tile(1,7,8), Tile(1,6,3), Tile(1,7,4),
        ]),
        ("AI:102 H:205", vec![
            Tile(5,6,4), Tile(1,7,3), Tile(9,6,8), Tile(5,6,3), Tile(5,2,8), Tile(9,6,4),
            Tile(5,7,3), Tile(5,2,4), Tile(9,7,8), Tile(5,7,8), Tile(9,2,4), Tile(1,2,3),
            Tile(1,2,8), Tile(9,6,3), Tile(1,6,3), Tile(9,7,4), Tile(9,2,3), Tile(5,2,3),
            Tile(1,6,8),
        ]),
        ("AI:103 H:104", vec![
            Tile(9,6,8), Tile(9,7,8), Tile(5,2,4), Tile(5,6,4), Tile(5,2,8), Tile(9,7,4),
            Tile(9,6,3), Tile(5,2,3), Tile(9,2,8), Tile(5,6,3), Tile(1,7,8), Tile(9,6,4),
            Tile(9,2,4), Tile(9,2,3), Tile(1,7,3), Tile(5,6,8), Tile(1,6,8), Tile(1,2,8),
        ]),
        ("AI:104 H:124", vec![
            Tile(1,6,4), Tile(5,7,4), Tile(9,7,4), Tile(9,7,3), Tile(1,2,3), Tile(5,6,4),
            Tile(1,6,8), Tile(1,7,4), Tile(5,2,4), Tile(9,2,4), Tile(9,6,8), Tile(5,2,8),
            Tile(1,6,3), Tile(5,2,3), Tile(5,6,8), Tile(5,7,8), Tile(5,7,3), Tile(9,7,8),
            Tile(5,6,3),
        ]),
        ("AI:107 H:166", vec![
            Tile(5,7,3), Tile(9,7,4), Tile(5,6,4), Tile(1,2,8), Tile(9,6,3), Tile(5,6,8),
            Tile(1,6,3), Tile(9,2,4), Tile(9,6,8), Tile(1,6,4), Tile(1,2,4), Tile(9,7,8),
            Tile(5,2,3), Tile(9,7,3), Tile(5,2,8), Tile(5,6,3), Tile(9,2,8), Tile(5,7,4),
        ]),
        ("AI:108 H:169", vec![
            Tile(9,6,4), Tile(1,2,4), Tile(1,2,8), Tile(9,2,3), Tile(5,2,4), Tile(5,7,3),
            Tile(9,2,8), Tile(5,7,4), Tile(1,2,3), Tile(1,6,4), Tile(5,2,8), Tile(1,6,8),
            Tile(5,6,4), Tile(9,7,8), Tile(9,6,3), Tile(1,7,3), Tile(9,6,8), Tile(9,2,4),
        ]),
    ];

    println!(
        "{:<20} {:>8}  {}",
        "Strategy", "Avg", "Individual scores"
    );
    println!("{}", "-".repeat(76));

    for (label, mode) in &strategies {
        let scores: Vec<i32> = loss_sequences
            .iter()
            .map(|(_, seq)| play_with_mode(&policy_net, device, seq, *mode))
            .collect();

        let a = avg(&scores);
        let details: Vec<String> = loss_sequences.iter().zip(&scores)
            .map(|((name, _), &s)| {
                let orig: i32 = name.split(':').nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
                let delta = s - orig;
                if delta > 0 {
                    format!("{:+}", delta)
                } else if delta < 0 {
                    format!("{:+}", delta)
                } else {
                    "=".to_string()
                }
            })
            .collect();

        println!(
            "{:<20} {:>8.1}  [{}]",
            label, a, details.join(", ")
        );
    }

    println!("\nOriginal AI scores:  [{}]",
        loss_sequences.iter().map(|(n, _)| {
            n.split(':').nth(1).unwrap().split(' ').next().unwrap().to_string()
        }).collect::<Vec<_>>().join(", ")
    );
    println!("Human scores:        [{}]",
        loss_sequences.iter().map(|(n, _)| {
            n.split("H:").nth(1).unwrap().to_string()
        }).collect::<Vec<_>>().join(", ")
    );

    // ── Ensemble Benchmark ──
    if !has_v3 {
        println!("\nSkipping ensemble benchmark (no V3 model)");
        return;
    }

    println!("\n{}", "=".repeat(76));
    println!("=== ENSEMBLE: V2 default + V3 when in danger ===\n");

    // Test different margins and min_switch_turn
    let ensemble_configs: Vec<(&str, f64, usize)> = vec![
        // (label, margin, min_switch_turn)
        ("V2 pure", f64::INFINITY, 0),  // never switch = pure V2
        ("V3 pure", f64::NEG_INFINITY, 0),  // always switch = pure V3
        ("Ens m=0.0 t>=4", 0.0, 4),
        ("Ens m=0.5 t>=4", 0.5, 4),
        ("Ens m=1.0 t>=4", 1.0, 4),
        ("Ens m=0.0 t>=6", 0.0, 6),
        ("Ens m=0.5 t>=6", 0.5, 6),
        ("Ens m=1.0 t>=6", 1.0, 6),
        ("Ens m=1.5 t>=6", 1.5, 6),
        ("Ens m=0.0 t>=8", 0.0, 8),
        ("Ens m=0.5 t>=8", 0.5, 8),
        ("Ens m=1.0 t>=8", 1.0, 8),
    ];

    println!(
        "{:<20} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Strategy", "Avg", "Std", "Min", "Max", "<110", "Delta", "DefTrn"
    );
    println!("{}", "-".repeat(84));

    let mut ens_baseline = 0.0;

    for (label, margin, min_turn) in &ensemble_configs {
        print!("  {}...", label);
        std::io::stdout().flush().ok();

        let results: Vec<(i32, usize)> = sequences
            .iter()
            .map(|seq| play_ensemble(&policy_net, &v3_net, device, seq, *margin, *min_turn))
            .collect();

        let scores: Vec<i32> = results.iter().map(|(s, _)| *s).collect();
        let total_def_turns: usize = results.iter().map(|(_, d)| *d).sum();
        let avg_def = total_def_turns as f64 / results.len() as f64;

        let a = avg(&scores);
        let s = std_dev(&scores);
        let mn = *scores.iter().min().unwrap();
        let mx = *scores.iter().max().unwrap();
        let below_110 = scores.iter().filter(|&&s| s < 110).count();

        if *margin == f64::INFINITY {
            ens_baseline = a;
            println!(
                "\r{:<20} {:>8.1} {:>8.1} {:>8} {:>8} {:>8} {:>8} {:>8.1}",
                label, a, s, mn, mx, below_110, "", avg_def
            );
        } else {
            println!(
                "\r{:<20} {:>8.1} {:>8.1} {:>8} {:>8} {:>8} {:>+8.1} {:>8.1}",
                label, a, s, mn, mx, below_110, a - ens_baseline, avg_def
            );
        }
    }
    println!("{}", "=".repeat(84));

    // Replay losses with best ensemble configs
    println!("\n=== ENSEMBLE REPLAY AI LOSSES ===\n");
    println!("{:<20} {:>8}  {}", "Strategy", "Avg", "Scores");
    println!("{}", "-".repeat(76));

    for (label, margin, min_turn) in &ensemble_configs {
        let results: Vec<(i32, usize)> = loss_sequences
            .iter()
            .map(|(_, seq)| play_ensemble(&policy_net, &v3_net, device, seq, *margin, *min_turn))
            .collect();

        let scores: Vec<i32> = results.iter().map(|(s, _)| *s).collect();
        let a = avg(&scores);

        let details: Vec<String> = loss_sequences.iter().zip(&scores)
            .map(|((name, _), &s)| {
                let orig: i32 = name.split(':').nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
                let delta = s - orig;
                if delta != 0 { format!("{:+}", delta) } else { "=".to_string() }
            })
            .collect();

        println!("{:<20} {:>8.1}  [{}]", label, a, details.join(", "));
    }
}
