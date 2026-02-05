//! Optimized GAT AlphaZero Training with Q-net
//!
//! Full AlphaZero implementation for GAT:
//! - Policy network (GAT 47ch) for move priors
//! - Value network (GAT 47ch) for position evaluation
//! - Q-net (GAT 47ch) for action pruning
//! - PUCT-based MCTS with Dirichlet noise
//! - Curriculum learning with increasing difficulty
//!
//! Usage: cargo run --release --bin train_gat_alphazero_optimized -- --iterations 100

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
#[command(name = "train_gat_alphazero_optimized")]
struct Args {
    #[arg(long, default_value_t = 100)]
    iterations: usize,

    #[arg(long, default_value_t = 200)]
    games_per_iter: usize,

    /// MCTS simulations (increases with curriculum)
    #[arg(long, default_value_t = 100)]
    base_simulations: usize,

    #[arg(long, default_value_t = 20)]
    epochs: usize,

    #[arg(long, default_value_t = 100)]
    eval_games: usize,

    #[arg(long, default_value_t = 0.001)]
    lr: f64,

    /// PUCT exploration constant
    #[arg(long, default_value_t = 2.5)]
    c_puct: f64,

    /// Dirichlet noise alpha
    #[arg(long, default_value_t = 0.3)]
    dirichlet_alpha: f64,

    /// Dirichlet noise weight
    #[arg(long, default_value_t = 0.25)]
    dirichlet_weight: f64,

    /// Temperature for first N moves
    #[arg(long, default_value_t = 10)]
    temp_threshold: usize,

    /// Top-K for Q-net pruning
    #[arg(long, default_value_t = 8)]
    top_k: usize,

    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Load pre-trained weights
    #[arg(long)]
    load_policy: Option<String>,

    #[arg(long)]
    load_value: Option<String>,

    #[arg(long, default_value = "model_weights/gat_alphazero")]
    save_path: String,
}

struct TrainingSample {
    features: Tensor,
    mcts_policy: Tensor,
    value_target: f32,
}

/// Replay buffer for experience replay
struct ReplayBuffer {
    samples: Vec<TrainingSample>,
    capacity: usize,
}

