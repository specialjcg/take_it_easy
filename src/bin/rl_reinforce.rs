//! REINFORCE with Baseline — RL Training for Graph Transformer
//!
//! Simpler alternative to PPO: vanilla policy gradient with a learned
//! value function as variance-reducing baseline.
//! Single pass per batch (no clipping, no multiple epochs).
//!
//! Usage: cargo run --release --bin rl_reinforce -- --iterations 100

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::path::Path;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::neural::device_util::parse_device;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::{GraphTransformerPolicyNet, GraphTransformerValueNet};
use take_it_easy::neural::model_io::{load_varstore, save_varstore};
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "rl_reinforce")]
struct Args {
    /// Number of training iterations
    #[arg(long, default_value_t = 200)]
    iterations: usize,

    /// Trajectories (games) per iteration
    #[arg(long, default_value_t = 500)]
    games_per_iter: usize,

    /// Evaluation games per iteration
    #[arg(long, default_value_t = 200)]
    eval_games: usize,

    /// Minibatch size (single epoch per iteration)
    #[arg(long, default_value_t = 128)]
    batch_size: usize,

    /// Policy learning rate
    #[arg(long, default_value_t = 0.00003)]
    lr_policy: f64,

    /// Value learning rate
    #[arg(long, default_value_t = 0.001)]
    lr_value: f64,

    /// Discount factor
    #[arg(long, default_value_t = 0.99)]
    gamma: f64,

    /// GAE lambda (1.0 = Monte-Carlo returns)
    #[arg(long, default_value_t = 0.95)]
    gae_lambda: f64,

    /// Terminal reward coefficient (mixed with dense line-completion rewards)
    #[arg(long, default_value_t = 0.0)]
    terminal_coeff: f64,

    /// Entropy bonus coefficient
    #[arg(long, default_value_t = 0.02)]
    entropy_coeff: f64,

    /// Value loss coefficient
    #[arg(long, default_value_t = 0.5)]
    value_coeff: f64,

    /// Early stopping patience
    #[arg(long, default_value_t = 20)]
    patience: usize,

    /// Path to initial policy weights
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    load_path: String,

    /// Path to save best policy
    #[arg(long, default_value = "model_weights/gt_reinforce_best.safetensors")]
    save_path: String,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Device: "cpu", "cuda", "cuda:0"
    #[arg(long, default_value = "cpu")]
    device: String,
}

// ── Trajectory data ────────────────────────────────────────────────

struct Step {
    features: Tensor, // [19, 47]
    mask: Tensor,     // [19]
    action: i64,
    value: f64,
    reward: f64, // dense: line-completion reward for this step
}

struct Trajectory {
    steps: Vec<Step>,
    final_score: f64, // raw game score (for logging)
}

// ── Dense reward: line-completion scoring ─────────────────────────

/// Returns immediate reward when placing a tile at `pos` completes one or more lines.
/// Each completed matching line yields value × multiplier / 100.0.
fn line_completion_reward(plateau: &take_it_easy::game::plateau::Plateau, pos: usize) -> f64 {
    // 15 lines: (positions, multiplier, selector: 0=t.0, 1=t.1, 2=t.2)
    const LINES: [(&[usize], i32, u8); 15] = [
        (&[0, 1, 2], 3, 0),
        (&[3, 4, 5, 6], 4, 0),
        (&[7, 8, 9, 10, 11], 5, 0),
        (&[12, 13, 14, 15], 4, 0),
        (&[16, 17, 18], 3, 0),
        (&[0, 3, 7], 3, 1),
        (&[1, 4, 8, 12], 4, 1),
        (&[2, 5, 9, 13, 16], 5, 1),
        (&[6, 10, 14, 17], 4, 1),
        (&[11, 15, 18], 3, 1),
        (&[7, 12, 16], 3, 2),
        (&[3, 8, 13, 17], 4, 2),
        (&[0, 4, 9, 14, 18], 5, 2),
        (&[1, 5, 10, 15], 4, 2),
        (&[2, 6, 11], 3, 2),
    ];

    #[inline]
    fn tile_val(tile: &Tile, axis: u8) -> i32 {
        match axis {
            0 => tile.0,
            1 => tile.1,
            _ => tile.2,
        }
    }

    let mut reward = 0.0;
    for &(indices, multiplier, axis) in &LINES {
        if !indices.contains(&pos) {
            continue;
        }
        // Check all positions filled (Tile != (0,0,0))
        let all_filled = indices
            .iter()
            .all(|&i| plateau.tiles[i] != Tile(0, 0, 0));
        if !all_filled {
            continue;
        }
        // Check all values match for this axis
        let first = tile_val(&plateau.tiles[indices[0]], axis);
        let all_match = indices.iter().all(|&i| tile_val(&plateau.tiles[i], axis) == first);
        if all_match {
            reward += (first * multiplier) as f64 / 100.0;
        }
    }
    reward
}

