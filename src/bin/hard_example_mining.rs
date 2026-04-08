//! Hard Example Mining for Risk-Aware Policy
//!
//! Step 1: Play N games with V2 policy, identify catastrophic sequences (score < threshold)
//! Step 2: Replay those sequences with 3-ply expectimax (better expert)
//! Step 3: Save corrected samples to TSV for fine-tuning
//!
//! Usage:
//!   # Step 1+2+3 in one shot:
//!   cargo run --release --bin hard_example_mining -- \
//!     --policy-path model_weights/distilled_v2_policy.safetensors \
//!     --value-path model_weights/value_net_v2_notanh.safetensors \
//!     --num-games 5000 --threshold 130 --depth 3 --min-turn 6 \
//!     --output-tsv data/hard_examples_3ply.tsv

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::io::{BufWriter, Write};
use std::time::Instant;
use tch::{nn, Device};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::device_util::{check_cuda, parse_device};
use take_it_easy::neural::graph_transformer::{
    GraphTransformerPolicyNet, GraphTransformerValueNet,
};
use take_it_easy::neural::model_io::load_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::expectimax::{
    expectimax_select, expectimax_2ply_select, expectimax_3ply_select, ExpectimaxConfig,
};

#[derive(Parser, Debug)]
#[command(name = "hard_example_mining")]
#[command(about = "Mine hard examples and replay with stronger expert")]
struct Args {
    #[arg(long, default_value = "cuda")]
    device: String,

    /// V2 policy model path
    #[arg(long, default_value = "model_weights/distilled_v2_policy.safetensors")]
    policy_path: String,

    /// Value net path (for expectimax replay)
    #[arg(long, default_value = "model_weights/value_net_v2_notanh.safetensors")]
    value_path: String,

    /// Number of games to play with V2 to find catastrophic sequences
    #[arg(long, default_value_t = 5000)]
    num_games: usize,

    /// Score threshold: sequences below this are considered catastrophic
    #[arg(long, default_value_t = 130)]
    threshold: i32,

    /// Expectimax depth for replay (2 or 3)
    #[arg(long, default_value_t = 3)]
    depth: usize,

    /// Min turn for expectimax activation during replay
    #[arg(long, default_value_t = 6)]
    min_turn: usize,

    /// Output TSV path for hard example samples
    #[arg(long, default_value = "data/hard_examples_3ply.tsv")]
    output_tsv: String,

    /// Also output the original V2 data for these sequences (for comparison)
    #[arg(long, default_value = "data/hard_examples_v2_baseline.tsv")]
    baseline_tsv: String,

    #[arg(long, default_value_t = 128)]
    embed_dim: i64,

    #[arg(long, default_value_t = 2)]
    num_layers: usize,

    #[arg(long, default_value_t = 4)]
    num_heads: i64,

    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Score normalization mean
    #[arg(long, default_value_t = 140.0)]
    score_mean: f64,

    /// Score normalization std
    #[arg(long, default_value_t = 40.0)]
    score_std: f64,

    #[arg(long, default_value_t = 3.0)]
    boost: f64,
}

struct GameRecord {
    tile_sequence: Vec<Tile>,
    v2_score: i32,
    v2_positions: Vec<usize>,
}

struct SampleRecord {
    plateau_raw: [i32; 19],
    tile_raw: (i32, i32, i32),
    position: usize,
    turn: usize,
    final_score: i32,
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

/// Play one game with V2 GT Direct, recording positions chosen.
fn play_v2_record(
    policy_net: &GraphTransformerPolicyNet,
    device: Device,
    tile_sequence: &[Tile],
) -> GameRecord {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut positions = Vec::with_capacity(19);

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
            .max_by(|&&a, &&b| logit_values[a].partial_cmp(&logit_values[b]).unwrap())
            .unwrap();

        positions.push(pos);
        plateau.tiles[pos] = tile;
    }

