//! Compare GAT Supervised vs CNN + MCTS + Q-Net
//!
//! This benchmark compares:
//! 1. GAT Supervised (trained on >140 pts games) - standalone
//! 2. GAT Supervised + MCTS (hybrid)
//! 3. CNN + Q-net MCTS (current production)
//!
//! Usage: cargo run --release --bin compare_gat_supervised_vs_cnn -- --games 100 --simulations 200

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::time::Instant;
use tch::{nn, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::deck::Deck;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_pure;
use take_it_easy::neural::gat::GATPolicyNet;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::neural::{NeuralConfig, NeuralManager, QNetManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "compare_gat_supervised_vs_cnn")]
struct Args {
    /// Number of games to evaluate
    #[arg(long, default_value_t = 100)]
    games: usize,

    /// MCTS simulations
    #[arg(long, default_value_t = 200)]
    simulations: usize,

    /// Top-K positions to consider
    #[arg(long, default_value_t = 6)]
    top_k: usize,

    /// GAT model path
    #[arg(long, default_value = "model_weights/gat_supervised_policy.pt")]
    gat_model: String,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     GAT Supervised vs CNN + MCTS + Q-Net Comparison          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config: {} games, {} simulations, top-K={}", args.games, args.simulations, args.top_k);

    // 1. Load GAT Supervised
    println!("\n🔷 Loading GAT Supervised model...");
    let mut vs_gat = nn::VarStore::new(Device::Cpu);
    let gat_policy = GATPolicyNet::new(&vs_gat, 47, &[128, 128], 4, 0.1);

    let gat_loaded = match vs_gat.load(&args.gat_model) {
        Ok(_) => {
            println!("   ✅ Loaded from {}", args.gat_model);
            true
        }
        Err(e) => {
            println!("   ⚠️ Could not load GAT: {} - will skip GAT tests", e);
            false
        }
    };

    // 2. Load CNN + Q-Net
    println!("\n🔶 Loading CNN + Q-Net...");
    let neural_config = NeuralConfig::default();
    let neural_manager = match NeuralManager::with_config(neural_config) {
        Ok(m) => {
            println!("   ✅ Loaded CNN policy/value networks");
            Some(m)
        }
        Err(e) => {
            println!("   ⚠️ Could not load CNN: {}", e);
            None
        }
    };

    let qnet_manager = match QNetManager::new("model_weights/qvalue_net.params") {
        Ok(m) => {
            println!("   ✅ Loaded Q-Net");
            Some(m)
        }
        Err(e) => {
            println!("   ⚠️ Could not load Q-net: {}", e);
            None
        }
    };

    // 3. Evaluate
    println!("\n📊 Evaluating on {} games...\n", args.games);

    let mut rng = StdRng::seed_from_u64(args.seed);

    let mut results: Vec<(&str, Vec<i32>, f64)> = Vec::new();

    // Baselines
    print!("   Random...");
    let start = Instant::now();
    let random_scores: Vec<i32> = (0..args.games)
        .map(|i| {
            let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
            play_random(&tiles, &mut StdRng::seed_from_u64(args.seed + 10000 + i as u64))
        })
        .collect();
    println!(" {:.1}s", start.elapsed().as_secs_f32());
    results.push(("Random", random_scores.clone(), 0.0));

    print!("   Greedy...");
    let start = Instant::now();
    let greedy_scores: Vec<i32> = (0..args.games)
        .map(|i| {
            let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
            play_greedy(&tiles)
        })
        .collect();
    println!(" {:.1}s", start.elapsed().as_secs_f32());
    results.push(("Greedy", greedy_scores.clone(), 0.0));

    // GAT Supervised standalone
    if gat_loaded {
        print!("   GAT Supervised (standalone)...");
        let start = Instant::now();
        let gat_scores: Vec<i32> = (0..args.games)
            .map(|i| {
                let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
                play_gat_policy(&tiles, &gat_policy)
            })
            .collect();
        let elapsed = start.elapsed().as_secs_f32();
        println!(" {:.1}s", elapsed);
        results.push(("GAT Supervised", gat_scores, elapsed as f64));

        // GAT + MCTS hybrid
        print!("   GAT + MCTS...");
        let start = Instant::now();
        let gat_mcts_scores: Vec<i32> = (0..args.games)
            .map(|i| {
                let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
                play_gat_mcts(&tiles, args.simulations, &gat_policy, args.top_k)
            })
            .collect();
        let elapsed = start.elapsed().as_secs_f32();
        println!(" {:.1}s", elapsed);
        results.push(("GAT + MCTS", gat_mcts_scores, elapsed as f64));
    }

