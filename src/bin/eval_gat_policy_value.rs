//! Evaluate GAT with both Policy and Value networks
//!
//! Combines policy (action preferences) with value (expected final score)
//! to make better decisions.
//!
//! Strategies:
//! - policy_only: argmax of policy logits
//! - value_only: evaluate all positions with value net, pick best
//! - policy_value: policy softmax * value, pick best
//! - top_k_value: top-K from policy, then pick best by value
//!
//! Usage: cargo run --release --bin eval_gat_policy_value -- --games 200

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use tch::{nn, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::gat::{GATPolicyNet, GATValueNet};
use take_it_easy::neural::model_io::load_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "eval_gat_policy_value")]
#[command(about = "Evaluate GAT with Policy and Value networks")]
struct Args {
    /// Policy model path (.safetensors)
    #[arg(long, default_value = "model_weights/gat_seed789_policy.safetensors")]
    policy_model: String,

    /// Value model path (.safetensors)
    #[arg(long, default_value = "model_weights/gat_value_value.safetensors")]
    value_model: String,

    /// Hidden layer sizes
    #[arg(long, default_value = "128,128")]
    hidden: String,

    /// Number of attention heads
    #[arg(long, default_value_t = 4)]
    heads: usize,

    /// Number of games to play
    #[arg(long, default_value_t = 200)]
    games: usize,

    /// Top-K positions for value evaluation
    #[arg(long, default_value_t = 5)]
    top_k: usize,

    /// Value weight (0 = policy only, 1 = equal, 2 = value dominant)
    #[arg(long, default_value_t = 1.0)]
    value_weight: f64,

    /// Score normalization mean
    #[arg(long, default_value_t = 140.0)]
    score_mean: f64,

    /// Score normalization std
    #[arg(long, default_value_t = 40.0)]
    score_std: f64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

fn main() {
    let args = Args::parse();
    let device = Device::Cpu;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           GAT Policy + Value Evaluation                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let hidden: Vec<i64> = args.hidden.split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    // Load policy network
    println!("Loading policy network: {}", args.policy_model);
    let mut vs_policy = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs_policy, 47, &hidden, args.heads, 0.0);
    if let Err(e) = load_varstore(&mut vs_policy, &args.policy_model) {
        println!("Failed to load policy model: {}", e);
        return;
    }
    println!("  Policy network loaded ✓");

    // Load value network
    println!("Loading value network: {}", args.value_model);
    let mut vs_value = nn::VarStore::new(device);
    let value_net = GATValueNet::new(&vs_value, 47, &hidden, args.heads, 0.0);
    if let Err(e) = load_varstore(&mut vs_value, &args.value_model) {
        println!("Failed to load value model: {}", e);
        return;
    }
    println!("  Value network loaded ✓");

    println!("\nConfig:");
    println!("  Games:        {}", args.games);
    println!("  Top-K:        {}", args.top_k);
    println!("  Value weight: {}", args.value_weight);

    let mut rng = StdRng::seed_from_u64(args.seed);

    // Evaluate different strategies
    println!("\n📊 Evaluating strategies on {} games...\n", args.games);

    // 1. Policy only (baseline)
    let mut rng1 = StdRng::seed_from_u64(args.seed);
    let (policy_avg, policy_scores) = eval_policy_only(&policy_net, args.games, &mut rng1);

    // 2. Top-K value
    let mut rng2 = StdRng::seed_from_u64(args.seed);
    let (topk_avg, topk_scores) = eval_top_k_value(
        &policy_net, &value_net, args.games, args.top_k,
        args.score_mean, args.score_std, &mut rng2
    );

    // 3. Value only (all positions)
    let mut rng3 = StdRng::seed_from_u64(args.seed);
    let (value_avg, value_scores) = eval_value_only(
        &value_net, args.games, args.score_mean, args.score_std, &mut rng3
    );

    // 4. Policy * Value weighted
    let mut rng4 = StdRng::seed_from_u64(args.seed);
    let (combined_avg, combined_scores) = eval_policy_value_combined(
        &policy_net, &value_net, args.games, args.value_weight,
        args.score_mean, args.score_std, &mut rng4
    );

    // 5. Greedy baseline
    let mut rng5 = StdRng::seed_from_u64(args.seed);
    let (greedy_avg, _) = eval_greedy(args.games, &mut rng5);

    // Print results
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                     RESULTS                                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("  Strategy               | Avg Score | ≥140 pts | ≥150 pts");
    println!("  -----------------------|-----------|----------|----------");
    print_result("Policy only", policy_avg, &policy_scores, args.games);
    print_result(&format!("Top-{} + Value", args.top_k), topk_avg, &topk_scores, args.games);
    print_result("Value only", value_avg, &value_scores, args.games);
    print_result("Policy×Value", combined_avg, &combined_scores, args.games);
    print_result("Greedy baseline", greedy_avg, &[], args.games);

    println!("\n  Best strategy vs Policy: {:+.2} pts",
        [topk_avg, value_avg, combined_avg].iter().cloned().fold(f64::NEG_INFINITY, f64::max) - policy_avg);
}

fn print_result(name: &str, avg: f64, scores: &[i32], n_games: usize) {
    let ge140 = scores.iter().filter(|&&s| s >= 140).count();
    let ge150 = scores.iter().filter(|&&s| s >= 150).count();
    if scores.is_empty() {
        println!("  {:22} | {:9.2} |    -     |    -", name, avg);
    } else {
        println!("  {:22} | {:9.2} | {:6.1}%  | {:6.1}%",
            name, avg,
            100.0 * ge140 as f64 / n_games as f64,
            100.0 * ge150 as f64 / n_games as f64);
    }
}