    let score = result(&plateau);
    GameRecord {
        tile_sequence: tile_sequence.to_vec(),
        v2_score: score,
        v2_positions: positions,
    }
}

/// Replay a tile sequence with expectimax, recording (plateau, tile, position, score) per turn.
fn replay_with_expectimax(
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    config: &ExpectimaxConfig,
    tile_sequence: &[Tile],
    depth: usize,
) -> Vec<SampleRecord> {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut samples = Vec::with_capacity(19);

    for (turn, &tile) in tile_sequence.iter().enumerate() {
        deck = replace_tile_in_deck(&deck, &tile);
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        // Record plateau state BEFORE placement
        let mut plateau_raw = [0i32; 19];
        for i in 0..19 {
            let t = &plateau.tiles[i];
            if t.0 != 0 || t.1 != 0 || t.2 != 0 {
                plateau_raw[i] = t.0 * 100 + t.1 * 10 + t.2;
            }
        }

        let pos = match depth {
            3 => expectimax_3ply_select(
                &plateau, &tile, &deck, turn, policy_net, value_net, config,
            ),
            2 => expectimax_2ply_select(
                &plateau, &tile, &deck, turn, policy_net, value_net, config,
            ),
            _ => expectimax_select(
                &plateau, &tile, &deck, turn, policy_net, value_net, config,
            ),
        };

        samples.push(SampleRecord {
            plateau_raw,
            tile_raw: (tile.0, tile.1, tile.2),
            position: pos,
            turn,
            final_score: 0, // filled after game ends
        });

        plateau.tiles[pos] = tile;
    }

    let final_score = result(&plateau);
    for s in &mut samples {
        s.final_score = final_score;
    }

    samples
}