    // CNN + Q-Net
    if let (Some(ref nm), Some(ref qm)) = (&neural_manager, &qnet_manager) {
        print!("   CNN + Q-Net MCTS...");
        let start = Instant::now();
        let cnn_scores: Vec<i32> = (0..args.games)
            .map(|i| {
                let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
                play_cnn_qnet(&tiles, args.simulations, nm, qm, args.top_k)
            })
            .collect();
        let elapsed = start.elapsed().as_secs_f32();
        println!(" {:.1}s", elapsed);
        results.push(("CNN + Q-Net MCTS", cnn_scores, elapsed as f64));
    }

    // Pure MCTS baseline
    print!("   Pure MCTS...");
    let start = Instant::now();
    let mcts_scores: Vec<i32> = (0..args.games)
        .map(|i| {
            let tiles = sample_tiles(&mut StdRng::seed_from_u64(args.seed + i as u64), 19);
            play_pure_mcts(&tiles, args.simulations)
        })
        .collect();
    let elapsed = start.elapsed().as_secs_f32();
    println!(" {:.1}s", elapsed);
    results.push(("Pure MCTS", mcts_scores, elapsed as f64));

    // 4. Results table
    let greedy_mean = mean(&greedy_scores);

    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                            RESULTS                                   ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");
    println!("║  Method                │   Avg   │   Min   │   Max   │ vs Greedy     ║");
    println!("╠══════════════════════════════════════════════════════════════════════╣");

    for (name, scores, _time) in &results {
        let avg = mean(scores);
        let min = *scores.iter().min().unwrap_or(&0);
        let max = *scores.iter().max().unwrap_or(&0);
        let vs_greedy = avg - greedy_mean;
        println!("║  {:20} │ {:>7.2} │ {:>7} │ {:>7} │ {:>+7.2}       ║",
                 name, avg, min, max, vs_greedy);
    }

    println!("╚══════════════════════════════════════════════════════════════════════╝");

    // Find best method
    let best = results.iter()
        .max_by(|a, b| mean(&a.1).partial_cmp(&mean(&b.1)).unwrap())
        .map(|(name, scores, _)| (name, mean(scores)));

    if let Some((name, score)) = best {
        println!("\n🏆 Best method: {} with {:.2} pts", name, score);
    }

    // GAT vs CNN comparison
    let gat_supervised = results.iter().find(|(n, _, _)| *n == "GAT Supervised");
    let gat_mcts = results.iter().find(|(n, _, _)| *n == "GAT + MCTS");
    let cnn_qnet = results.iter().find(|(n, _, _)| *n == "CNN + Q-Net MCTS");

    if let (Some((_, gat_scores, _)), Some((_, cnn_scores, _))) = (gat_mcts, cnn_qnet) {
        let gat_avg = mean(gat_scores);
        let cnn_avg = mean(cnn_scores);
        println!("\n📊 GAT + MCTS vs CNN + Q-Net: {:+.2} pts", gat_avg - cnn_avg);

        if gat_avg > cnn_avg + 2.0 {
            println!("   🎉 GAT + MCTS BEATS CNN + Q-Net!");
        } else if gat_avg > cnn_avg - 2.0 {
            println!("   ➖ Performance comparable");
        } else {
            println!("   📉 CNN + Q-Net still ahead");
        }
    }

    if let Some((_, gat_scores, _)) = gat_supervised {
        let gat_avg = mean(gat_scores);
        let above_100 = gat_scores.iter().filter(|&&s| s >= 100).count();
        let above_140 = gat_scores.iter().filter(|&&s| s >= 140).count();
        println!("\n📈 GAT Supervised stats:");
        println!("   Games >= 100 pts: {} ({:.1}%)", above_100, above_100 as f64 / args.games as f64 * 100.0);
        println!("   Games >= 140 pts: {} ({:.1}%)", above_140, above_140 as f64 / args.games as f64 * 100.0);
    }
}

fn sample_tiles(rng: &mut StdRng, count: usize) -> Vec<Tile> {
    let mut deck = create_deck();
    let mut tiles = Vec::new();

    for _ in 0..count {
        let available = get_available_tiles(&deck);
        if available.is_empty() { break; }
        let tile = *available.choose(rng).unwrap();
        tiles.push(tile);
        deck = replace_tile_in_deck(&deck, &tile);
    }

    tiles
}