// ── Collect trajectories (sampling from policy) ───────────────────

fn collect_trajectories(
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    n_games: usize,
    rng: &mut StdRng,
) -> Vec<Trajectory> {
    let mut trajectories = Vec::with_capacity(n_games);

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut available: Vec<Tile> = deck
            .tiles()
            .iter()
            .copied()
            .filter(|t| *t != Tile(0, 0, 0))
            .collect();

        let mut steps = Vec::with_capacity(19);

        for turn in 0..19 {
            if available.is_empty() {
                break;
            }
            let tile_idx = rng.random_range(0..available.len());
            let tile = available.remove(tile_idx);
            deck = replace_tile_in_deck(&deck, &tile);

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let mut mask_arr = [0.0f32; 19];
            for i in 0..19 {
                if plateau.tiles[i] != Tile(0, 0, 0) {
                    mask_arr[i] = f32::NEG_INFINITY;
                }
            }
            let mask_tensor = Tensor::from_slice(&mask_arr);

            let feat_batch = feat.unsqueeze(0);
            let logits = tch::no_grad(|| policy_net.forward(&feat_batch, false)).squeeze_dim(0);
            let masked_logits = &logits + &mask_tensor;
            let probs = masked_logits.softmax(-1, Kind::Float);
            let action = probs.multinomial(1, true).int64_value(&[0]);

            let value = tch::no_grad(|| value_net.forward(&feat_batch, false))
                .squeeze_dim(0)
                .double_value(&[0]);

            plateau.tiles[action as usize] = tile;
            let step_reward = line_completion_reward(&plateau, action as usize);

            steps.push(Step {
                features: feat,
                mask: mask_tensor,
                action,
                value,
                reward: step_reward,
            });
        }

        let score = result(&plateau) as f64;
        trajectories.push(Trajectory { steps, final_score: score });
    }

    trajectories
}

// ── GAE computation ────────────────────────────────────────────────

fn compute_gae(traj: &Trajectory, gamma: f64, lambda: f64, terminal_coeff: f64) -> (Vec<f64>, Vec<f64>) {
    let n = traj.steps.len();
    let mut advantages = vec![0.0; n];
    let mut returns = vec![0.0; n];
    let mut gae = 0.0;

    // Normalised terminal reward: (score - 100) / 100
    let terminal_reward = (traj.final_score - 100.0) / 100.0;

    for t in (0..n).rev() {
        let next_value = if t == n - 1 { 0.0 } else { traj.steps[t + 1].value };
        // Dense reward from line completion + optional terminal bonus at last step
        let r = traj.steps[t].reward
            + if t == n - 1 { terminal_coeff * terminal_reward } else { 0.0 };
        let delta = r + gamma * next_value - traj.steps[t].value;
        gae = delta + gamma * lambda * gae;
        advantages[t] = gae;
        returns[t] = advantages[t] + traj.steps[t].value;
    }

    (advantages, returns)
}

// ── REINFORCE update (single epoch) ───────────────────────────────

struct FlatStep {
    features: Tensor,
    mask: Tensor,
    action: i64,
    advantage: f64,
    ret: f64,
}

struct UpdateStats {
    policy_loss: f64,
    value_loss: f64,
    entropy: f64,
    nan_batches: usize,
}