/// Replay with V2 (for baseline comparison TSV).
fn replay_with_v2(
    policy_net: &GraphTransformerPolicyNet,
    device: Device,
    tile_sequence: &[Tile],
) -> Vec<SampleRecord> {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut samples = Vec::with_capacity(19);

    for (turn, &tile) in tile_sequence.iter().enumerate() {
        deck = replace_tile_in_deck(&deck, &tile);
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let mut plateau_raw = [0i32; 19];
        for i in 0..19 {
            let t = &plateau.tiles[i];
            if t.0 != 0 || t.1 != 0 || t.2 != 0 {
                plateau_raw[i] = t.0 * 100 + t.1 * 10 + t.2;
            }
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
            .max_by(|&&a, &&b| logit_values[a].partial_cmp(&logit_values[b]).unwrap())
            .unwrap();

        samples.push(SampleRecord {
            plateau_raw,
            tile_raw: (tile.0, tile.1, tile.2),
            position: pos,
            turn,
            final_score: 0,
        });

        plateau.tiles[pos] = tile;
    }

    let final_score = result(&plateau);
    for s in &mut samples {
        s.final_score = final_score;
    }

    samples
}

fn save_samples(samples: &[SampleRecord], path: &str) -> std::io::Result<usize> {
    let file = std::fs::File::create(path)?;
    let mut w = BufWriter::new(file);
    writeln!(w, "# plateau(19)\ttile(3)\tposition\tturn\tfinal_score\tweight")?;
    let mut written = 0;
    for s in samples {
        let plateau_str: Vec<String> = s.plateau_raw.iter().map(|v| v.to_string()).collect();
        let weight = (s.final_score as f64 / 100.0).powf(3.0);
        writeln!(
            w,
            "{}\t{},{},{}\t{}\t{}\t{}\t{:.6}",
            plateau_str.join(","),
            s.tile_raw.0, s.tile_raw.1, s.tile_raw.2,
            s.position, s.turn, s.final_score, weight
        )?;
        written += 1;
    }
    Ok(written)
}

fn avg(scores: &[i32]) -> f64 {
    if scores.is_empty() { return 0.0; }
    scores.iter().sum::<i32>() as f64 / scores.len() as f64
}

fn main() {
    let args = Args::parse();

    let device = match parse_device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };
    check_cuda();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  Hard Example Mining — Risk-Aware Policy Training           ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");
    println!("Device: {:?}", device);
    println!("Threshold: {} pts", args.threshold);
    println!("Expert depth: {}-ply (min_turn={})", args.depth, args.min_turn);

    // Load models
    let mut policy_vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(
        &policy_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
    );
    if let Err(e) = load_varstore(&mut policy_vs, &args.policy_path) {
        eprintln!("Error loading policy: {}", e);
        return;
    }
    println!("Loaded V2 policy from {}", args.policy_path);

    let mut value_vs = nn::VarStore::new(device);
    let value_net = GraphTransformerValueNet::new(
        &value_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
    );
    if let Err(e) = load_varstore(&mut value_vs, &args.value_path) {
        eprintln!("Error loading value net: {}", e);
        return;
    }
    println!("Loaded value net from {}", args.value_path);

    let ex_config = ExpectimaxConfig {
        device,
        boost: args.boost,
        score_mean: args.score_mean,
        score_std: args.score_std,
        min_turn: args.min_turn,
        top_k_ply1: 3,
        top_k_ply2: 2,
    };

    // ── Step 1: Play games with V2, find catastrophic sequences ──
    println!("\n--- Step 1: Playing {} games with V2 ---\n", args.num_games);
    let step1_start = Instant::now();
    let mut rng = StdRng::seed_from_u64(args.seed);

    let mut all_scores = Vec::with_capacity(args.num_games);
    let mut catastrophic: Vec<GameRecord> = Vec::new();

    for i in 0..args.num_games {
        let seq = random_tile_sequence(&mut rng);
        let record = play_v2_record(&policy_net, device, &seq);
        all_scores.push(record.v2_score);
        if record.v2_score < args.threshold {
            catastrophic.push(record);
        }
        if (i + 1) % 1000 == 0 {
            println!(
                "  {}/{} games | {} catastrophic so far ({:.1}%)",
                i + 1,
                args.num_games,
                catastrophic.len(),
                100.0 * catastrophic.len() as f64 / (i + 1) as f64
            );
        }
    }

    let step1_time = step1_start.elapsed();
    println!(
        "\nStep 1 complete ({:.1}s): {} total games, {} catastrophic ({:.1}%)",
        step1_time.as_secs_f32(),
        args.num_games,
        catastrophic.len(),
        100.0 * catastrophic.len() as f64 / args.num_games as f64
    );
    println!(
        "V2 avg: {:.1} pts | Catastrophic avg: {:.1} pts",
        avg(&all_scores),
        avg(&catastrophic.iter().map(|g| g.v2_score).collect::<Vec<_>>())
    );

    if catastrophic.is_empty() {
        println!("No catastrophic games found. Try lowering --threshold.");
        return;
    }

    // ── Step 2: Replay catastrophic sequences with expert ──
    println!(
        "\n--- Step 2: Replaying {} catastrophic sequences with {}-ply expert ---\n",
        catastrophic.len(),
        args.depth
    );
    let step2_start = Instant::now();

    let mut expert_samples: Vec<SampleRecord> = Vec::new();
    let mut v2_baseline_samples: Vec<SampleRecord> = Vec::new();
    let mut expert_scores: Vec<i32> = Vec::new();

    for (i, game) in catastrophic.iter().enumerate() {
        // Replay with expert
        let expert_records =
            replay_with_expectimax(&policy_net, &value_net, &ex_config, &game.tile_sequence, args.depth);
        let expert_score = expert_records.last().map(|s| s.final_score).unwrap_or(0);
        expert_scores.push(expert_score);

        // Also replay with V2 for baseline
        let v2_records = replay_with_v2(&policy_net, device, &game.tile_sequence);

        // Count divergent moves
        let divergent = expert_records.iter().zip(v2_records.iter())
            .filter(|(e, v)| e.position != v.position)
            .count();

        if (i + 1) % 50 == 0 || i == 0 {
            println!(
                "  [{}/{}] V2: {} → Expert: {} ({:+}) | {} divergent moves",
                i + 1,
                catastrophic.len(),
                game.v2_score,
                expert_score,
                expert_score - game.v2_score,
                divergent
            );
        }

        expert_samples.extend(expert_records);
        v2_baseline_samples.extend(v2_records);
    }

    let step2_time = step2_start.elapsed();
    let v2_cat_avg = avg(&catastrophic.iter().map(|g| g.v2_score).collect::<Vec<_>>());
    let expert_avg = avg(&expert_scores);

    println!(
        "\nStep 2 complete ({:.1}s): {} samples generated",
        step2_time.as_secs_f32(),
        expert_samples.len()
    );
    println!(
        "Catastrophic V2 avg: {:.1} → Expert avg: {:.1} ({:+.1})",
        v2_cat_avg, expert_avg, expert_avg - v2_cat_avg
    );

    // Score distribution comparison
    println!("\nScore improvement distribution:");
    let mut improvements: Vec<i32> = expert_scores
        .iter()
        .zip(catastrophic.iter())
        .map(|(&e, g)| e - g.v2_score)
        .collect();
    improvements.sort();
    let improved = improvements.iter().filter(|&&d| d > 0).count();
    let same = improvements.iter().filter(|&&d| d == 0).count();
    let worse = improvements.iter().filter(|&&d| d < 0).count();
    println!(
        "  Improved: {} ({:.0}%) | Same: {} | Worse: {} ({:.0}%)",
        improved,
        100.0 * improved as f64 / improvements.len() as f64,
        same,
        worse,
        100.0 * worse as f64 / improvements.len() as f64
    );
    if !improvements.is_empty() {
        println!(
            "  Improvement: min={:+}, median={:+}, max={:+}",
            improvements[0],
            improvements[improvements.len() / 2],
            improvements[improvements.len() - 1]
        );
    }

    // ── Step 3: Save to TSV ──
    println!("\n--- Step 3: Saving samples ---\n");

    match save_samples(&expert_samples, &args.output_tsv) {
        Ok(n) => println!("Expert samples: {} written to {}", n, args.output_tsv),
        Err(e) => eprintln!("Error saving expert TSV: {}", e),
    }

    match save_samples(&v2_baseline_samples, &args.baseline_tsv) {
        Ok(n) => println!("V2 baseline:    {} written to {}", n, args.baseline_tsv),
        Err(e) => eprintln!("Error saving baseline TSV: {}", e),
    }

    // ── Summary ──
    println!("\n{}", "=".repeat(68));
    println!("SUMMARY");
    println!("{}", "-".repeat(68));
    println!("Games played:       {}", args.num_games);
    println!("Catastrophic (< {}): {} ({:.1}%)",
        args.threshold, catastrophic.len(),
        100.0 * catastrophic.len() as f64 / args.num_games as f64);
    println!("Expert depth:       {}-ply", args.depth);
    println!("V2 avg on catastrophic:     {:.1} pts", v2_cat_avg);
    println!("Expert avg on catastrophic: {:.1} pts ({:+.1})", expert_avg, expert_avg - v2_cat_avg);
    println!("Expert samples:     {}", expert_samples.len());
    println!("Output:             {}", args.output_tsv);
    println!("{}", "=".repeat(68));
    println!("\nNext step: merge with original data and fine-tune:");
    println!("  cat data/expectimax_2ply_10k.tsv {} > data/mixed_training.tsv", args.output_tsv);
    println!("  cargo run --release --bin distill_expectimax -- \\");
    println!("    --load-tsv data/mixed_training.tsv \\");
    println!("    --policy-path model_weights/distilled_v2_policy.safetensors \\");
    println!("    --value-path model_weights/value_net_v2_notanh.safetensors \\");
    println!("    --save-path model_weights/distilled_v3_risk.safetensors \\");
    println!("    --weight-mode standard --epochs 20 --eval-games 500");
}
