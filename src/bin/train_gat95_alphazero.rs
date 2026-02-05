//! AlphaZero-style training for GAT 95ch with Q-net
//!
//! Uses extended 95-channel features for better hexagonal understanding.
//! Trains both policy and value networks with self-play.
//!
//! Usage: cargo run --release --bin train_gat95_alphazero -- --iterations 30

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
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_extended;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "train_gat95_alphazero")]
struct Args {
    #[arg(long, default_value_t = 30)]
    iterations: usize,

    #[arg(long, default_value_t = 100)]
    games_per_iter: usize,

    #[arg(long, default_value_t = 50)]
    simulations: usize,

    #[arg(long, default_value_t = 10)]
    epochs: usize,

    #[arg(long, default_value_t = 50)]
    eval_games: usize,

    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    #[arg(long, default_value_t = 1.0)]
    temperature: f64,

    #[arg(long, default_value_t = 42)]
    seed: u64,

    #[arg(long, default_value = "model_weights/gat95_alphazero")]
    save_path: String,
}

struct SelfPlaySample {
    features: Tensor,
    mcts_policy: Tensor,
    final_score: f32,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║      GAT 95ch AlphaZero-Style Self-Play Training             ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Channels:   95 (extended features)");
    println!("  Iterations: {}", args.iterations);
    println!("  Games/iter: {}", args.games_per_iter);
    println!("  MCTS sims:  {}", args.simulations);

    let device = Device::Cpu;

    // Initialize networks with 95 channels
    let vs_policy = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs_policy, 95, &[128, 128, 128], 4, 0.1);
    let mut opt_policy = nn::Adam::default().build(&vs_policy, args.lr).unwrap();

    let vs_value = nn::VarStore::new(device);
    let value_net = GATValueNet::new(&vs_value, 95, &[128, 128, 128], 4, 0.1);
    let mut opt_value = nn::Adam::default().build(&vs_value, args.lr).unwrap();

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut best_score = 0.0f64;

    // Baselines
    println!("\n📊 Initial evaluation...");
    let greedy_baseline = eval_greedy(args.eval_games, args.seed);
    let random_baseline = eval_random(args.eval_games, args.seed);
    println!("   Greedy baseline: {:.2} pts", greedy_baseline);
    println!("   Random baseline: {:.2} pts", random_baseline);

    // Training loop
    for iteration in 0..args.iterations {
        let iter_start = Instant::now();
        println!("\n{}", "=".repeat(60));
        println!("Iteration {}/{} (GAT 95ch)", iteration + 1, args.iterations);
        println!("{}", "=".repeat(60));

        // Self-play
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

        // Train policy
        println!("  [2/3] Training policy...");
        let policy_loss = train_policy(&policy_net, &mut opt_policy, &samples, args.epochs);
        println!("        Policy loss: {:.4}", policy_loss);

        // Train value
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

        if eval_score > best_score {
            best_score = eval_score;
            println!("  💾 New best! Saving model...");
            let _ = vs_policy.save(format!("{}_policy.pt", args.save_path));
            let _ = vs_value.save(format!("{}_value.pt", args.save_path));
        }
    }

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                     TRAINING COMPLETE                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\n  Best score (GAT 95ch): {:.2} pts", best_score);
    println!("  vs Greedy:  {:+.2} pts", best_score - greedy_baseline);
    println!("  vs Random:  {:+.2} pts", best_score - random_baseline);

    if best_score > greedy_baseline {
        println!("\n  🏆 GAT 95ch BEATS GREEDY!");
    }
}

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