impl ReplayBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn add(&mut self, samples: Vec<TrainingSample>) {
        for sample in samples {
            if self.samples.len() >= self.capacity {
                self.samples.remove(0);
            }
            self.samples.push(sample);
        }
    }

    fn sample(&self, batch_size: usize, rng: &mut StdRng) -> Vec<&TrainingSample> {
        let indices: Vec<usize> = (0..self.samples.len()).collect();
        indices
            .sample(rng, batch_size.min(self.samples.len()))
            .map(|&i| &self.samples[i])
            .collect()
    }

    fn len(&self) -> usize {
        self.samples.len()
    }
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Optimized GAT AlphaZero with Q-net Pruning               ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Config:");
    println!("  Iterations:    {}", args.iterations);
    println!("  Games/iter:    {}", args.games_per_iter);
    println!("  Base sims:     {}", args.base_simulations);
    println!("  C_PUCT:        {}", args.c_puct);
    println!("  Dirichlet α:   {}", args.dirichlet_alpha);
    println!("  Top-K prune:   {}", args.top_k);

    let device = Device::Cpu;

    // Initialize or load networks
    let mut vs_policy = nn::VarStore::new(device);
    let policy_net = GATPolicyNet::new(&vs_policy, 47, &[256, 256, 128], 8, 0.1);

    let mut vs_value = nn::VarStore::new(device);
    let value_net = GATValueNet::new(&vs_value, 47, &[256, 256, 128], 8, 0.1);

    // Q-net for pruning (same architecture as policy)
    let mut vs_qnet = nn::VarStore::new(device);
    let qnet = GATPolicyNet::new(&vs_qnet, 47, &[256, 256, 128], 8, 0.1);

    // Load pre-trained weights if available
    if let Some(ref path) = args.load_policy {
        if let Err(e) = vs_policy.load(path) {
            println!("Warning: Could not load policy weights: {}", e);
        } else {
            println!("✅ Loaded policy from {}", path);
        }
    }
    if let Some(ref path) = args.load_value {
        if let Err(e) = vs_value.load(path) {
            println!("Warning: Could not load value weights: {}", e);
        } else {
            println!("✅ Loaded value from {}", path);
        }
    }

    // Try loading previous best
    let _ = vs_policy.load(format!("{}_policy.pt", args.save_path));
    let _ = vs_value.load(format!("{}_value.pt", args.save_path));
    let _ = vs_qnet.load(format!("{}_qnet.pt", args.save_path));

    // Optimizers with weight decay
    let mut opt_policy = nn::AdamW::default().build(&vs_policy, args.lr).unwrap();
    let mut opt_value = nn::AdamW::default().build(&vs_value, args.lr).unwrap();
    let mut opt_qnet = nn::AdamW::default().build(&vs_qnet, args.lr).unwrap();

    // Replay buffer
    let mut replay_buffer = ReplayBuffer::new(100_000);

    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut best_score = 0.0f64;

    // Baselines
    println!("\n📊 Initial evaluation...");
    let greedy_baseline = eval_greedy(args.eval_games, args.seed);
    let random_baseline = eval_random(args.eval_games, args.seed);
    println!("   Greedy: {:.2} pts", greedy_baseline);
    println!("   Random: {:.2} pts", random_baseline);

    // Initial eval
    let init_score = eval_gat_mcts(
        &policy_net, &value_net, &qnet,
        50, args.base_simulations, args.c_puct, args.top_k,
        args.seed,
    );
    println!("   Initial GAT: {:.2} pts", init_score);
    best_score = init_score;

    // Training loop with curriculum
    for iteration in 0..args.iterations {
        let iter_start = Instant::now();

        // Curriculum: increase simulations over time
        let curriculum_factor = 1.0 + (iteration as f64 / args.iterations as f64) * 0.5;
        let num_sims = (args.base_simulations as f64 * curriculum_factor) as usize;

        // Decrease temperature over time
        let temperature = if iteration < args.iterations / 3 {
            1.0
        } else if iteration < 2 * args.iterations / 3 {
            0.5
        } else {
            0.25
        };

        println!("\n{}", "=".repeat(70));
        println!("Iteration {}/{} | sims={} | temp={:.2} | buffer={}",
                 iteration + 1, args.iterations, num_sims, temperature, replay_buffer.len());
        println!("{}", "=".repeat(70));

        // Self-play
        println!("  [1/4] Self-play ({} games)...", args.games_per_iter);
        let new_samples = self_play_games(
            &policy_net, &value_net, &qnet,
            args.games_per_iter, num_sims,
            args.c_puct, args.dirichlet_alpha, args.dirichlet_weight,
            temperature, args.temp_threshold, args.top_k,
            &mut rng,
        );
        let num_new = new_samples.len();
        replay_buffer.add(new_samples);
        println!("        +{} samples (total: {})", num_new, replay_buffer.len());

        // Train from replay buffer
        let batch_size = 64;
        let batches_per_epoch = replay_buffer.len() / batch_size;

        println!("  [2/4] Training policy...");
        let policy_loss = train_policy_from_buffer(
            &policy_net, &mut opt_policy, &replay_buffer,
            args.epochs, batch_size, &mut rng,
        );
        println!("        Loss: {:.4}", policy_loss);

        println!("  [3/4] Training value...");
        let value_loss = train_value_from_buffer(
            &value_net, &mut opt_value, &replay_buffer,
            args.epochs, batch_size, &mut rng,
        );
        println!("        Loss: {:.4}", value_loss);

        println!("  [4/4] Training Q-net...");
        let qnet_loss = train_qnet_from_buffer(
            &qnet, &mut opt_qnet, &replay_buffer,
            args.epochs, batch_size, &mut rng,
        );
        println!("        Loss: {:.4}", qnet_loss);

        // Evaluate
        let eval_score = eval_gat_mcts(
            &policy_net, &value_net, &qnet,
            args.eval_games, num_sims, args.c_puct, args.top_k,
            args.seed + iteration as u64 * 1000,
        );

        let elapsed = iter_start.elapsed().as_secs_f32();
        println!("\n  📈 Eval: {:.2} pts (vs greedy: {:+.2}, vs random: {:+.2})",
                 eval_score, eval_score - greedy_baseline, eval_score - random_baseline);
        println!("  ⏱️  Time: {:.1}s", elapsed);

        if eval_score > best_score {
            best_score = eval_score;
            println!("  💾 New best! Saving...");
            let _ = vs_policy.save(format!("{}_policy.pt", args.save_path));
            let _ = vs_value.save(format!("{}_value.pt", args.save_path));
            let _ = vs_qnet.save(format!("{}_qnet.pt", args.save_path));
        }

        // Periodic checkpoint
        if (iteration + 1) % 10 == 0 {
            let _ = vs_policy.save(format!("{}_policy_iter{}.pt", args.save_path, iteration + 1));
        }
    }

    // Final results
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                     TRAINING COMPLETE                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("\n  Best score: {:.2} pts", best_score);
    println!("  vs Greedy:  {:+.2} pts", best_score - greedy_baseline);
    println!("  vs Random:  {:+.2} pts", best_score - random_baseline);

    if best_score > greedy_baseline * 3.0 {
        println!("\n  🏆 GAT DOMINATES! (3x Greedy)");
    } else if best_score > greedy_baseline {
        println!("\n  🏆 GAT BEATS GREEDY!");
    }
}

