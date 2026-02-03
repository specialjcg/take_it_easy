//! Compare GAT vs CNN with MCTS
//!
//! This benchmark compares:
//! 1. Pure MCTS (baseline)
//! 2. CNN + Q-net pruning (current best)
//! 3. GAT policy standalone (no MCTS)
//! 4. GAT + MCTS (new hybrid)
//!
//! Usage: cargo run --release --bin compare_gat_vs_cnn_mcts -- --games 50 --simulations 100

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, IndexOp, Kind, Tensor};

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
#[command(name = "compare_gat_vs_cnn_mcts")]
struct Args {
    #[arg(long, default_value_t = 50)]
    games: usize,

    #[arg(long, default_value_t = 100)]
    simulations: usize,

    #[arg(long, default_value_t = 500)]
    train_games: usize,

    #[arg(long, default_value_t = 100)]
    epochs: usize,

    #[arg(long, default_value_t = 6)]
    top_k: usize,

    #[arg(long, default_value_t = 42)]
    seed: u64,
}

struct TrainSample {
    features: Tensor,
    best_pos: i64,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          GAT vs CNN with MCTS Comparison                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config: {} games, {} simulations, top-K={}", args.games, args.simulations, args.top_k);

    // 1. Train GAT from scratch using greedy data
    println!("\n🔷 Training GAT (47ch) from {} greedy games...", args.train_games);
    let start = Instant::now();
    let gat_policy = train_gat_networks(args.train_games, args.epochs, args.seed);
    println!("   Trained in {:.2}s", start.elapsed().as_secs_f32());

    // 2. Load CNN networks
    println!("\n🔶 Loading CNN networks...");
    let neural_config = NeuralConfig::default();
    let neural_manager = match NeuralManager::with_config(neural_config) {
        Ok(m) => Some(m),
        Err(e) => {
            println!("   Warning: Could not load CNN: {}", e);
            None
        }
    };

    let qnet_manager = match QNetManager::new("model_weights/qvalue_net.params") {
        Ok(m) => Some(m),
        Err(e) => {
            println!("   Warning: Could not load Q-net: {}", e);
            None
        }
    };

    // 3. Run evaluation
    println!("\n📊 Evaluating on {} games...", args.games);

    let mut rng = StdRng::seed_from_u64(args.seed + 10000);

    let mut random_scores = Vec::new();
    let mut greedy_scores = Vec::new();
    let mut pure_mcts_scores = Vec::new();
    let mut gat_policy_scores = Vec::new();
    let mut gat_mcts_scores = Vec::new();
    let mut cnn_mcts_scores = Vec::new();

    for game_idx in 0..args.games {
        let tiles = sample_tiles(&mut rng, 19);

        // Random baseline
        let mut rng2 = StdRng::seed_from_u64(args.seed + 10000 + game_idx as u64);
        random_scores.push(play_random(&tiles, &mut rng2));

        // Greedy baseline
        greedy_scores.push(play_greedy(&tiles));

        // Pure MCTS
        pure_mcts_scores.push(play_pure_mcts(&tiles, args.simulations));

        // GAT policy only (no MCTS)
        gat_policy_scores.push(play_gat_policy(&tiles, &gat_policy));

        // GAT + MCTS hybrid (use policy for pruning)
        gat_mcts_scores.push(play_gat_mcts(&tiles, args.simulations, &gat_policy, args.top_k));

        // CNN + Q-net (if available)
        if let (Some(ref nm), Some(ref qm)) = (&neural_manager, &qnet_manager) {
            cnn_mcts_scores.push(play_cnn_qnet(&tiles, args.simulations, nm, qm, args.top_k));
        }

        if (game_idx + 1) % 10 == 0 {
            print!("   Game {}/{}...\r", game_idx + 1, args.games);
        }
    }
    println!();

    // 4. Results
    let random_mean = mean(&random_scores);
    let greedy_mean = mean(&greedy_scores);
    let pure_mcts_mean = mean(&pure_mcts_scores);
    let gat_policy_mean = mean(&gat_policy_scores);
    let gat_mcts_mean = mean(&gat_mcts_scores);

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                        RESULTS                                ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Method              │  Score  │ vs Random │ vs Greedy       ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Random              │ {:>6.2}  │     -     │  {:>+6.2}         ║", random_mean, random_mean - greedy_mean);
    println!("║  Greedy              │ {:>6.2}  │  {:>+6.2}   │     -           ║", greedy_mean, greedy_mean - random_mean);
    println!("║  Pure MCTS ({:>3} sim) │ {:>6.2}  │  {:>+6.2}   │  {:>+6.2}         ║", args.simulations, pure_mcts_mean, pure_mcts_mean - random_mean, pure_mcts_mean - greedy_mean);
    println!("║  GAT Policy (47ch)   │ {:>6.2}  │  {:>+6.2}   │  {:>+6.2}         ║", gat_policy_mean, gat_policy_mean - random_mean, gat_policy_mean - greedy_mean);
    println!("║  GAT + MCTS          │ {:>6.2}  │  {:>+6.2}   │  {:>+6.2}         ║", gat_mcts_mean, gat_mcts_mean - random_mean, gat_mcts_mean - greedy_mean);