fn reinforce_update(
    policy_net: &GraphTransformerPolicyNet,
    value_net: &GraphTransformerValueNet,
    opt_policy: &mut nn::Optimizer,
    opt_value: &mut nn::Optimizer,
    flat: &[FlatStep],
    args: &Args,
    rng: &mut StdRng,
) -> UpdateStats {
    let n = flat.len();
    let mut total_ploss = 0.0;
    let mut total_vloss = 0.0;
    let mut total_entropy = 0.0;
    let mut n_updates = 0;
    let mut nan_batches = 0;

    // Single epoch — shuffle and iterate minibatches
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(rng);

    let n_batches = n / args.batch_size;
    for batch_i in 0..n_batches {
        let start = batch_i * args.batch_size;
        let end = start + args.batch_size;
        let batch_idx = &indices[start..end];

        let features: Vec<Tensor> = batch_idx.iter().map(|&i| flat[i].features.shallow_clone()).collect();
        let masks: Vec<Tensor> = batch_idx.iter().map(|&i| flat[i].mask.shallow_clone()).collect();
        let actions: Vec<i64> = batch_idx.iter().map(|&i| flat[i].action).collect();
        let advantages_raw: Vec<f64> = batch_idx.iter().map(|&i| flat[i].advantage).collect();
        let rets: Vec<f64> = batch_idx.iter().map(|&i| flat[i].ret).collect();

        let feat_t = Tensor::stack(&features, 0); // [B, 19, 47]
        let mask_t = Tensor::stack(&masks, 0);     // [B, 19]
        let action_t = Tensor::from_slice(&actions);
        let ret_t = Tensor::from_slice(&rets).to_kind(Kind::Float);

        // Normalise advantages per minibatch
        let adv_t = {
            let raw = Tensor::from_slice(&advantages_raw).to_kind(Kind::Float);
            let mean = raw.mean(Kind::Float);
            let std = raw.std(false).clamp_min(1e-8);
            (raw - mean) / std
        };

        // ── Policy forward ──
        let logits = policy_net.forward(&feat_t, false);
        let masked_logits = logits + &mask_t;
        let log_probs_all = masked_logits.log_softmax(-1, Kind::Float);
        let log_prob_actions = log_probs_all
            .gather(1, &action_t.unsqueeze(1), false)
            .squeeze_dim(1); // [B]

        // REINFORCE: loss = -E[log π(a|s) * A(s,a)]
        let policy_loss = -(&log_prob_actions * &adv_t).mean(Kind::Float);

        // Entropy bonus (clamp log_probs to avoid -inf gradient)
        let probs = masked_logits.softmax(-1, Kind::Float);
        let safe_log_probs = log_probs_all.clamp_min(-20.0);
        let entropy = -(probs * safe_log_probs)
            .sum_dim_intlist(-1, false, Kind::Float)
            .mean(Kind::Float);

        // ── Policy backward ──
        let p_loss = &policy_loss - args.entropy_coeff * &entropy;
        opt_policy.zero_grad();
        p_loss.backward();

        // NaN gradient guard
        let policy_grad_ok = !opt_policy.trainable_variables().iter().any(|t| {
            let g = t.grad();
            g.defined()
                && (g.isnan().any().int64_value(&[]) != 0
                    || g.isinf().any().int64_value(&[]) != 0)
        });
        if policy_grad_ok {
            opt_policy.clip_grad_norm(1.0);
            opt_policy.step();
        } else {
            nan_batches += 1;
        }

        // ── Value forward + backward ──
        let new_value = value_net.forward(&feat_t, false).squeeze_dim(-1);
        let value_loss = (&new_value - &ret_t).pow_tensor_scalar(2).mean(Kind::Float);
        let v_loss = args.value_coeff * &value_loss;
        opt_value.zero_grad();
        v_loss.backward();
        opt_value.clip_grad_norm(1.0);
        opt_value.step();

        total_ploss += f64::try_from(&policy_loss).unwrap();
        total_vloss += f64::try_from(&value_loss).unwrap();
        total_entropy += f64::try_from(&entropy).unwrap();
        n_updates += 1;
    }

    if n_updates == 0 {
        return UpdateStats { policy_loss: 0.0, value_loss: 0.0, entropy: 0.0, nan_batches };
    }

    UpdateStats {
        policy_loss: total_ploss / n_updates as f64,
        value_loss: total_vloss / n_updates as f64,
        entropy: total_entropy / n_updates as f64,
        nan_batches,
    }
}

// ── Evaluation (GT Direct argmax) ──────────────────────────────────