fn play_random(tiles: &[Tile], rng: &mut StdRng) -> i32 {
    let mut plateau = create_plateau_empty();
    for tile in tiles {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }
        let pos = *avail.choose(rng).unwrap();
        plateau.tiles[pos] = *tile;
    }
    result(&plateau)
}

fn play_greedy(tiles: &[Tile]) -> i32 {
    let mut plateau = create_plateau_empty();
    for tile in tiles {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }
        let pos = find_greedy(&plateau, tile, &avail);
        plateau.tiles[pos] = *tile;
    }
    result(&plateau)
}

fn find_greedy(p: &Plateau, t: &Tile, a: &[usize]) -> usize {
    a.iter().copied().max_by_key(|&pos| {
        let mut test = p.clone();
        test.tiles[pos] = *t;
        result(&test)
    }).unwrap_or(a[0])
}

fn play_gat_policy(tiles: &[Tile], policy: &GATPolicyNet) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }

        let features = convert_plateau_for_gat_47ch(&plateau, tile, &deck, turn, 19);
        let logits = policy.forward(&features.unsqueeze(0), false);

        let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
        for &pos in &avail {
            let _ = mask.i(pos as i64).fill_(0.0);
        }
        let best: i64 = (logits.squeeze_dim(0) + mask).argmax(0, false).int64_value(&[]);

        plateau.tiles[best as usize] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_gat_mcts(tiles: &[Tile], num_sims: usize, policy: &GATPolicyNet, top_k: usize) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }

        // Use GAT policy for pruning
        let should_prune = avail.len() > top_k + 2;

        let best_pos = if should_prune {
            let features = convert_plateau_for_gat_47ch(&plateau, tile, &deck, turn, 19);
            let logits = policy.forward(&features.unsqueeze(0), false).squeeze_dim(0);

            let mut scored: Vec<(usize, f64)> = avail.iter()
                .map(|&pos| (pos, logits.double_value(&[pos as i64])))
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_positions: Vec<usize> = scored.iter().take(top_k).map(|(pos, _)| *pos).collect();

            let sims_per_pos = num_sims / top_positions.len().max(1);
            let mut best = top_positions[0];
            let mut best_score = f64::NEG_INFINITY;

            for &pos in &top_positions {
                let mut temp_plateau = plateau.clone();
                temp_plateau.tiles[pos] = *tile;
                let temp_deck = replace_tile_in_deck(&deck, tile);

                let mut total = 0.0;
                for _ in 0..sims_per_pos {
                    total += simulate_random_game(&temp_plateau, &temp_deck) as f64;
                }
                let avg = total / sims_per_pos as f64;

                if avg > best_score {
                    best_score = avg;
                    best = pos;
                }
            }
            best
        } else {
            let result = mcts_find_best_position_for_tile_pure(
                &mut plateau.clone(),
                &mut deck.clone(),
                *tile,
                num_sims,
                turn,
                19,
                None,
            );
            result.best_position
        };

        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_cnn_qnet(
    tiles: &[Tile],
    num_sims: usize,
    neural_manager: &NeuralManager,
    qnet_manager: &QNetManager,
    top_k: usize,
) -> i32 {
    use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_qnet;

    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    let policy_net = neural_manager.policy_net();
    let value_net = neural_manager.value_net();
    let qvalue_net = qnet_manager.net();

    for (turn, tile) in tiles.iter().enumerate() {
        let result = mcts_find_best_position_for_tile_with_qnet(
            &mut plateau,
            &mut deck,
            *tile,
            policy_net,
            value_net,
            qvalue_net,
            num_sims,
            turn,
            19,
            top_k,
            None,
        );
        plateau.tiles[result.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn play_pure_mcts(tiles: &[Tile], num_sims: usize) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let result = mcts_find_best_position_for_tile_pure(
            &mut plateau,
            &mut deck,
            *tile,
            num_sims,
            turn,
            19,
            None,
        );
        plateau.tiles[result.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    result(&plateau)
}

fn simulate_random_game(plateau: &Plateau, deck: &Deck) -> i32 {
    let mut rng = rand::rng();
    let mut plateau = plateau.clone();
    let mut deck = deck.clone();

    loop {
        let tiles = get_available_tiles(&deck);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(&mut rng).unwrap();

        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }
        let pos = *avail.choose(&mut rng).unwrap();

        plateau.tiles[pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    result(&plateau)
}

fn mean(values: &[i32]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<i32>() as f64 / values.len() as f64
}
