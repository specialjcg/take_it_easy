//! Self-Play Reinforcement Learning for GAT
//!
//! Trains GAT using AlphaZero-style self-play:
//! 1. Play games using GAT + MCTS
//! 2. Collect (state, MCTS policy, final_score) tuples
//! 3. Train GAT policy on MCTS policy distribution
//! 4. Train GAT value on final game score
//! 5. Repeat
//!
//! Usage: cargo run --release --bin train_gat_selfplay -- --iterations 50 --games-per-iter 100

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
use take_it_easy::neural::gat::{GATPolicyNet, GATValueNet};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "train_gat_selfplay")]
struct Args {
    /// Number of training iterations
    #[arg(long, default_value_t = 50)]
    iterations: usize,

    /// Games per iteration for self-play
    #[arg(long, default_value_t = 100)]
    games_per_iter: usize,

    /// MCTS simulations per move
    #[arg(long, default_value_t = 50)]
    simulations: usize,

    /// Training epochs per iteration
    #[arg(long, default_value_t = 10)]
    epochs: usize,

    /// Evaluation games
    #[arg(long, default_value_t = 50)]
    eval_games: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// Temperature for MCTS policy
    #[arg(long, default_value_t = 1.0)]
    temperature: f64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Save model path
    #[arg(long, default_value = "model_weights/gat_selfplay")]
    save_path: String,

    /// Load pre-trained policy weights
    #[arg(long)]
    load_policy: Option<String>,

    /// Load pre-trained value weights
    #[arg(long)]
    load_value: Option<String>,
}

/// Training sample from self-play
struct SelfPlaySample {
    features: Tensor,           // [19, 47] node features
    mcts_policy: Tensor,        // [19] MCTS visit distribution
    final_score: f32,           // Normalized final score
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         GAT Self-Play Reinforcement Learning                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Iterations: {}", args.iterations);
    println!("  Games/iter: {}", args.games_per_iter);
    println!("  MCTS sims:  {}", args.simulations);
    println!("  Epochs:     {}", args.epochs);
    println!("  LR:         {}", args.lr);

    let device = Device::Cpu;

    // Initialize networks
    let mut vs_policy = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs_policy, 47, &[128, 128], 4, 0.1);
    let mut opt_policy = nn::Adam::default().build(&vs_policy, args.lr).unwrap();

    let mut vs_value = nn::VarStore::new(device);
    let value_net = GATValueNet::new(&vs_value, 47, &[128, 128], 4, 0.1);
    let mut opt_value = nn::Adam::default().build(&vs_value, args.lr).unwrap();

    // Load pre-trained weights if provided
    if let Some(ref path) = args.load_policy {
        match vs_policy.load(path) {
            Ok(_) => println!("  ✅ Loaded policy weights from {}", path),
            Err(e) => println!("  ⚠️ Could not load policy: {}", e),
        }
    }
    if let Some(ref path) = args.load_value {
        match vs_value.load(path) {
            Ok(_) => println!("  ✅ Loaded value weights from {}", path),
            Err(e) => println!("  ⚠️ Could not load value: {}", e),
        }
    }

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut best_score = 0.0f64;

    // Baseline evaluation
    println!("\n📊 Initial evaluation...");
    let greedy_baseline = eval_greedy(args.eval_games, args.seed);
    let random_baseline = eval_random(args.eval_games, args.seed);
    println!("   Greedy baseline: {:.2} pts", greedy_baseline);
    println!("   Random baseline: {:.2} pts", random_baseline);

    // Training loop
    for iteration in 0..args.iterations {
        let iter_start = Instant::now();
        println!("\n{}", "=".repeat(60));
        println!("Iteration {}/{}", iteration + 1, args.iterations);
        println!("{}", "=".repeat(60));

        // 1. Self-play: collect training data
        println!("  [1/3] Self-play ({} games)...", args.games_per_iter);
        let samples = self_play_games(
            &policy_net,
            &value_net,
            args.games_per_iter,
            args.simulations,
            args.temperature,
            &mut rng,
        );
        println!("        Collected {} samples", samples.len());

        // 2. Train policy network
        println!("  [2/3] Training policy...");
        let policy_loss = train_policy(&policy_net, &mut opt_policy, &samples, args.epochs);
        println!("        Policy loss: {:.4}", policy_loss);

        // 3. Train value network
        println!("  [3/3] Training value...");
        let value_loss = train_value(&value_net, &mut opt_value, &samples, args.epochs);
        println!("        Value loss: {:.4}", value_loss);

        // Evaluate
        let eval_score = eval_gat_mcts(
            &policy_net,
            &value_net,
            args.eval_games,
            args.simulations,
            args.seed + iteration as u64 * 1000,
        );

        let elapsed = iter_start.elapsed().as_secs_f32();
        println!("\n  📈 Eval: {:.2} pts (vs greedy: {:+.2}, vs random: {:+.2})",
                 eval_score, eval_score - greedy_baseline, eval_score - random_baseline);
        println!("  ⏱️  Time: {:.1}s", elapsed);

        // Save best model
        if eval_score > best_score {
            best_score = eval_score;
            println!("  💾 New best! Saving model...");
            let _ = vs_policy.save(format!("{}_policy.pt", args.save_path));
            let _ = vs_value.save(format!("{}_value.pt", args.save_path));
        }
    }

    // Final results
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                     TRAINING COMPLETE                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\n  Best score: {:.2} pts", best_score);
    println!("  vs Greedy:  {:+.2} pts", best_score - greedy_baseline);
    println!("  vs Random:  {:+.2} pts", best_score - random_baseline);

    if best_score > greedy_baseline {
        println!("\n  🏆 GAT BEATS GREEDY!");
    }
}