fn eval_model(policy_net: &GraphTransformerPolicyNet, n_games: usize, rng: &mut StdRng) -> f64 {
    let mut total = 0i64;

    for _ in 0..n_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut available: Vec<Tile> = deck
            .tiles()
            .iter()
            .copied()
            .filter(|t| *t != Tile(0, 0, 0))
            .collect();

        for turn in 0..19 {
            if available.is_empty() {
                break;
            }
            let tile_idx = rng.random_range(0..available.len());
            let tile = available.remove(tile_idx);
            deck = replace_tile_in_deck(&deck, &tile);

            let legal = get_legal_moves(&plateau);
            if legal.is_empty() {
                break;
            }

            let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19).unsqueeze(0);
            let logits = tch::no_grad(|| policy_net.forward(&feat, false)).squeeze_dim(0);
            let mut mask = [0.0f32; 19];
            for i in 0..19 {
                if plateau.tiles[i] != Tile(0, 0, 0) {
                    mask[i] = f32::NEG_INFINITY;
                }
            }
            let masked = logits + Tensor::from_slice(&mask);
            let best_pos = masked.argmax(-1, false).int64_value(&[]) as usize;
            plateau.tiles[best_pos] = tile;
        }

        total += result(&plateau) as i64;
    }

    total as f64 / n_games as f64
}

