//! Evaluate Ensemble GAT Policy + Value with MCTS
//!
//! Uses 5-model ensemble for policy priors in MCTS.

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
#[command(name = "eval_ensemble_mcts")]
struct Args {
    /// Model paths (comma-separated)
    #[arg(long, default_value = "model_weights/gat_seed42_policy.safetensors,model_weights/gat_seed123_policy.safetensors,model_weights/gat_seed456_policy.safetensors,model_weights/gat_seed789_policy.safetensors,model_weights/gat_seed2024_policy.safetensors")]
    models: String,

    /// Value model path
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

    /// MCTS simulations
    #[arg(long, default_value_t = 100)]
    simulations: usize,

    /// PUCT constant
    #[arg(long, default_value_t = 1.5)]
    c_puct: f64,

    /// Score normalization
    #[arg(long, default_value_t = 140.0)]
    score_mean: f64,

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
    println!("║      Ensemble Policy + MCTS Evaluation                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let hidden: Vec<i64> = args.hidden.split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    let model_paths: Vec<&str> = args.models.split(',').map(|s| s.trim()).collect();

    // Load policy models
    println!("Loading {} policy models...", model_paths.len());
    let mut policy_nets = Vec::new();
    for (i, path) in model_paths.iter().enumerate() {
        let mut vs = nn::VarStore::new(device);
        let net = GATPolicyNet::new(&vs, 47, &hidden, args.heads, 0.0);
        match load_varstore(&mut vs, path) {
            Ok(_) => {
                println!("  [{}] {} ✓", i + 1, path);
                policy_nets.push((vs, net));
            }
            Err(e) => println!("  [{}] {} FAILED: {}", i + 1, path, e),
        }
    }

    // Load value network
    println!("Loading value network...");
    let mut vs_value = nn::VarStore::new(device);
    let value_net = GATValueNet::new(&vs_value, 47, &hidden, args.heads, 0.0);
    if let Err(e) = load_varstore(&mut vs_value, &args.value_model) {
        println!("Failed to load value: {}", e);
        return;
    }
    println!("  Value: {} ✓", args.value_model);

    println!("\nConfig:");
    println!("  Models:      {} ensemble", policy_nets.len());
    println!("  Games:       {}", args.games);
    println!("  Simulations: {}", args.simulations);
    println!("  C_PUCT:      {}", args.c_puct);

    // Evaluate
    println!("\n📊 Evaluating...\n");

    // 1. Ensemble policy only
    let mut rng1 = StdRng::seed_from_u64(args.seed);
    let start = Instant::now();
    let (ensemble_avg, ensemble_scores) = eval_ensemble_policy(&policy_nets, args.games, &mut rng1);
    let t1 = start.elapsed().as_secs_f32();

    // 2. Ensemble + MCTS
    let mut rng2 = StdRng::seed_from_u64(args.seed);
    let start = Instant::now();
    let (mcts_avg, mcts_scores) = eval_ensemble_mcts(
        &policy_nets, &value_net, args.games, args.simulations,
        args.c_puct, args.score_mean, args.score_std, &mut rng2
    );
    let t2 = start.elapsed().as_secs_f32();

    // 3. Greedy
    let mut rng3 = StdRng::seed_from_u64(args.seed);
    let (greedy_avg, _) = eval_greedy(args.games, &mut rng3);

    // Results
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                     RESULTS                                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let ge140_e = ensemble_scores.iter().filter(|&&s| s >= 140).count();
    let ge150_e = ensemble_scores.iter().filter(|&&s| s >= 150).count();
    let ge140_m = mcts_scores.iter().filter(|&&s| s >= 140).count();
    let ge150_m = mcts_scores.iter().filter(|&&s| s >= 150).count();

    println!("  Strategy                    | Avg Score | ≥140 pts | ≥150 pts | Time");
    println!("  ----------------------------|-----------|----------|----------|------");
    println!("  Ensemble ({} models)        | {:9.2} | {:6.1}%  | {:6.1}%  | {:.1}s",
        policy_nets.len(), ensemble_avg,
        100.0 * ge140_e as f64 / args.games as f64,
        100.0 * ge150_e as f64 / args.games as f64, t1);
    println!("  Ensemble + MCTS ({}sim)    | {:9.2} | {:6.1}%  | {:6.1}%  | {:.1}s",
        args.simulations, mcts_avg,
        100.0 * ge140_m as f64 / args.games as f64,
        100.0 * ge150_m as f64 / args.games as f64, t2);
    println!("  Greedy baseline             | {:9.2} |    -     |    -     |  -", greedy_avg);

    println!("\n  Ensemble + MCTS vs Ensemble: {:+.2} pts", mcts_avg - ensemble_avg);
}

fn eval_ensemble_policy(
    models: &[(nn::VarStore, GATPolicyNet)],
    n_games: usize,
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

            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let features_batch = features.unsqueeze(0);

            // Average logits from all models
            let mut sum_logits: Option<Tensor> = None;
            for (_, net) in models.iter() {
                let logits = tch::no_grad(|| net.forward(&features_batch, false).squeeze_dim(0));
                sum_logits = Some(match sum_logits {
                    None => logits,
                    Some(s) => s + logits,
                });
            }
            let avg_logits = sum_logits.unwrap() / models.len() as f64;

            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
            let best_pos = (avg_logits + mask).argmax(0, false).int64_value(&[]) as usize;

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

fn eval_ensemble_mcts(
    models: &[(nn::VarStore, GATPolicyNet)],
    value_net: &GATValueNet,
    n_games: usize,
    n_sims: usize,
    c_puct: f64,
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

            let best_pos = ensemble_mcts_search(
                &plateau, &deck, &tile, &avail,
                models, value_net, n_sims, c_puct,
                score_mean, score_std, turn, rng,
            );

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}

fn ensemble_mcts_search(
    plateau: &Plateau,
    deck: &Deck,
    tile: &Tile,
    avail: &[usize],
    models: &[(nn::VarStore, GATPolicyNet)],
    value_net: &GATValueNet,
    n_sims: usize,
    c_puct: f64,
    score_mean: f64,
    score_std: f64,
    turn: usize,
    rng: &mut StdRng,
) -> usize {
    let mut visit_counts = vec![0u32; 19];
    let mut total_values = vec![0.0f64; 19];

    // Get ensemble prior
    let features = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19);
    let features_batch = features.unsqueeze(0);

    let mut sum_logits: Option<Tensor> = None;
    for (_, net) in models.iter() {
        let logits = tch::no_grad(|| net.forward(&features_batch, false).squeeze_dim(0));
        sum_logits = Some(match sum_logits {
            None => logits,
            Some(s) => s + logits,
        });
    }
    let avg_logits = sum_logits.unwrap() / models.len() as f64;

    let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
    for &pos in avail { let _ = mask.i(pos as i64).fill_(0.0); }
    let prior = (avg_logits + &mask).softmax(0, Kind::Float);

    // MCTS with PUCT
    for _ in 0..n_sims {
        let total_visits: u32 = visit_counts.iter().sum();

        let best_pos = avail.iter().copied().max_by(|&a, &b| {
            let q_a = if visit_counts[a] > 0 {
                total_values[a] / visit_counts[a] as f64
            } else { 0.5 };
            let q_b = if visit_counts[b] > 0 {
                total_values[b] / visit_counts[b] as f64
            } else { 0.5 };

            let p_a: f64 = prior.double_value(&[a as i64]);
            let p_b: f64 = prior.double_value(&[b as i64]);

            let sqrt_total = (total_visits as f64 + 1.0).sqrt();
            let u_a = q_a + c_puct * p_a * sqrt_total / (1.0 + visit_counts[a] as f64);
            let u_b = q_b + c_puct * p_b * sqrt_total / (1.0 + visit_counts[b] as f64);

            u_a.partial_cmp(&u_b).unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap_or(avail[0]);

        let mut sim_plateau = plateau.clone();
        sim_plateau.tiles[best_pos] = *tile;
        let sim_deck = replace_tile_in_deck(deck, tile);

        let value = evaluate_with_value_net(
            &sim_plateau, &sim_deck, value_net,
            score_mean, score_std, turn + 1, rng,
        );

        visit_counts[best_pos] += 1;
        total_values[best_pos] += value;
    }

    *avail.iter().max_by_key(|&&p| visit_counts[p]).unwrap_or(&avail[0])
}

fn evaluate_with_value_net(
    plateau: &Plateau,
    deck: &Deck,
    value_net: &GATValueNet,
    score_mean: f64,
    score_std: f64,
    turn: usize,
    rng: &mut StdRng,
) -> f64 {
    if turn >= 18 {
        return result(plateau) as f64 / 200.0;
    }

    let tiles = get_available_tiles(deck);
    if tiles.is_empty() {
        return result(plateau) as f64 / 200.0;
    }

    let next_tile = tiles[0];
    let features = convert_plateau_for_gat_47ch(plateau, &next_tile, deck, turn, 19);
    let value_pred = tch::no_grad(|| {
        value_net.forward(&features.unsqueeze(0), false).double_value(&[0, 0])
    });

    let value_score = ((value_pred * score_std + score_mean) / 200.0).clamp(0.0, 1.0);

    // Mix with quick rollout
    let rollout = quick_rollout(plateau, deck, rng) as f64 / 200.0;
    0.7 * value_score + 0.3 * rollout
}

fn quick_rollout(plateau: &Plateau, deck: &Deck, rng: &mut StdRng) -> i32 {
    let mut plateau = plateau.clone();
    let mut deck = deck.clone();

    loop {
        let tiles = get_available_tiles(&deck);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(rng).unwrap();

        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }

        let pos = *avail.choose(rng).unwrap();
        plateau.tiles[pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    result(&plateau)
}

fn eval_greedy(n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
    let mut scores = Vec::new();

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        loop {
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
                if s > best_score { best_score = s; best_pos = pos; }
            }

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        scores.push(result(&plateau));
    }

    (scores.iter().sum::<i32>() as f64 / scores.len() as f64, scores)
}