fn self_play_games(
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    qnet: &GATPolicyNet,
    n_games: usize,
    n_sims: usize,
    c_puct: f64,
    dirichlet_alpha: f64,
    dirichlet_weight: f64,
    temperature: f64,
    temp_threshold: usize,
    top_k: usize,
    rng: &mut StdRng,
) -> Vec<TrainingSample> {
    let mut all_samples = Vec::new();

    for _ in 0..n_games {
        let samples = play_one_game(
            policy_net, value_net, qnet,
            n_sims, c_puct, dirichlet_alpha, dirichlet_weight,
            temperature, temp_threshold, top_k, rng,
        );
        all_samples.extend(samples);
    }

    all_samples
}

fn play_one_game(
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    qnet: &GATPolicyNet,
    n_sims: usize,
    c_puct: f64,
    dirichlet_alpha: f64,
    dirichlet_weight: f64,
    temperature: f64,
    temp_threshold: usize,
    top_k: usize,
    rng: &mut StdRng,
) -> Vec<TrainingSample> {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let mut game_history: Vec<(Tensor, Tensor)> = Vec::new();

    for turn in 0..19 {
        let tiles = get_available_tiles(&deck);
        if tiles.is_empty() { break; }
        let tile = *tiles.choose(rng).unwrap();

        let avail: Vec<usize> = (0..19)
            .filter(|&i| plateau.tiles[i] == Tile(0, 0, 0))
            .collect();
        if avail.is_empty() { break; }

        let features = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);

        // Use temperature for early moves, greedy for later
        let use_temp = turn < temp_threshold;
        let temp = if use_temp { temperature } else { 0.1 };

        let (mcts_policy, best_pos) = mcts_search_with_qnet(
            &plateau, &deck, &tile, &avail,
            policy_net, value_net, qnet,
            n_sims, c_puct, dirichlet_alpha, dirichlet_weight,
            temp, top_k, turn, rng,
        );

        game_history.push((features, mcts_policy));

        plateau.tiles[best_pos] = tile;
        deck = replace_tile_in_deck(&deck, &tile);
    }

    // Compute value target from final score
    let final_score = result(&plateau);
    let value_target = (final_score as f32 - 50.0) / 150.0;

    game_history
        .into_iter()
        .map(|(features, mcts_policy)| TrainingSample {
            features,
            mcts_policy,
            value_target,
        })
        .collect()
}