fn play_one_game(
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    n_sims: usize,
    temperature: f64,
    rng: &mut StdRng,
) -> Vec<SelfPlaySample> {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut game_states: Vec<(Tensor, Tensor)> = Vec::new();

    for turn in 0..19 {
        let tiles = get_available_tiles(&deck);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(rng).unwrap();

        let avail: Vec<usize> = (0..19)
            .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
            .collect();
        if avail.is_empty() { break; }

        // Get 95ch features
        let features = convert_plateau_for_gat_extended(&plateau, &tile, &deck, turn, 19);

        // MCTS search
        let (mcts_policy, best_pos) = mcts_search(
            &plateau, &deck, &tile, &avail,
            policy_net, value_net,
            n_sims, temperature, rng, turn,
        );

        game_states.push((features, mcts_policy));

        plateau.tiles[best_pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    let final_score = result(&plateau);
    let normalized_score = (final_score as f32 - 50.0) / 150.0;

    game_states
        .into_iter()
        .map(|(features, mcts_policy)| SelfPlaySample {
            features,
            mcts_policy,
            final_score: normalized_score,
        })
        .collect()
}

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
    turn: usize,
) -> (Tensor, usize) {
    let mut visit_counts = vec![0u32; 19];
    let mut total_values = vec![0.0f64; 19];

    // Get prior from policy network (95ch)
    let features = convert_plateau_for_gat_extended(plateau, tile, deck, turn, 19);
    let prior_logits = policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0);

    let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
    for &pos in avail {
        let _ = mask.i(pos as i64).fill_(0.0);
    }
    let prior = (prior_logits + &mask).softmax(0, Kind::Float);

    // MCTS simulations
    for _ in 0..n_sims {
        let total_visits: u32 = visit_counts.iter().sum();
        let c_puct = 2.0; // Higher exploration for 95ch

        // UCB selection
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

        // Evaluate position
        let mut sim_plateau = plateau.clone();
        sim_plateau.tiles[best_pos] = *tile;
        let sim_deck = replace_tile_in_deck(deck, tile);

        let value = evaluate_position(&sim_plateau, &sim_deck, value_net, rng, turn + 1);

        visit_counts[best_pos] += 1;
        total_values[best_pos] += value;
    }

    // Create policy from visits
    let visits_tensor = Tensor::from_slice(&visit_counts.iter().map(|&v| v as f32).collect::<Vec<_>>());
    let policy = if temperature > 0.01 {
        (visits_tensor / temperature).softmax(0, Kind::Float)
    } else {
        let max_visits = *visit_counts.iter().max().unwrap_or(&1);
        Tensor::from_slice(&visit_counts.iter().map(|&v| if v == max_visits { 1.0f32 } else { 0.0 }).collect::<Vec<_>>())
    };

    // Select action
    let best_pos = if temperature > 0.5 {
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
        *avail.iter().max_by_key(|&&p| visit_counts[p]).unwrap_or(&avail[0])
    };

    (policy, best_pos)
}

fn evaluate_position(
    plateau: &Plateau,
    deck: &Deck,
    value_net: &GATValueNet,
    rng: &mut StdRng,
    turn: usize,
) -> f64 {
    if turn >= 16 {
        return random_rollout(plateau, deck, rng) as f64 / 200.0;
    }

    let tiles = get_available_tiles(deck);
    if tiles.is_empty() {
        return result(plateau) as f64 / 200.0;
    }
    let tile = tiles[0];

    // Use 95ch features for value estimation
    let features = convert_plateau_for_gat_extended(plateau, &tile, deck, turn, 19);
    let value_pred: f64 = value_net.forward(&features.unsqueeze(0), false)
        .double_value(&[0, 0]);

    // Mix with rollout
    let network_weight = 0.7; // More trust in 95ch value net
    let rollout_score = random_rollout(plateau, deck, rng) as f64 / 200.0;

    network_weight * value_pred + (1.0 - network_weight) * rollout_score
}

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
            let log_probs = logits.log_softmax(-1, Kind::Float);
            let loss = -(targets * log_probs).sum(Kind::Float) / bs as f64;

            opt.backward_step(&loss);
            total_loss += f64::try_from(&loss).unwrap();
        }
    }

    total_loss / (epochs * nb) as f64
}

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
            let loss = (predictions - targets).pow_tensor_scalar(2).mean(Kind::Float);

            opt.backward_step(&loss);
            total_loss += f64::try_from(&loss).unwrap();
        }
    }

    total_loss / (epochs * nb) as f64
}

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

            let (_, best_pos) = mcts_search(
                &plateau, &deck, &tile, &avail,
                policy_net, value_net,
                n_sims, 0.1, &mut rng, turn,
            );

            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        total += result(&plateau);
    }

    total as f64 / n_games as f64
}

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