// ── Main ───────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();

    println!("==========================================");
    println!("  REINFORCE + Dense Reward — RL Trainer");
    println!("==========================================\n");

    println!("Config:");
    println!("  Iterations:     {}", args.iterations);
    println!("  Games/iter:     {}", args.games_per_iter);
    println!("  Eval games:     {}", args.eval_games);
    println!("  Batch size:     {}", args.batch_size);
    println!("  LR policy:      {}", args.lr_policy);
    println!("  LR value:       {}", args.lr_value);
    println!("  Gamma:          {}", args.gamma);
    println!("  GAE lambda:     {}", args.gae_lambda);
    println!("  Terminal coeff: {}", args.terminal_coeff);
    println!("  Entropy coeff:  {}", args.entropy_coeff);
    println!("  Value coeff:    {}", args.value_coeff);
    println!("  Patience:       {}", args.patience);
    println!("  Load from:      {}", args.load_path);
    println!("  Save to:        {}", args.save_path);

    let device = match parse_device(&args.device) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };
    println!("  Device:         {:?}", device);

    // ── Policy VarStore (initialised from production GT) ──
    let mut policy_vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(&policy_vs, 47, 128, 2, 4, 0.1);

    if !Path::new(&args.load_path).exists() {
        eprintln!("\nError: model weights not found: {}", args.load_path);
        return;
    }
    match load_varstore(&mut policy_vs, &args.load_path) {
        Ok(()) => println!("\n  Loaded policy from {}", args.load_path),
        Err(e) => {
            eprintln!("\nError loading policy weights: {}", e);
            return;
        }
    }

    // ── Value VarStore (random init) ──
    let value_vs = nn::VarStore::new(device);
    let value_net = GraphTransformerValueNet::new(&value_vs, 47, 128, 2, 4, 0.1);
    println!("  Value net: random init");

    // ── Optimizers ──
    let mut opt_policy = nn::Adam::default().build(&policy_vs, args.lr_policy).unwrap();
    let mut opt_value = nn::Adam::default().build(&value_vs, args.lr_value).unwrap();

    // ── Baseline evaluation ──
    let mut rng = StdRng::seed_from_u64(args.seed);
    println!("\n--- Baseline evaluation ({} games) ---", args.eval_games);
    let baseline = eval_model(&policy_net, args.eval_games, &mut rng);
    println!("  GT Direct baseline: {:.1} pts", baseline);

    let mut best_score = baseline;
    let mut no_improve = 0usize;
    let total_start = Instant::now();

    // ═════════════════════════════════════════════
    //          REINFORCE Main Loop
    // ═════════════════════════════════════════════
    for iter in 0..args.iterations {
        let iter_start = Instant::now();
        println!(
            "\n══════════════════════════════════════════",
        );
        println!(
            "  Iteration {}/{} | best={:.1} | no_improve={}/{}",
            iter + 1, args.iterations, best_score, no_improve, args.patience
        );
        println!("══════════════════════════════════════════");

        // 1. Collect trajectories
        print!("  [1/3] Collecting {} games...", args.games_per_iter);
        let collect_start = Instant::now();
        let trajectories = collect_trajectories(
            &policy_net,
            &value_net,
            args.games_per_iter,
            &mut rng,
        );

        let avg_score: f64 = trajectories
            .iter()
            .map(|t| t.final_score)
            .sum::<f64>()
            / trajectories.len() as f64;
        let avg_dense: f64 = trajectories
            .iter()
            .map(|t| t.steps.iter().map(|s| s.reward).sum::<f64>())
            .sum::<f64>()
            / trajectories.len() as f64;
        println!(
            " avg={:.1} pts dense_r={:.3} ({:.1}s)",
            avg_score, avg_dense, collect_start.elapsed().as_secs_f64()
        );

        // 2. Compute GAE and flatten
        let mut flat_steps: Vec<FlatStep> = Vec::new();
        for traj in &trajectories {
            let (advantages, returns) = compute_gae(traj, args.gamma, args.gae_lambda, args.terminal_coeff);
            for (i, step) in traj.steps.iter().enumerate() {
                flat_steps.push(FlatStep {
                    features: step.features.shallow_clone(),
                    mask: step.mask.shallow_clone(),
                    action: step.action,
                    advantage: advantages[i],
                    ret: returns[i],
                });
            }
        }

        let avg_adv: f64 = flat_steps.iter().map(|s| s.advantage).sum::<f64>() / flat_steps.len() as f64;
        let std_adv: f64 = {
            let var: f64 = flat_steps.iter().map(|s| (s.advantage - avg_adv).powi(2)).sum::<f64>()
                / flat_steps.len() as f64;
            var.sqrt()
        };
        println!("  Steps: {} | Adv: mean={:.3} std={:.3}", flat_steps.len(), avg_adv, std_adv);

        // 3. REINFORCE update (single epoch)
        print!("  [2/3] REINFORCE update (bs={})...", args.batch_size);
        let stats = reinforce_update(
            &policy_net,
            &value_net,
            &mut opt_policy,
            &mut opt_value,
            &flat_steps,
            &args,
            &mut rng,
        );
        if stats.nan_batches > 0 {
            println!(
                " pi_loss={:.4} v_loss={:.4} ent={:.4} nan_skip={}",
                stats.policy_loss, stats.value_loss, stats.entropy, stats.nan_batches
            );
        } else {
            println!(
                " pi_loss={:.4} v_loss={:.4} ent={:.4}",
                stats.policy_loss, stats.value_loss, stats.entropy
            );
        }

        // 4. Evaluate
        print!("  [3/3] Eval ({} games GT Direct)...", args.eval_games);
        let new_score = eval_model(&policy_net, args.eval_games, &mut rng);
        let delta = new_score - best_score;
        let iter_elapsed = iter_start.elapsed().as_secs_f64();

        if new_score > best_score {
            println!(
                " {:.1} pts ({:+.1} vs best) *** NEW BEST *** ({:.0}s)",
                new_score, delta, iter_elapsed
            );
            best_score = new_score;
            no_improve = 0;

            if let Err(e) = save_varstore(&policy_vs, &args.save_path) {
                eprintln!("  Warning: failed to save: {}", e);
            } else {
                println!("  Saved to {}", args.save_path);
            }
        } else {
            no_improve += 1;
            println!(
                " {:.1} pts ({:+.1} vs best) | no_improve={}/{} ({:.0}s)",
                new_score, delta, no_improve, args.patience, iter_elapsed
            );

            if no_improve >= args.patience {
                println!("\n  Early stopping: no improvement for {} iterations", args.patience);
                break;
            }
        }
    }

    // ═════════════════════════════════════════════
    //              Final Summary
    // ═════════════════════════════════════════════
    let total_elapsed = total_start.elapsed().as_secs_f64();
    println!("\n==========================================");
    println!("       REINFORCE TRAINING COMPLETE");
    println!("==========================================");
    println!("  Baseline GT Direct:  {:.1} pts", baseline);
    println!("  Best score:          {:.1} pts", best_score);
    println!("  Delta:               {:+.1} pts", best_score - baseline);
    println!("  Total time:          {:.0}s ({:.1} min)", total_elapsed, total_elapsed / 60.0);

    // Final verification with fresh seed
    if best_score > baseline {
        println!("\n--- Final verification (500 games, fresh seed) ---");
        if let Err(e) = load_varstore(&mut policy_vs, &args.save_path) {
            eprintln!("  Warning: could not reload best weights: {}", e);
        }
        let mut verify_rng = StdRng::seed_from_u64(args.seed + 9999);
        let final_score = eval_model(&policy_net, 500, &mut verify_rng);
        println!("  GT Direct (best REINFORCE): {:.1} pts (500 games)", final_score);
    }
}
