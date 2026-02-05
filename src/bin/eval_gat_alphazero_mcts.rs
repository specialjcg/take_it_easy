//! Evaluate GAT with AlphaZero-style MCTS
//!
//! Uses trained 47ch policy and value networks with PUCT-based MCTS.
//! Policy provides action priors, Value evaluates leaf nodes.
//!
//! Usage: cargo run --release --bin eval_gat_alphazero_mcts -- --games 100 --simulations 50

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::time::Instant;
use tch::{nn, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::deck::Deck;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::gat::{GATPolicyNet, GATValueNet};
use take_it_easy::neural::model_io::load_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "eval_gat_alphazero_mcts")]
#[command(about = "Evaluate GAT with AlphaZero-style MCTS")]
struct Args {
    /// Policy model path (safetensors)
    #[arg(long, default_value = "model_weights/gat_seed789_policy.safetensors")]
    policy_model: String,

    /// Value model path (safetensors)
    #[arg(long, default_value = "model_weights/gat_value_value.safetensors")]
    value_model: String,

    /// Hidden layer sizes
    #[arg(long, default_value = "128,128")]
    hidden: String,

    /// Number of attention heads
    #[arg(long, default_value_t = 4)]
    heads: usize,

    /// Number of games
    #[arg(long, default_value_t = 100)]
    games: usize,

    /// MCTS simulations per move
    #[arg(long, default_value_t = 50)]
    simulations: usize,

    /// PUCT exploration constant
    #[arg(long, default_value_t = 1.5)]
    c_puct: f64,

    /// Value network weight (vs rollout)
    #[arg(long, default_value_t = 0.8)]
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
    println!("║      GAT AlphaZero-style MCTS Evaluation                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let hidden: Vec<i64> = args.hidden.split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    // Load networks
    println!("Loading networks...");
    let mut vs_policy = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs_policy, 47, &hidden, args.heads, 0.0);
    if let Err(e) = load_varstore(&mut vs_policy, &args.policy_model) {
        println!("Failed to load policy: {}", e);
        return;
    }
    println!("  Policy: {} ✓", args.policy_model);

    let mut vs_value = nn::VarStore::new(device);
    let value_net = GATValueNet::new(&vs_value, 47, &hidden, args.heads, 0.0);
    if let Err(e) = load_varstore(&mut vs_value, &args.value_model) {
        println!("Failed to load value: {}", e);
        return;
    }
    println!("  Value: {} ✓", args.value_model);

    println!("\nConfig:");
    println!("  Games:        {}", args.games);
    println!("  Simulations:  {}", args.simulations);
    println!("  C_PUCT:       {}", args.c_puct);
    println!("  Value weight: {}", args.value_weight);

    let mut rng = StdRng::seed_from_u64(args.seed);

    // Evaluate different configurations
    println!("\n📊 Evaluating strategies...\n");

    // 1. Policy only (baseline)
    let mut rng1 = StdRng::seed_from_u64(args.seed);
    let start = Instant::now();
    let (policy_avg, policy_scores) = eval_policy_only(&policy_net, args.games, &mut rng1);
    let policy_time = start.elapsed().as_secs_f32();

    // 2. AlphaZero MCTS
    let mut rng2 = StdRng::seed_from_u64(args.seed);
    let start = Instant::now();
    let (mcts_avg, mcts_scores) = eval_alphazero_mcts(
        &policy_net, &value_net, args.games, args.simulations,
        args.c_puct, args.value_weight, args.score_mean, args.score_std,
        &mut rng2
    );
    let mcts_time = start.elapsed().as_secs_f32();

    // 3. MCTS with pure value (no policy prior)
    let mut rng3 = StdRng::seed_from_u64(args.seed);
    let start = Instant::now();
    let (mcts_value_avg, mcts_value_scores) = eval_mcts_value_only(
        &value_net, args.games, args.simulations,
        args.score_mean, args.score_std, &mut rng3
    );
    let mcts_value_time = start.elapsed().as_secs_f32();

    // 4. Greedy baseline
    let mut rng4 = StdRng::seed_from_u64(args.seed);
    let (greedy_avg, _) = eval_greedy(args.games, &mut rng4);

