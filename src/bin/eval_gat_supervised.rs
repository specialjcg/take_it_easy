//! Evaluate GAT policy trained with supervised learning
//!
//! Usage: cargo run --release --bin eval_gat_supervised -- --games 200

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use tch::{nn, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::gat::GATPolicyNet;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "eval_gat_supervised")]
struct Args {
    /// Number of games to play
    #[arg(long, default_value_t = 200)]
    games: usize,

    /// Model path
    #[arg(long, default_value = "model_weights/gat_supervised_policy.pt")]
    model: String,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Show individual game scores
    #[arg(long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           GAT Supervised Policy Evaluation                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let device = Device::Cpu;

    // Load model
    let mut vs = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs, 47, &[128, 128], 4, 0.1);

    match vs.load(&args.model) {
        Ok(_) => println!("✅ Loaded model from {}", args.model),
        Err(e) => {
            println!("❌ Could not load model: {}", e);
            return;
        }
    }

    let mut rng = StdRng::seed_from_u64(args.seed);

    // Evaluate GAT
    println!("\n🎮 Playing {} games with GAT policy...\n", args.games);
    let (gat_scores, gat_avg) = eval_policy(&policy_net, args.games, &mut rng, args.verbose);

    // Evaluate baselines
    println!("\n📊 Evaluating baselines...");
    let greedy_avg = eval_greedy(args.games, args.seed);
    let random_avg = eval_random(args.games, args.seed);

    // Results
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                        RESULTS                               ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("  GAT Supervised: {:.2} pts", gat_avg);
    println!("  Greedy:         {:.2} pts", greedy_avg);
    println!("  Random:         {:.2} pts", random_avg);
    println!();
    println!("  vs Greedy: {:+.2} pts", gat_avg - greedy_avg);
    println!("  vs Random: {:+.2} pts", gat_avg - random_avg);

    // Score distribution
    let min_score = *gat_scores.iter().min().unwrap();
    let max_score = *gat_scores.iter().max().unwrap();
    let above_100: usize = gat_scores.iter().filter(|&&s| s >= 100).count();
    let above_140: usize = gat_scores.iter().filter(|&&s| s >= 140).count();

    println!("\n  Score range: {} - {}", min_score, max_score);
    println!("  Games >= 100 pts: {} ({:.1}%)", above_100, above_100 as f64 / args.games as f64 * 100.0);
    println!("  Games >= 140 pts: {} ({:.1}%)", above_140, above_140 as f64 / args.games as f64 * 100.0);

    if gat_avg > greedy_avg {
        println!("\n  🏆 GAT BEATS GREEDY!");
    }
}

/// Evaluate GAT policy
fn eval_policy(
    policy_net: &GATPolicyNet,
    n_games: usize,
    rng: &mut StdRng,
    verbose: bool,
) -> (Vec<i32>, f64) {
    let mut scores = Vec::new();

    for game_i in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(rng).unwrap();

            let avail: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();
            if avail.is_empty() { break; }

            // Get features and policy prediction
            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let logits = policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0);

            // Mask unavailable positions
            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail {
                let _ = mask.i(pos as i64).fill_(0.0);
            }
            let masked_logits = logits + mask;

            // Select best position
            let best_pos = masked_logits.argmax(0, false).int64_value(&[]) as usize;

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        let score = result(&plateau);
        scores.push(score);

        if verbose {
            println!("  Game {}: {} pts", game_i + 1, score);
        }
    }

    let avg = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    (scores, avg)
}

/// Evaluate greedy baseline
fn eval_greedy(n_games: usize, seed: u64) -> f64 {
    let mut rng = StdRng::seed_from_u64(seed + 10000);
    let mut total = 0;

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for _ in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();

            let avail: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();
            if avail.is_empty() { break; }

            // Greedy: maximize immediate score
            let best_pos = avail.iter().copied().max_by_key(|&pos| {
                let mut test = plateau.clone();
                test.tiles[pos] = tile;
                result(&test)
            }).unwrap_or(avail[0]);

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total += result(&plateau);
    }

    total as f64 / n_games as f64
}

/// Evaluate random baseline
fn eval_random(n_games: usize, seed: u64) -> f64 {
    let mut rng = StdRng::seed_from_u64(seed + 20000);
    let mut total = 0;

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for _ in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();

            let avail: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();
            if avail.is_empty() { break; }

            let pos = *avail.choose(&mut rng).unwrap();
            plateau.tiles[pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total += result(&plateau);
    }

    total as f64 / n_games as f64
}