    if !cnn_mcts_scores.is_empty() {
        let cnn_mcts_mean = mean(&cnn_mcts_scores);
        println!("║  CNN + Q-net         │ {:>6.2}  │  {:>+6.2}   │  {:>+6.2}         ║", cnn_mcts_mean, cnn_mcts_mean - random_mean, cnn_mcts_mean - greedy_mean);
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    // Analysis
    println!("\n📈 Analysis:");
    println!("   GAT Policy vs Greedy: {:>+.2} pts", gat_policy_mean - greedy_mean);
    println!("   GAT + MCTS vs Pure MCTS: {:>+.2} pts", gat_mcts_mean - pure_mcts_mean);

    if !cnn_mcts_scores.is_empty() {
        let cnn_mcts_mean = mean(&cnn_mcts_scores);
        println!("   GAT + MCTS vs CNN + Q-net: {:>+.2} pts", gat_mcts_mean - cnn_mcts_mean);

        if gat_mcts_mean > cnn_mcts_mean + 1.0 {
            println!("\n   🏆 GAT+MCTS BEATS CNN+Q-net!");
        } else if gat_mcts_mean > cnn_mcts_mean - 1.0 {
            println!("\n   ➖ GAT+MCTS matches CNN+Q-net");
        } else {
            println!("\n   📉 CNN+Q-net still better (pre-trained advantage)");
        }
    }
}

fn train_gat_networks(n_games: usize, epochs: usize, seed: u64) -> GATPolicyNet {
    // Generate training data
    let mut data = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);

    for _ in 0..n_games {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let best = find_greedy(&plateau, &tile, &avail) as i64;

            data.push(TrainSample { features, best_pos: best });

            plateau.tiles[best as usize] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
    }

    // Train policy network
    let vs_policy = nn::VarStore::new(Device::Cpu);
    let policy = GATPolicyNet::new(&vs_policy, 47, &[128, 128], 4, 0.1);
    let mut opt = nn::Adam::default().build(&vs_policy, 0.001).unwrap();

    let bs = 32;
    let nb = data.len() / bs;

    for epoch in 0..epochs {
        let mut loss_sum = 0.0;
        let mut idx: Vec<usize> = (0..data.len()).collect();
        idx.shuffle(&mut rng);

        for b in 0..nb {
            let bi = &idx[b*bs..(b+1)*bs];
            let feats: Vec<Tensor> = bi.iter().map(|&i| data[i].features.shallow_clone()).collect();
            let tgts: Vec<i64> = bi.iter().map(|&i| data[i].best_pos).collect();

            let bf = Tensor::stack(&feats, 0);
            let bt = Tensor::from_slice(&tgts);
            let logits = policy.forward(&bf, true);
            let loss = logits.cross_entropy_for_logits(&bt);
            opt.backward_step(&loss);
            loss_sum += f64::try_from(&loss).unwrap();
        }

        if (epoch + 1) % 20 == 0 {
            println!("   Epoch {}/{}: loss={:.4}", epoch + 1, epochs, loss_sum / nb as f64);
        }
    }

    policy
}

fn find_greedy(p: &Plateau, t: &Tile, a: &[usize]) -> usize {
    a.iter().copied().max_by_key(|&pos| {
        let mut test = p.clone();
        test.tiles[pos] = *t;
        result(&test)
    }).unwrap_or(a[0])
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

fn play_gat_policy(tiles: &[Tile], policy: &GATPolicyNet) -> i32 {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
        if avail.is_empty() { break; }

        let features = convert_plateau_for_gat_47ch(&plateau, tile, &deck, turn, 19);
        let logits = policy.forward(&features.unsqueeze(0), false);

        // Mask invalid positions
        let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
        for &pos in &avail {
            let _ = mask.i(pos as i64).fill_(0.0);
        }
        let best: i64 = (logits.squeeze_dim(0) + mask).argmax(0, false).try_into().unwrap();

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

        // Use GAT policy for pruning in early game
        let should_prune = avail.len() > top_k + 2 && turn < 10;

        let best_pos = if should_prune {
            // Get top-K positions from GAT policy network
            let features = convert_plateau_for_gat_47ch(&plateau, tile, &deck, turn, 19);
            let logits = policy.forward(&features.unsqueeze(0), false).squeeze_dim(0);

            // Get top-K from available positions
            let mut scored: Vec<(usize, f64)> = avail.iter()
                .map(|&pos| (pos, logits.double_value(&[pos as i64])))
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_positions: Vec<usize> = scored.iter().take(top_k).map(|(pos, _)| *pos).collect();

            // Run simulations on top positions only
            let sims_per_pos = num_sims / top_positions.len().max(1);
            let mut best = top_positions[0];
            let mut best_score = f64::NEG_INFINITY;

            for &pos in &top_positions {
                let mut temp_plateau = plateau.clone();
                temp_plateau.tiles[pos] = *tile;
                let temp_deck = replace_tile_in_deck(&deck, tile);

                let mut total = 0.0;
                for _ in 0..sims_per_pos {
                    total += simulate_random_game(&temp_plateau, &temp_deck.clone()) as f64;
                }
                let avg = total / sims_per_pos as f64;

                if avg > best_score {
                    best_score = avg;
                    best = pos;
                }
            }
            best
        } else {
            // Full MCTS for late game
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