    // Results
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                     RESULTS                                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("  Strategy                | Avg Score | ≥140 pts | ≥150 pts | Time");
    println!("  ------------------------|-----------|----------|----------|-------");
    print_result("Policy only (baseline)", policy_avg, &policy_scores, args.games, policy_time);
    print_result(&format!("AlphaZero MCTS ({}sim)", args.simulations), mcts_avg, &mcts_scores, args.games, mcts_time);
    print_result(&format!("MCTS Value only ({}sim)", args.simulations), mcts_value_avg, &mcts_value_scores, args.games, mcts_value_time);
    print_result("Greedy", greedy_avg, &[], args.games, 0.0);

    println!("\n  AlphaZero MCTS vs Policy: {:+.2} pts", mcts_avg - policy_avg);

    if mcts_avg > policy_avg {
        println!("\n  🎯 MCTS improves over pure policy!");
    } else {
        println!("\n  ℹ️  Pure policy is stronger (try more simulations or tune c_puct)");
    }
}

fn print_result(name: &str, avg: f64, scores: &[i32], n_games: usize, time: f32) {
    let ge140 = scores.iter().filter(|&&s| s >= 140).count();
    let ge150 = scores.iter().filter(|&&s| s >= 150).count();
    if scores.is_empty() {
        println!("  {:23} | {:9.2} |    -     |    -     |  -", name, avg);
    } else {
        println!("  {:23} | {:9.2} | {:6.1}%  | {:6.1}%  | {:.1}s",
            name, avg,
            100.0 * ge140 as f64 / n_games as f64,
            100.0 * ge150 as f64 / n_games as f64,
            time);
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

fn eval_alphazero_mcts(
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    n_games: usize,
    n_sims: usize,
    c_puct: f64,
    value_weight: f64,
    score_mean: f64,
    score_std: f64,
    rng: &mut StdRng,
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

            let best_pos = alphazero_mcts_search(
                &plateau, &deck, &tile, &avail,
                policy_net, value_net,
                n_sims, c_puct, value_weight,
                score_mean, score_std, turn, rng,
            );

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

fn alphazero_mcts_search(
    plateau: &Plateau,
    deck: &Deck,
    tile: &Tile,
    avail: &[usize],
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    n_sims: usize,
    c_puct: f64,
    value_weight: f64,
    score_mean: f64,
    score_std: f64,
    turn: usize,
    rng: &mut StdRng,
) -> usize {
    let mut visit_counts = vec![0u32; 19];
    let mut total_values = vec![0.0f64; 19];

    // Get policy prior
    let features = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19);
    let prior_logits = tch::no_grad(|| policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0));

    let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
    for &pos in avail {
        let _ = mask.i(pos as i64).fill_(0.0);
    }
    let prior = (prior_logits + &mask).softmax(0, Kind::Float);

    // MCTS simulations with PUCT
    for _ in 0..n_sims {
        let total_visits: u32 = visit_counts.iter().sum();

        // PUCT selection: Q(s,a) + c_puct * P(s,a) * sqrt(N(s)) / (1 + N(s,a))
        let best_pos = avail.iter().copied().max_by(|&a, &b| {
            let q_a = if visit_counts[a] > 0 {
                total_values[a] / visit_counts[a] as f64
            } else {
                0.5 // Optimistic prior
            };
            let q_b = if visit_counts[b] > 0 {
                total_values[b] / visit_counts[b] as f64
            } else {
                0.5
            };

            let p_a: f64 = prior.double_value(&[a as i64]);
            let p_b: f64 = prior.double_value(&[b as i64]);

            let sqrt_total = (total_visits as f64 + 1.0).sqrt();
            let u_a = q_a + c_puct * p_a * sqrt_total / (1.0 + visit_counts[a] as f64);
            let u_b = q_b + c_puct * p_b * sqrt_total / (1.0 + visit_counts[b] as f64);

            u_a.partial_cmp(&u_b).unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap_or(avail[0]);

        // Simulate move
        let mut sim_plateau = plateau.clone();
        sim_plateau.tiles[best_pos] = *tile;
        let sim_deck = replace_tile_in_deck(deck, tile);

        // Evaluate with value network + optional rollout
        let value = evaluate_position_mixed(
            &sim_plateau, &sim_deck, value_net,
            value_weight, score_mean, score_std,
            turn + 1, rng,
        );

        visit_counts[best_pos] += 1;
        total_values[best_pos] += value;
    }

    // Select action with most visits
    *avail.iter().max_by_key(|&&p| visit_counts[p]).unwrap_or(&avail[0])
}

fn evaluate_position_mixed(
    plateau: &Plateau,
    deck: &Deck,
    value_net: &GATValueNet,
    value_weight: f64,
    score_mean: f64,
    score_std: f64,
    turn: usize,
    rng: &mut StdRng,
) -> f64 {
    // For late game, just use actual score
    if turn >= 18 {
        return result(plateau) as f64 / 200.0;
    }

    let tiles = get_available_tiles(deck);
    if tiles.is_empty() {
        return result(plateau) as f64 / 200.0;
    }

    // Value network prediction
    let next_tile = tiles[0]; // Use first available tile for evaluation
    let features = convert_plateau_for_gat_47ch(plateau, &next_tile, deck, turn, 19);
    let value_pred = tch::no_grad(|| {
        value_net.forward(&features.unsqueeze(0), false).double_value(&[0, 0])
    });
    // Convert from normalized to [0, 1] range
    let value_score = ((value_pred * score_std + score_mean) / 200.0).clamp(0.0, 1.0);

    // Optional rollout for mixing
    if value_weight < 1.0 {
        let rollout_score = quick_rollout(plateau, deck, rng) as f64 / 200.0;
        value_weight * value_score + (1.0 - value_weight) * rollout_score
    } else {
        value_score
    }
}

fn quick_rollout(plateau: &Plateau, deck: &Deck, rng: &mut StdRng) -> i32 {
    let mut plateau = plateau.clone();
    let mut deck = deck.clone();

    for _ in 0..19 {
        let tiles = get_available_tiles(&deck);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(rng).unwrap();

        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }

        // Random placement
        let pos = *avail.choose(rng).unwrap();
        plateau.tiles[pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    result(&plateau)
}

fn eval_mcts_value_only(
    value_net: &GATValueNet,
    n_games: usize,
    n_sims: usize,
    score_mean: f64,
    score_std: f64,
    rng: &mut StdRng,
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

            // MCTS with uniform prior (value only)
            let best_pos = mcts_value_only_search(
                &plateau, &deck, &tile, &avail,
                value_net, n_sims, score_mean, score_std, turn, rng,
            );

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

fn mcts_value_only_search(
    plateau: &Plateau,
    deck: &Deck,
    tile: &Tile,
    avail: &[usize],
    value_net: &GATValueNet,
    n_sims: usize,
    score_mean: f64,
    score_std: f64,
    turn: usize,
    rng: &mut StdRng,
) -> usize {
    let mut visit_counts = vec![0u32; 19];
    let mut total_values = vec![0.0f64; 19];
    let c_puct = 2.0;

    for _ in 0..n_sims {
        let total_visits: u32 = visit_counts.iter().sum();

        // UCB with uniform prior
        let best_pos = avail.iter().copied().max_by(|&a, &b| {
            let q_a = if visit_counts[a] > 0 {
                total_values[a] / visit_counts[a] as f64
            } else {
                0.5
            };
            let q_b = if visit_counts[b] > 0 {
                total_values[b] / visit_counts[b] as f64
            } else {
                0.5
            };

            let sqrt_total = (total_visits as f64 + 1.0).sqrt();
            let u_a = q_a + c_puct * sqrt_total / (1.0 + visit_counts[a] as f64);
            let u_b = q_b + c_puct * sqrt_total / (1.0 + visit_counts[b] as f64);

            u_a.partial_cmp(&u_b).unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap_or(avail[0]);

        let mut sim_plateau = plateau.clone();
        sim_plateau.tiles[best_pos] = *tile;
        let sim_deck = replace_tile_in_deck(deck, tile);

        let value = evaluate_position_mixed(
            &sim_plateau, &sim_deck, value_net,
            0.8, score_mean, score_std,
            turn + 1, rng,
        );

        visit_counts[best_pos] += 1;
        total_values[best_pos] += value;
    }

    *avail.iter().max_by_key(|&&p| visit_counts[p]).unwrap_or(&avail[0])
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