/// Play self-play games and collect training samples
fn self_play_games(
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    n_games: usize,
    n_sims: usize,
    temperature: f64,
    rng: &mut StdRng,
) -> Vec<SelfPlaySample> {
    let mut samples = Vec::new();

    for _ in 0..n_games {
        let game_samples = play_one_game(policy_net, value_net, n_sims, temperature, rng);
        samples.extend(game_samples);
    }

    samples
}

/// Play one game and return samples
fn play_one_game(
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    n_sims: usize,
    temperature: f64,
    rng: &mut StdRng,
) -> Vec<SelfPlaySample> {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut game_states: Vec<(Tensor, Tensor)> = Vec::new(); // (features, mcts_policy)

    for turn in 0..19 {
        let tiles = get_available_tiles(&deck);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(rng).unwrap();

        let avail: Vec<usize> = (0..19)
            .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
            .collect();
        if avail.is_empty() { break; }

        // Get features
        let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);

        // Run MCTS to get policy distribution
        let (mcts_policy, best_pos) = mcts_search(
            &plateau,
            &deck,
            &tile,
            &avail,
            policy_net,
            value_net,
            n_sims,
            temperature,
            rng,
        );

        game_states.push((features, mcts_policy));

        // Make move
        plateau.tiles[best_pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    // Get final score and normalize
    let final_score = result(&plateau);
    let normalized_score = (final_score as f32 - 50.0) / 150.0; // Normalize to roughly [-0.3, 1.0]

    // Create samples with final score
    game_states
        .into_iter()
        .map(|(features, mcts_policy)| SelfPlaySample {
            features,
            mcts_policy,
            final_score: normalized_score,
        })
        .collect()
}

/// MCTS search with neural network guidance
fn mcts_search(
    plateau: &Plateau,
    deck: &Deck,
    tile: &Tile,
    avail: &[usize],
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    n_sims: usize,
    temperature: f64,
    rng: &mut StdRng,
) -> (Tensor, usize) {
    // Visit counts for each position
    let mut visit_counts = vec![0u32; 19];
    let mut total_values = vec![0.0f64; 19];

    // Get prior policy from network
    let features = convert_plateau_for_gat_47ch(plateau, tile, deck,
        plateau.tiles.iter().filter(|t| **t != Tile(0, 0, 0)).count(), 19);
    let prior_logits = policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0);

    // Mask unavailable positions and get softmax
    let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
    for &pos in avail {
        let _ = mask.i(pos as i64).fill_(0.0);
    }
    let prior = (prior_logits + &mask).softmax(0, Kind::Float);

    // Run simulations
    for _ in 0..n_sims {
        // UCB selection
        let total_visits: u32 = visit_counts.iter().sum();
        let c_puct = 1.5;

        let best_pos = avail.iter().copied().max_by(|&a, &b| {
            let q_a = if visit_counts[a] > 0 {
                total_values[a] / visit_counts[a] as f64
            } else {
                0.0
            };
            let q_b = if visit_counts[b] > 0 {
                total_values[b] / visit_counts[b] as f64
            } else {
                0.0
            };

            let p_a: f64 = prior.double_value(&[a as i64]);
            let p_b: f64 = prior.double_value(&[b as i64]);

            let u_a = q_a + c_puct * p_a * ((total_visits as f64).sqrt() / (1.0 + visit_counts[a] as f64));
            let u_b = q_b + c_puct * p_b * ((total_visits as f64).sqrt() / (1.0 + visit_counts[b] as f64));

            u_a.partial_cmp(&u_b).unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap_or(avail[0]);

        // Simulate from this position
        let mut sim_plateau = plateau.clone();
        sim_plateau.tiles[best_pos] = *tile;
        let sim_deck = replace_tile_in_deck(deck, tile);

        // Use value network for evaluation + random rollout
        let value = evaluate_position(&sim_plateau, &sim_deck, value_net, rng);

        // Update
        visit_counts[best_pos] += 1;
        total_values[best_pos] += value;
    }

    // Create policy from visit counts with temperature
    let visits_tensor = Tensor::from_slice(&visit_counts.iter().map(|&v| v as f32).collect::<Vec<_>>());
    let policy = if temperature > 0.01 {
        (visits_tensor / temperature).softmax(0, Kind::Float)
    } else {
        // Greedy
        let max_visits = *visit_counts.iter().max().unwrap_or(&1);
        Tensor::from_slice(&visit_counts.iter().map(|&v| if v == max_visits { 1.0f32 } else { 0.0 }).collect::<Vec<_>>())
    };

    // Select action (sample from policy for exploration)
    let best_pos = if temperature > 0.5 {
        // Sample proportionally
        let probs: Vec<f64> = (0..19).map(|i| policy.double_value(&[i])).collect();
        let total: f64 = avail.iter().map(|&p| probs[p]).sum();
        let mut r = rng.random::<f64>() * total;
        let mut selected = avail[0];
        for &pos in avail {
            r -= probs[pos];
            if r <= 0.0 {
                selected = pos;
                break;
            }
        }
        selected
    } else {
        // Greedy
        *avail.iter().max_by_key(|&&p| visit_counts[p]).unwrap_or(&avail[0])
    };

    (policy, best_pos)
}

/// Evaluate position using value net + rollout
fn evaluate_position(
    plateau: &Plateau,
    deck: &Deck,
    value_net: &GATValueNet,
    rng: &mut StdRng,
) -> f64 {
    // Mix of value network prediction and random rollout
    let turn = plateau.tiles.iter().filter(|t| **t != Tile(0, 0, 0)).count();

    if turn >= 15 {
        // Late game: just use rollout
        return random_rollout(plateau, deck, rng) as f64 / 200.0;
    }

    // Get any available tile for feature computation
    let tiles = get_available_tiles(deck);
    if tiles.is_empty() {
        return result(plateau) as f64 / 200.0;
    }
    let tile = tiles[0];

    let features = convert_plateau_for_gat_47ch(plateau, &tile, deck, turn, 19);
    let value_pred: f64 = value_net.forward(&features.unsqueeze(0), false)
        .double_value(&[0, 0]);

    // Mix value network with rollout (more rollout early, more network late)
    let network_weight = (turn as f64) / 19.0;
    let rollout_score = random_rollout(plateau, deck, rng) as f64 / 200.0;

    network_weight * value_pred + (1.0 - network_weight) * rollout_score
}

/// Random rollout to end of game
fn random_rollout(plateau: &Plateau, deck: &Deck, rng: &mut StdRng) -> i32 {
    let mut plateau = plateau.clone();
    let mut deck = deck.clone();

    loop {
        let tiles = get_available_tiles(&deck);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(rng).unwrap();

        let avail: Vec<usize> = (0..19)
            .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
            .collect();
        if avail.is_empty() { break; }

        let pos = *avail.choose(rng).unwrap();
        plateau.tiles[pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    result(&plateau)
}

/// Train policy network on MCTS policy distributions
fn train_policy(
    net: &GATPolicyNet,
    opt: &mut nn::Optimizer,
    samples: &[SelfPlaySample],
    epochs: usize,
) -> f64 {
    let bs = 32;
    let nb = samples.len() / bs;
    if nb == 0 { return 0.0; }

    let mut total_loss = 0.0;

    for _ in 0..epochs {
        for b in 0..nb {
            let batch_features: Vec<Tensor> = samples[b*bs..(b+1)*bs]
                .iter()
                .map(|s| s.features.shallow_clone())
                .collect();
            let batch_targets: Vec<Tensor> = samples[b*bs..(b+1)*bs]
                .iter()
                .map(|s| s.mcts_policy.shallow_clone())
                .collect();

            let features = Tensor::stack(&batch_features, 0);
            let targets = Tensor::stack(&batch_targets, 0);

            let logits = net.forward(&features, true);

            // Cross-entropy with soft targets (KL divergence)
            let log_probs = logits.log_softmax(-1, Kind::Float);
            let loss = -(targets * log_probs).sum(Kind::Float) / bs as f64;

            opt.backward_step(&loss);
            total_loss += f64::try_from(&loss).unwrap();
        }
    }

    total_loss / (epochs * nb) as f64
}

/// Train value network on final scores
fn train_value(
    net: &GATValueNet,
    opt: &mut nn::Optimizer,
    samples: &[SelfPlaySample],
    epochs: usize,
) -> f64 {
    let bs = 32;
    let nb = samples.len() / bs;
    if nb == 0 { return 0.0; }

    let mut total_loss = 0.0;

    for _ in 0..epochs {
        for b in 0..nb {
            let batch_features: Vec<Tensor> = samples[b*bs..(b+1)*bs]
                .iter()
                .map(|s| s.features.shallow_clone())
                .collect();
            let batch_scores: Vec<f32> = samples[b*bs..(b+1)*bs]
                .iter()
                .map(|s| s.final_score)
                .collect();

            let features = Tensor::stack(&batch_features, 0);
            let targets = Tensor::from_slice(&batch_scores).view([bs as i64, 1]);

            let predictions = net.forward(&features, true);

            // MSE loss
            let loss = (predictions - targets).pow_tensor_scalar(2).mean(Kind::Float);

            opt.backward_step(&loss);
            total_loss += f64::try_from(&loss).unwrap();
        }
    }

    total_loss / (epochs * nb) as f64
}

/// Evaluate GAT + MCTS
fn eval_gat_mcts(
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    n_games: usize,
    n_sims: usize,
    seed: u64,
) -> f64 {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut total = 0;

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();

            let avail: Vec<usize> = (0..19)
                .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
                .collect();
            if avail.is_empty() { break; }

            // Use MCTS with low temperature (greedy)
            let (_, best_pos) = mcts_search(
                &plateau, &deck, &tile, &avail,
                policy_net, value_net,
                n_sims, 0.1, &mut rng,
            );

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total += result(&plateau);
    }

    total as f64 / n_games as f64
}

/// Evaluate greedy baseline
fn eval_greedy(n_games: usize, seed: u64) -> f64 {
    let mut rng = StdRng::seed_from_u64(seed + 30000);
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

            // Greedy selection
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