fn eval_policy_only(net: &GATPolicyNet, n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
    let mut scores = Vec::new();

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let logits = tch::no_grad(|| net.forward(&features.unsqueeze(0), false).squeeze_dim(0));

            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
            let best_pos = (logits + mask).argmax(0, false).int64_value(&[]) as usize;

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

fn eval_value_only(
    net: &GATValueNet, n_games: usize,
    score_mean: f64, score_std: f64, rng: &mut StdRng
) -> (f64, Vec<i32>) {
    let mut scores = Vec::new();

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            // Evaluate each position
            let mut best_pos = avail[0];
            let mut best_value = f64::NEG_INFINITY;

            for &pos in &avail {
                let mut test_plateau = plateau.clone();
                test_plateau.tiles[pos] = tile;
                let test_deck = replace_tile_in_deck(&deck, &tile);

                // Get next expected tile (average over remaining)
                let remaining = get_available_tiles(&test_deck);
                if remaining.is_empty() {
                    // Last tile - just use current score
                    let v = result(&test_plateau) as f64;
                    if v > best_value {
                        best_value = v;
                        best_pos = pos;
                    }
                } else {
                    // Average value over possible next tiles
                    let next_tile = remaining[0]; // Just use first for speed
                    let features = convert_plateau_for_gat_47ch(&test_plateau, &next_tile, &test_deck, turn + 1, 19);
                    let v = tch::no_grad(|| {
                        net.forward(&features.unsqueeze(0), false).double_value(&[0, 0])
                    }) * score_std + score_mean;

                    if v > best_value {
                        best_value = v;
                        best_pos = pos;
                    }
                }
            }

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

fn eval_top_k_value(
    policy_net: &GATPolicyNet, value_net: &GATValueNet,
    n_games: usize, top_k: usize,
    score_mean: f64, score_std: f64, rng: &mut StdRng
) -> (f64, Vec<i32>) {
    let mut scores = Vec::new();

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            // Get policy logits
            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let logits = tch::no_grad(|| policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0));

            // Mask unavailable positions
            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
            let masked_logits = logits + mask;

            // Get top-K positions
            let (_, top_indices) = masked_logits.topk(top_k.min(avail.len()) as i64, 0, true, true);
            let top_positions: Vec<usize> = (0..top_indices.size()[0])
                .map(|i| top_indices.int64_value(&[i]) as usize)
                .collect();

            // Evaluate top-K with value network
            let mut best_pos = top_positions[0];
            let mut best_value = f64::NEG_INFINITY;

            for &pos in &top_positions {
                let mut test_plateau = plateau.clone();
                test_plateau.tiles[pos] = tile;
                let test_deck = replace_tile_in_deck(&deck, &tile);

                let remaining = get_available_tiles(&test_deck);
                if remaining.is_empty() {
                    let v = result(&test_plateau) as f64;
                    if v > best_value {
                        best_value = v;
                        best_pos = pos;
                    }
                } else {
                    let next_tile = remaining[0];
                    let features = convert_plateau_for_gat_47ch(&test_plateau, &next_tile, &test_deck, turn + 1, 19);
                    let v = tch::no_grad(|| {
                        value_net.forward(&features.unsqueeze(0), false).double_value(&[0, 0])
                    }) * score_std + score_mean;

                    if v > best_value {
                        best_value = v;
                        best_pos = pos;
                    }
                }
            }

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

fn eval_policy_value_combined(
    policy_net: &GATPolicyNet, value_net: &GATValueNet,
    n_games: usize, value_weight: f64,
    score_mean: f64, score_std: f64, rng: &mut StdRng
) -> (f64, Vec<i32>) {
    let mut scores = Vec::new();

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            // Get policy probabilities
            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let logits = tch::no_grad(|| policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0));

            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
            let probs = (logits + &mask).softmax(0, Kind::Float);

            // Combine policy probs with value estimates
            let mut best_pos = avail[0];
            let mut best_score = f64::NEG_INFINITY;

            for &pos in &avail {
                let policy_prob: f64 = probs.double_value(&[pos as i64]);

                let mut test_plateau = plateau.clone();
                test_plateau.tiles[pos] = tile;
                let test_deck = replace_tile_in_deck(&deck, &tile);

                let remaining = get_available_tiles(&test_deck);
                let value_est = if remaining.is_empty() {
                    result(&test_plateau) as f64
                } else {
                    let next_tile = remaining[0];
                    let feat = convert_plateau_for_gat_47ch(&test_plateau, &next_tile, &test_deck, turn + 1, 19);
                    tch::no_grad(|| {
                        value_net.forward(&feat.unsqueeze(0), false).double_value(&[0, 0])
                    }) * score_std + score_mean
                };

                // Combined score: policy_prob + value_weight * normalized_value
                let combined = policy_prob + value_weight * (value_est / 200.0);

                if combined > best_score {
                    best_score = combined;
                    best_pos = pos;
                }
            }

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

fn eval_greedy(n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
    let mut scores = Vec::new();

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for _ in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            let mut best_pos = avail[0];
            let mut best_score = i32::MIN;
            for &pos in &avail {
                let mut test = plateau.clone();
                test.tiles[pos] = tile;
                let s = result(&test);
                if s > best_score {
                    best_score = s;
                    best_pos = pos;
                }
            }

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}
