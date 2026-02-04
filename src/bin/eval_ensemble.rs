//! Ensemble evaluation for GAT models
//!
//! Loads multiple trained GAT models and evaluates their ensemble performance
//! by averaging policy logits across all models.

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use tch::{nn, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::gat::GATPolicyNet;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "eval_ensemble")]
#[command(about = "Evaluate GAT ensemble from multiple trained models")]
struct Args {
    /// Model paths (comma-separated)
    #[arg(long, default_value = "model_weights/gat_seed42_policy.pt,model_weights/gat_seed123_policy.pt,model_weights/gat_seed456_policy.pt,model_weights/gat_seed789_policy.pt,model_weights/gat_seed2024_policy.pt")]
    models: String,

    /// Hidden layer sizes
    #[arg(long, default_value = "128,128")]
    hidden: String,

    /// Number of attention heads
    #[arg(long, default_value_t = 4)]
    heads: usize,

    /// Number of games to play
    #[arg(long, default_value_t = 500)]
    games: usize,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

fn main() {
    let args = Args::parse();
    let device = Device::Cpu;

    // Parse hidden sizes
    let hidden_sizes: Vec<i64> = args
        .hidden
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    // Parse model paths
    let model_paths: Vec<&str> = args.models.split(',').map(|s| s.trim()).collect();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           GAT Ensemble Evaluation                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Load all models
    println!("Loading {} models...", model_paths.len());
    let mut models = Vec::new();
    for (i, path) in model_paths.iter().enumerate() {
        let mut vs = nn::VarStore::new(device);
        let net = GATPolicyNet::new(&vs, 47, &hidden_sizes, args.heads, 0.0);
        match vs.load(path) {
            Ok(_) => {
                println!("  [{}] {} ✓", i + 1, path);
                models.push((vs, net));
            }
            Err(e) => {
                println!("  [{}] {} FAILED: {}", i + 1, path, e);
            }
        }
    }

    if models.is_empty() {
        println!("No models loaded. Exiting.");
        return;
    }

    println!();
    println!("Loaded {} models successfully.", models.len());
    println!();

    // Evaluate ensemble
    let mut rng = StdRng::seed_from_u64(args.seed);

    println!("Playing {} games with ensemble...", args.games);
    let (ensemble_avg, ensemble_scores) = eval_games_ensemble(&models, args.games, &mut rng);

    // Also evaluate individual models for comparison
    println!();
    println!("Comparing individual models:");
    let mut individual_scores = Vec::new();
    for (i, (_, net)) in models.iter().enumerate() {
        let mut rng2 = StdRng::seed_from_u64(args.seed); // Same seed for fair comparison
        let (avg, _) = eval_games_single(net, args.games, &mut rng2);
        println!("  Model {} (seed {}): {:.2} pts", i + 1,
            model_paths[i].split("seed").last().unwrap_or("?").replace("_policy.pt", ""),
            avg);
        individual_scores.push(avg);
    }

    // Evaluate greedy baseline
    let mut rng_greedy = StdRng::seed_from_u64(args.seed);
    let (greedy_avg, _) = eval_games_greedy(args.games, &mut rng_greedy);

    // Statistics
    let games_100 = ensemble_scores.iter().filter(|&&s| s >= 100).count();
    let games_140 = ensemble_scores.iter().filter(|&&s| s >= 140).count();
    let games_150 = ensemble_scores.iter().filter(|&&s| s >= 150).count();
    let games_160 = ensemble_scores.iter().filter(|&&s| s >= 160).count();
    let max_score = ensemble_scores.iter().max().unwrap_or(&0);
    let min_score = ensemble_scores.iter().min().unwrap_or(&0);

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                     RESULTS                                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Ensemble ({} models): {:.2} pts", models.len(), ensemble_avg);
    println!("  Best individual:      {:.2} pts", individual_scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    println!("  Average individual:   {:.2} pts", individual_scores.iter().sum::<f64>() / individual_scores.len() as f64);
    println!("  Greedy baseline:      {:.2} pts", greedy_avg);
    println!();
    println!("  vs Greedy: +{:.2} pts", ensemble_avg - greedy_avg);
    println!("  vs Best individual: {:+.2} pts", ensemble_avg - individual_scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    println!();
    println!("  Score range: {} - {}", min_score, max_score);
    println!("  Games >= 100 pts: {} ({:.1}%)", games_100, 100.0 * games_100 as f64 / args.games as f64);
    println!("  Games >= 140 pts: {} ({:.1}%)", games_140, 100.0 * games_140 as f64 / args.games as f64);
    println!("  Games >= 150 pts: {} ({:.1}%)", games_150, 100.0 * games_150 as f64 / args.games as f64);
    println!("  Games >= 160 pts: {} ({:.1}%)", games_160, 100.0 * games_160 as f64 / args.games as f64);
}

fn eval_games_ensemble(models: &[(nn::VarStore, GATPolicyNet)], n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
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

            // Get logits from all models and average
            let mut sum_logits: Option<Tensor> = None;
            for (_, net) in models.iter() {
                let logits = net.forward(&features_batch, false).squeeze_dim(0);
                sum_logits = Some(match sum_logits {
                    None => logits,
                    Some(s) => s + logits,
                });
            }
            let avg_logits = sum_logits.unwrap() / models.len() as f64;

            // Apply mask and select best position
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

fn eval_games_single(net: &GATPolicyNet, n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
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
            let logits = net.forward(&features.unsqueeze(0), false).squeeze_dim(0);

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

fn eval_games_greedy(n_games: usize, rng: &mut StdRng) -> (f64, Vec<i32>) {
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

            // Simple greedy: pick the position that maximizes immediate score
            let mut best_pos = avail[0];
            let mut best_score = i32::MIN;
            for &pos in &avail {
                let mut test_plateau = plateau.clone();
                test_plateau.tiles[pos] = tile;
                let score = result(&test_plateau);
                if score > best_score {
                    best_score = score;
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