fn mcts_search_with_qnet(
    plateau: &Plateau,
    deck: &Deck,
    tile: &Tile,
    avail: &[usize],
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    qnet: &GATPolicyNet,
    n_sims: usize,
    c_puct: f64,
    dirichlet_alpha: f64,
    dirichlet_weight: f64,
    temperature: f64,
    top_k: usize,
    turn: usize,
    rng: &mut StdRng,
) -> (Tensor, usize) {
    // Q-net pruning for early game
    let search_positions = if avail.len() > top_k + 2 && turn < 12 {
        get_top_k_positions(qnet, plateau, tile, deck, avail, top_k, turn)
    } else {
        avail.to_vec()
    };

    let mut visit_counts = vec![0u32; 19];
    let mut total_values = vec![0.0f64; 19];
    let mut prior_probs = vec![0.0f64; 19];

    // Get prior from policy network
    let features = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19);
    let prior_logits = policy_net.forward(&features.unsqueeze(0), false).squeeze_dim(0);

    // Mask and softmax
    let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
    for &pos in &search_positions {
        let _ = mask.i(pos as i64).fill_(0.0);
    }
    let prior = (prior_logits + &mask).softmax(0, Kind::Float);

    for &pos in &search_positions {
        prior_probs[pos] = prior.double_value(&[pos as i64]);
    }

    // Add Dirichlet-like noise to root (simplified)
    if dirichlet_weight > 0.0 && search_positions.len() > 1 {
        // Approximate Dirichlet noise with Gamma samples
        let mut noise = Vec::with_capacity(search_positions.len());
        let mut noise_sum = 0.0;
        for _ in 0..search_positions.len() {
            // Gamma(alpha, 1) approximation using Box-Muller
            let u1: f64 = rng.random();
            let u2: f64 = rng.random();
            let n = (-u1.ln()).powf(dirichlet_alpha) * (2.0 * std::f64::consts::PI * u2).cos().abs();
            noise.push(n.max(0.01));
            noise_sum += n.max(0.01);
        }
        // Normalize to Dirichlet
        for (i, &pos) in search_positions.iter().enumerate() {
            let noise_val = noise[i] / noise_sum;
            prior_probs[pos] = (1.0 - dirichlet_weight) * prior_probs[pos]
                             + dirichlet_weight * noise_val;
        }
    }

    // MCTS simulations
    for _ in 0..n_sims {
        let total_visits: u32 = visit_counts.iter().sum();

        // PUCT selection
        let best_pos = search_positions.iter().copied().max_by(|&a, &b| {
            let q_a = if visit_counts[a] > 0 {
                total_values[a] / visit_counts[a] as f64
            } else {
                0.5 // Optimistic initialization
            };
            let q_b = if visit_counts[b] > 0 {
                total_values[b] / visit_counts[b] as f64
            } else {
                0.5
            };

            let u_a = q_a + c_puct * prior_probs[a] * ((total_visits as f64).sqrt() / (1.0 + visit_counts[a] as f64));
            let u_b = q_b + c_puct * prior_probs[b] * ((total_visits as f64).sqrt() / (1.0 + visit_counts[b] as f64));

            u_a.partial_cmp(&u_b).unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap_or(search_positions[0]);

        // Evaluate
        let mut sim_plateau = plateau.clone();
        sim_plateau.tiles[best_pos] = *tile;
        let sim_deck = replace_tile_in_deck(deck, tile);

        let value = evaluate_with_network(&sim_plateau, &sim_deck, value_net, turn + 1, rng);

        visit_counts[best_pos] += 1;
        total_values[best_pos] += value;
    }

    // Create policy from visit counts
    let visits_f: Vec<f32> = visit_counts.iter().map(|&v| v as f32).collect();
    let visits_tensor = Tensor::from_slice(&visits_f);

    let policy = if temperature > 0.01 {
        (visits_tensor / temperature).softmax(0, Kind::Float)
    } else {
        let max_visits = *visit_counts.iter().max().unwrap_or(&1);
        Tensor::from_slice(&visit_counts.iter()
            .map(|&v| if v == max_visits { 1.0f32 } else { 0.0 })
            .collect::<Vec<_>>())
    };

    // Select action
    let best_pos = if temperature > 0.3 {
        // Sample from policy
        let probs: Vec<f64> = (0..19).map(|i| policy.double_value(&[i])).collect();
        let total: f64 = search_positions.iter().map(|&p| probs[p]).sum();
        let mut r = rng.random::<f64>() * total;
        let mut selected = search_positions[0];
        for &pos in &search_positions {
            r -= probs[pos];
            if r <= 0.0 {
                selected = pos;
                break;
            }
        }
        selected
    } else {
        // Greedy
        *search_positions.iter().max_by_key(|&&p| visit_counts[p]).unwrap_or(&search_positions[0])
    };

    (policy, best_pos)
}

fn get_top_k_positions(
    qnet: &GATPolicyNet,
    plateau: &Plateau,
    tile: &Tile,
    deck: &Deck,
    avail: &[usize],
    top_k: usize,
    turn: usize,
) -> Vec<usize> {
    let features = convert_plateau_for_gat_47ch(plateau, tile, deck, turn, 19);
    let logits = qnet.forward(&features.unsqueeze(0), false).squeeze_dim(0);

    let mut scored: Vec<(usize, f64)> = avail.iter()
        .map(|&pos| (pos, logits.double_value(&[pos as i64])))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored.iter().take(top_k).map(|(pos, _)| *pos).collect()
}

fn evaluate_with_network(
    plateau: &Plateau,
    deck: &Deck,
    value_net: &GATValueNet,
    turn: usize,
    rng: &mut StdRng,
) -> f64 {
    // Late game: use rollout
    if turn >= 17 {
        return random_rollout(plateau, deck, rng) as f64 / 200.0;
    }

    let tiles = get_available_tiles(deck);
    if tiles.is_empty() {
        return result(plateau) as f64 / 200.0;
    }
    let tile = tiles[0];

    let features = convert_plateau_for_gat_47ch(plateau, &tile, deck, turn, 19);
    let value: f64 = value_net.forward(&features.unsqueeze(0), false)
        .double_value(&[0, 0]);

    // Clamp to valid range
    value.clamp(-1.0, 1.0) * 0.5 + 0.5
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

fn train_policy_from_buffer(
    net: &GATPolicyNet,
    opt: &mut nn::Optimizer,
    buffer: &ReplayBuffer,
    epochs: usize,
    batch_size: usize,
    rng: &mut StdRng,
) -> f64 {
    if buffer.len() < batch_size { return 0.0; }

    let mut total_loss = 0.0;
    let mut count = 0;

    for _ in 0..epochs {
        let batch = buffer.sample(batch_size, rng);
        if batch.is_empty() { continue; }

        let features: Vec<Tensor> = batch.iter().map(|s| s.features.shallow_clone()).collect();
        let targets: Vec<Tensor> = batch.iter().map(|s| s.mcts_policy.shallow_clone()).collect();

        let features_t = Tensor::stack(&features, 0);
        let targets_t = Tensor::stack(&targets, 0);

        let logits = net.forward(&features_t, true);
        let log_probs = logits.log_softmax(-1, Kind::Float);

        // KL divergence loss
        let loss = -(targets_t * log_probs).sum(Kind::Float) / batch.len() as f64;

        opt.backward_step(&loss);
        total_loss += f64::try_from(&loss).unwrap();
        count += 1;
    }

    if count > 0 { total_loss / count as f64 } else { 0.0 }
}

fn train_value_from_buffer(
    net: &GATValueNet,
    opt: &mut nn::Optimizer,
    buffer: &ReplayBuffer,
    epochs: usize,
    batch_size: usize,
    rng: &mut StdRng,
) -> f64 {
    if buffer.len() < batch_size { return 0.0; }

    let mut total_loss = 0.0;
    let mut count = 0;

    for _ in 0..epochs {
        let batch = buffer.sample(batch_size, rng);
        if batch.is_empty() { continue; }

        let features: Vec<Tensor> = batch.iter().map(|s| s.features.shallow_clone()).collect();
        let targets: Vec<f32> = batch.iter().map(|s| s.value_target).collect();

        let features_t = Tensor::stack(&features, 0);
        let targets_t = Tensor::from_slice(&targets).view([batch.len() as i64, 1]);

        let predictions = net.forward(&features_t, true);
        let loss = (predictions - targets_t).pow_tensor_scalar(2).mean(Kind::Float);

        opt.backward_step(&loss);
        total_loss += f64::try_from(&loss).unwrap();
        count += 1;
    }

    if count > 0 { total_loss / count as f64 } else { 0.0 }
}

fn train_qnet_from_buffer(
    net: &GATPolicyNet,
    opt: &mut nn::Optimizer,
    buffer: &ReplayBuffer,
    epochs: usize,
    batch_size: usize,
    rng: &mut StdRng,
) -> f64 {
    // Q-net learns to predict MCTS policy (same as policy net but separate network)
    train_policy_from_buffer(net, opt, buffer, epochs, batch_size, rng)
}

fn eval_gat_mcts(
    policy_net: &GATPolicyNet,
    value_net: &GATValueNet,
    qnet: &GATPolicyNet,
    n_games: usize,
    n_sims: usize,
    c_puct: f64,
    top_k: usize,
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

            let (_, best_pos) = mcts_search_with_qnet(
                &plateau, &deck, &tile, &avail,
                policy_net, value_net, qnet,
                n_sims, c_puct, 0.0, 0.0, // No noise for eval
                0.1, top_k, turn, &mut rng,
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
