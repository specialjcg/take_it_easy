//! Benchmark MCTS at various simulation budgets.
//!
//! Compares batched MCTS (CPU or GPU) against GT Direct baseline.
//!
//! Usage:
//!   cargo run --release --bin benchmark_mcts_gpu -- --device cpu --num-games 20 --sim-counts "50,100"
//!   cargo run --release --bin benchmark_mcts_gpu -- --device cuda --num-games 50 --sim-counts "100,500,1000"

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::path::Path;
use std::time::Instant;
use tch::{nn, Device};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::device_util::{check_cuda, parse_device};
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::load_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::neural::graph_transformer::GraphTransformerValueNet;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::batched_mcts::{batched_gt_mcts_select, BatchedMctsConfig};
use take_it_easy::strategy::expectimax::{expectimax_select, ExpectimaxConfig};

#[derive(Parser)]
#[command(name = "benchmark_mcts_gpu", about = "Benchmark batched MCTS at various sim budgets")]
struct Args {
    /// Device: "cpu", "cuda", "cuda:0"
    #[arg(long, default_value = "cpu")]
    device: String,

    /// Number of games per sim-count
    #[arg(long, default_value_t = 50)]
    num_games: usize,

    /// Comma-separated simulation counts to benchmark
    #[arg(long, default_value = "50,100,500")]
    sim_counts: String,

    /// Rollout batch size (number of rollouts per forward pass)
    #[arg(long, default_value_t = 32)]
    batch_size: usize,

    /// Line-boost strength
    #[arg(long, default_value_t = 3.0)]
    boost: f64,

    /// PUCT exploration constant
    #[arg(long, default_value_t = 2.5)]
    c_puct: f64,

    /// Path to GT policy model weights
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    model_path: String,

    /// Embedding dimension
    #[arg(long, default_value_t = 128)]
    embed_dim: i64,

    /// Number of transformer layers
    #[arg(long, default_value_t = 2)]
    num_layers: usize,

    /// Number of attention heads
    #[arg(long, default_value_t = 4)]
    num_heads: i64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Path to value network weights (enables Expectimax benchmark)
    #[arg(long)]
    value_model_path: Option<String>,
}

/// Play one game using GT Direct (argmax, no heuristics).
fn play_gt_direct(
    policy_net: &GraphTransformerPolicyNet,
    device: Device,
    tile_sequence: &[Tile],
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

        let mut best_pos = legal[0];
        let mut best_val = f64::NEG_INFINITY;
        for &pos in &legal {
            if logit_values[pos] > best_val {
                best_val = logit_values[pos];
                best_pos = pos;
            }
        }
        plateau.tiles[best_pos] = tile;
    }

    result(&plateau)
}

/// Play one game using batched MCTS.
fn play_mcts(
    policy_net: &GraphTransformerPolicyNet,
    config: &BatchedMctsConfig,
    tile_sequence: &[Tile],
    rng: &mut StdRng,
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, &tile) in tile_sequence.iter().enumerate() {
        deck = replace_tile_in_deck(&deck, &tile);
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let pos = batched_gt_mcts_select(&plateau, &tile, &deck, turn, policy_net, config, rng);
        plateau.tiles[pos] = tile;
    }

    result(&plateau)
}

/// Play one game using expectimax (value network lookahead).
fn play_expectimax(
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    config: &ExpectimaxConfig,
    tile_sequence: &[Tile],
) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, &tile) in tile_sequence.iter().enumerate() {
        deck = replace_tile_in_deck(&deck, &tile);
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }
        let pos = expectimax_select(&plateau, &tile, &deck, turn, policy_net, value_net, config);
        plateau.tiles[pos] = tile;
    }

    result(&plateau)
}

/// Generate a random tile sequence (19 tiles drawn from a full deck).
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
        let idx = rng.random_range(0..available.len());
        seq.push(available.remove(idx));
    }
    seq
}

struct BenchResult {
    label: String,
    scores: Vec<i32>,
    elapsed_ms: f64,
}

impl BenchResult {
    fn avg(&self) -> f64 {
        self.scores.iter().sum::<i32>() as f64 / self.scores.len() as f64
    }
    fn std(&self) -> f64 {
        let mean = self.avg();
        let var = self.scores.iter().map(|&s| (s as f64 - mean).powi(2)).sum::<f64>()
            / self.scores.len() as f64;
        var.sqrt()
    }
    fn min(&self) -> i32 {
        *self.scores.iter().min().unwrap()
    }
    fn max(&self) -> i32 {
        *self.scores.iter().max().unwrap()
    }
    fn ms_per_game(&self) -> f64 {
        self.elapsed_ms / self.scores.len() as f64
    }
}

fn main() {
    let args = Args::parse();

    println!("================================================");
    println!("  Batched MCTS Benchmark");
    println!("================================================\n");

    // Parse device
    let device = match parse_device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    check_cuda();
    println!("Using device: {:?}\n", device);

    // Parse sim counts
    let sim_counts: Vec<usize> = args
        .sim_counts
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if sim_counts.is_empty() {
        eprintln!("Error: no valid sim counts in '{}'", args.sim_counts);
        return;
    }

    // Load model
    let mut vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(
        &vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.1,
    );

    if !Path::new(&args.model_path).exists() {
        eprintln!("Error: model weights not found: {}", args.model_path);
        return;
    }
    match load_varstore(&mut vs, &args.model_path) {
        Ok(()) => println!("Loaded model from {}", args.model_path),
        Err(e) => {
            eprintln!("Error loading model: {}", e);
            return;
        }
    }

    // Generate tile sequences (shared across all strategies)
    let mut rng = StdRng::seed_from_u64(args.seed);
    let sequences: Vec<Vec<Tile>> = (0..args.num_games)
        .map(|_| random_tile_sequence(&mut rng))
        .collect();

    println!(
        "Benchmarking {} games | sim_counts={:?} | batch_size={}\n",
        args.num_games, sim_counts, args.batch_size
    );

    // 1. GT Direct baseline
    print!("Running GT Direct baseline...");
    let start = Instant::now();
    let direct_scores: Vec<i32> = sequences
        .iter()
        .map(|seq| play_gt_direct(&policy_net, device, seq))
        .collect();
    let direct_result = BenchResult {
        label: "GT Direct".into(),
        scores: direct_scores,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
    };
    println!(" done ({:.0}ms)", direct_result.elapsed_ms);

    // 2. MCTS at each sim count
    let mut results = vec![direct_result];

    for &sims in &sim_counts {
        let label = format!("MCTS-{}", sims);
        print!("Running {} ({} games)...", label, args.num_games);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let config = BatchedMctsConfig {
            num_sims: sims,
            boost: args.boost,
            c_puct: args.c_puct,
            device,
            rollout_batch_size: args.batch_size,
        };

        let start = Instant::now();
        let scores: Vec<i32> = sequences
            .iter()
            .enumerate()
            .map(|(i, seq)| {
                let score = play_mcts(&policy_net, &config, seq, &mut rng);
                if (i + 1) % 10 == 0 {
                    print!(" {}g", i + 1);
                    std::io::Write::flush(&mut std::io::stdout()).ok();
                }
                score
            })
            .collect();
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        println!(" done ({:.0}ms)", elapsed_ms);

        results.push(BenchResult {
            label,
            scores,
            elapsed_ms,
        });
    }

    // 3. Expectimax (if value model provided)
    if let Some(ref value_path) = args.value_model_path {
        if Path::new(value_path).exists() {
            print!("Running Expectimax ({} games)...", args.num_games);
            std::io::Write::flush(&mut std::io::stdout()).ok();

            let mut value_vs = nn::VarStore::new(device);
            let value_net = GraphTransformerValueNet::new(
                &value_vs, 47, args.embed_dim, args.num_layers, args.num_heads, 0.0,
            );
            match load_varstore(&mut value_vs, value_path) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("\nError loading value model: {}", e);
                    std::process::exit(1);
                }
            }

            let ex_config = ExpectimaxConfig {
                device,
                boost: args.boost,
                score_mean: 140.0,
                score_std: 40.0,
                min_turn: 0,
                top_k_ply1: 3,
                top_k_ply2: 2,
            };

            let start = Instant::now();
            let scores: Vec<i32> = sequences
                .iter()
                .enumerate()
                .map(|(i, seq)| {
                    let score = play_expectimax(&policy_net, &value_net, &ex_config, seq);
                    if (i + 1) % 10 == 0 {
                        print!(" {}g", i + 1);
                        std::io::Write::flush(&mut std::io::stdout()).ok();
                    }
                    score
                })
                .collect();
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            println!(" done ({:.0}ms)", elapsed_ms);

            results.push(BenchResult {
                label: "Expectimax".into(),
                scores,
                elapsed_ms,
            });
        } else {
            eprintln!("Warning: value model not found: {}", value_path);
        }
    }

    // Print results table
    println!("\n{}", "=".repeat(76));
    println!(
        "{:<16} {:>8} {:>8} {:>8} {:>8} {:>10}",
        "Strategy", "Avg", "Std", "Min", "Max", "ms/game"
    );
    println!("{}", "-".repeat(76));

    let baseline_avg = results[0].avg();
    for r in &results {
        let delta = r.avg() - baseline_avg;
        let delta_str = if delta.abs() < 0.05 {
            String::new()
        } else {
            format!(" ({:+.1})", delta)
        };
        println!(
            "{:<16} {:>7.1}{} {:>7.1} {:>8} {:>8} {:>9.0}",
            r.label,
            r.avg(),
            delta_str,
            r.std(),
            r.min(),
            r.max(),
            r.ms_per_game()
        );
    }
    println!("{}", "=".repeat(76));
}
