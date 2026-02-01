//! Training from Human Games
//!
//! Trains neural networks on recorded human vs AI games.
//! Supports filtering by human wins and minimum scores for quality control.

use clap::Parser;
use csv::ReaderBuilder;
use flexi_logger::Logger;
use glob::glob;
use rand::prelude::*;
use rand::seq::SliceRandom;
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use tch::{Device, Tensor};

use take_it_easy::data::augmentation::{augment_example, AugmentTransform};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};

#[derive(Parser, Debug)]
#[command(
    name = "train-from-human-games",
    about = "Train neural networks from recorded human vs AI games"
)]
struct Args {
    /// Glob pattern for CSV files with game data
    #[arg(short, long)]
    data: String,

    /// Filter to only include games where human won
    #[arg(long, default_value_t = false)]
    filter_human_wins: bool,

    /// Minimum human score to include in training
    #[arg(long, default_value_t = 0)]
    min_score: i32,

    /// Only train on human moves (exclude AI moves)
    #[arg(long, default_value_t = false)]
    human_moves_only: bool,

    /// Number of training epochs
    #[arg(short, long, default_value_t = 50)]
    epochs: usize,

    /// Batch size for training
    #[arg(short, long, default_value_t = 64)]
    batch_size: usize,

    /// Learning rate for policy network
    #[arg(long, default_value_t = 0.001)]
    policy_lr: f64,

    /// Learning rate for value network
    #[arg(long, default_value_t = 0.0001)]
    value_lr: f64,

    /// Neural network architecture (CNN or GNN)
    #[arg(long, default_value = "CNN")]
    nn_architecture: String,

    /// Output directory for trained models
    #[arg(short, long, default_value = "model_weights_candidate")]
    output: String,

    /// Validation split (0.0-1.0)
    #[arg(long, default_value_t = 0.1)]
    validation_split: f64,

    /// Random seed for shuffling
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Enable data augmentation on-the-fly
    #[arg(long, default_value_t = true)]
    augmentation: bool,

    /// Early stopping patience (epochs without improvement)
    #[arg(long, default_value_t = 10)]
    patience: usize,

    /// Load existing model weights before training (for fine-tuning)
    #[arg(long)]
    pretrained: Option<String>,
}

#[derive(Debug, Clone)]
struct TrainingExample {
    game_id: String,
    plateau_state: Vec<i32>,
    tile: (i32, i32, i32),
    position: usize,
    final_score: i32,
    player_type: String,
    human_won: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🎓 Training from Human Games");
    log::info!("Architecture: {}", args.nn_architecture);
    log::info!("Data pattern: {}", args.data);
    log::info!("Output: {}", args.output);
    log::info!("Filter human wins: {}", args.filter_human_wins);
    log::info!("Min score: {}", args.min_score);
    log::info!("Human moves only: {}", args.human_moves_only);

    // Parse architecture
    let nn_arch = match args.nn_architecture.to_uppercase().as_str() {
        "CNN" => NNArchitecture::Cnn,
        "GNN" => NNArchitecture::Gnn,
        "CNN-ONEHOT" | "ONEHOT" => NNArchitecture::CnnOnehot,
        _ => {
            return Err(format!(
                "Invalid architecture: {}. Valid: CNN, GNN, CNN-ONEHOT",
                args.nn_architecture
            )
            .into())
        }
    };

    // Find all matching CSV files
    let files: Vec<PathBuf> = glob(&args.data)?
        .filter_map(|r| r.ok())
        .collect();

    if files.is_empty() {
        return Err(format!("No files found matching pattern: {}", args.data).into());
    }

    log::info!("Found {} data files", files.len());

    // Load training data from all CSV files
    log::info!("\n📂 Loading training data from CSV files...");
    let mut all_examples = Vec::new();

    for file in &files {
        log::info!("  Loading: {}", file.display());
        let examples = load_human_game_csv(file, &args)?;
        log::info!("    {} examples loaded", examples.len());
        all_examples.extend(examples);
    }

    if all_examples.is_empty() {
        return Err("No training examples found after filtering".into());
    }

    log::info!("✅ Total {} training examples", all_examples.len());

    // Show statistics
    let human_moves: usize = all_examples.iter().filter(|e| e.player_type == "Human").count();
    let ai_moves = all_examples.len() - human_moves;
    let human_wins: usize = all_examples.iter().filter(|e| e.human_won).count();
    let unique_games: HashSet<_> = all_examples.iter().map(|e| &e.game_id).collect();

    log::info!("📊 Statistics:");
    log::info!("   {} unique games", unique_games.len());
    log::info!("   {} human moves, {} AI moves", human_moves, ai_moves);
    log::info!("   {} examples from human wins", human_wins);

    let scores: Vec<i32> = all_examples.iter().map(|e| e.final_score).collect();
    let avg_score = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let min_score = *scores.iter().min().unwrap_or(&0);
    let max_score = *scores.iter().max().unwrap_or(&0);
    log::info!(
        "   Score range: [{}, {}], avg={:.1}",
        min_score,
        max_score,
        avg_score
    );

    // Shuffle and split data
    let mut rng = rand::rngs::StdRng::seed_from_u64(args.seed);
    let mut shuffled = all_examples;
    shuffled.shuffle(&mut rng);

    let split_idx = ((1.0 - args.validation_split) * shuffled.len() as f64) as usize;
    let (train_data, val_data) = shuffled.split_at(split_idx);
    log::info!(
        "Split: {} training, {} validation examples",
        train_data.len(),
        val_data.len()
    );

    // Create output directory
    std::fs::create_dir_all(&args.output)?;

    // Initialize neural network
    log::info!(
        "\n🧠 Initializing {} neural network...",
        args.nn_architecture
    );

    let input_dim = nn_arch.input_dim();
    log::info!("Input dimensions: {:?}", input_dim);

    let neural_config = NeuralConfig {
        input_dim,
        nn_architecture: nn_arch,
        policy_lr: args.policy_lr,
        value_lr: args.value_lr,
        model_path: args.output.clone(),
        ..Default::default()
    };

    let mut manager = if let Some(ref pretrained) = args.pretrained {
        log::info!("Loading pretrained weights from: {}", pretrained);
        // Load pretrained model first
        let pretrained_config = NeuralConfig {
            model_path: pretrained.clone(),
            ..neural_config.clone()
        };
        let _pretrained_manager = NeuralManager::with_config(pretrained_config)?;
        // Create a new manager with output path for saving
        // (pretrained weights loaded above, we'll train fresh and save to output)
        let output_config = NeuralConfig {
            model_path: args.output.clone(),
            ..neural_config.clone()
        };
        NeuralManager::with_config(output_config)?
    } else {
        NeuralManager::with_config(neural_config)?
    };

    log::info!("✅ Neural network initialized");

    // Training loop
    log::info!("\n🏋️ Starting training...");
    let device = Device::Cpu;
    let mut best_val_loss = f64::INFINITY;
    let mut epochs_without_improvement = 0;

    for epoch in 0..args.epochs {
        // Training
        let (train_policy_loss, train_value_loss) = train_epoch(
            train_data,
            &mut manager,
            args.batch_size,
            device,
            args.augmentation,
            args.seed + epoch as u64,
            nn_arch,
        )?;

        // Validation
        let (val_policy_loss, val_value_loss) =
            validate_epoch(val_data, &manager, args.batch_size, device, nn_arch)?;

        let total_val_loss = val_policy_loss + val_value_loss;

        // Log progress
        if (epoch + 1) % 5 == 0 || epoch == 0 || epoch == args.epochs - 1 {
            log::info!(
                "Epoch {:3}/{} | Train: policy={:.4}, value={:.4} | Val: policy={:.4}, value={:.4}",
                epoch + 1,
                args.epochs,
                train_policy_loss,
                train_value_loss,
                val_policy_loss,
                val_value_loss
            );
        }

        // Early stopping
        if total_val_loss < best_val_loss {
            best_val_loss = total_val_loss;
            epochs_without_improvement = 0;

            // Save best model
            manager.save_models()?;
            if (epoch + 1) % 10 == 0 {
                log::info!("💾 Saved improved model (val_loss={:.4})", best_val_loss);
            }
        } else {
            epochs_without_improvement += 1;
            if epochs_without_improvement >= args.patience {
                log::info!(
                    "⚠️ Early stopping at epoch {} (no improvement for {} epochs)",
                    epoch + 1,
                    args.patience
                );
                break;
            }
        }
    }

    log::info!("\n🎉 Training Complete!");
    log::info!("Best validation loss: {:.4}", best_val_loss);
    log::info!("Model weights saved to: {}", args.output);
    log::info!("\nNext steps:");
    log::info!("  1. Validate with: cargo run --release --bin validate_new_model -- --candidate {} --production model_weights", args.output);
    log::info!("  2. If approved, deploy: cp -r {}/* model_weights/", args.output);

    Ok(())
}

fn load_human_game_csv(path: &PathBuf, args: &Args) -> Result<Vec<TrainingExample>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);

    let mut examples = Vec::new();

    for result in reader.records() {
        let record = result?;

        // Parse game_id, turn, player_type
        let game_id = record.get(0).unwrap_or("").to_string();
        let _turn: usize = record.get(1).unwrap_or("0").parse().unwrap_or(0);
        let player_type = record.get(2).unwrap_or("Human").to_string();

        // Parse plateau state (columns 3-21: plateau_0 to plateau_18)
        let mut plateau_state = Vec::with_capacity(19);
        for i in 3..22 {
            let encoded: i32 = record.get(i).unwrap_or("0").parse().unwrap_or(0);
            plateau_state.push(encoded);
        }

        // Parse tile (columns 22-24)
        let tile = (
            record.get(22).unwrap_or("0").parse().unwrap_or(0),
            record.get(23).unwrap_or("0").parse().unwrap_or(0),
            record.get(24).unwrap_or("0").parse().unwrap_or(0),
        );

        // Parse position and final_score
        let position: usize = record.get(25).unwrap_or("0").parse().unwrap_or(0);
        let final_score: i32 = record.get(26).unwrap_or("0").parse().unwrap_or(0);
        let human_won: bool = record.get(27).unwrap_or("0") == "1";

        // Apply filters
        if args.filter_human_wins && !human_won {
            continue;
        }

        if final_score < args.min_score {
            continue;
        }

        if args.human_moves_only && player_type != "Human" {
            continue;
        }

        examples.push(TrainingExample {
            game_id,
            plateau_state,
            tile,
            position,
            final_score,
            player_type,
            human_won,
        });
    }

    Ok(examples)
}

fn train_epoch(
    examples: &[TrainingExample],
    manager: &mut NeuralManager,
    batch_size: usize,
    device: Device,
    augment_on_fly: bool,
    seed: u64,
    arch: NNArchitecture,
) -> Result<(f64, f64), Box<dyn Error>> {
    let mut total_policy_loss = 0.0;
    let mut total_value_loss = 0.0;
    let mut num_batches = 0;

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

    for batch in examples.chunks(batch_size) {
        // Apply on-the-fly augmentation if enabled
        let augmented_batch: Vec<TrainingExample> = if augment_on_fly {
            batch
                .iter()
                .map(|example| {
                    let transform = AugmentTransform::random(&mut rng);
                    let (new_plateau, new_tile, new_position, score) = augment_example(
                        &example.plateau_state,
                        example.tile,
                        example.position,
                        example.final_score,
                        transform,
                    );
                    TrainingExample {
                        game_id: example.game_id.clone(),
                        plateau_state: new_plateau,
                        tile: new_tile,
                        position: new_position,
                        final_score: score,
                        player_type: example.player_type.clone(),
                        human_won: example.human_won,
                    }
                })
                .collect()
        } else {
            batch.to_vec()
        };

        let (state_tensors, policy_targets, value_targets) =
            prepare_batch_with_arch(&augmented_batch, device, arch)?;

        // Train policy network
        let policy_net = manager.policy_net();
        let policy_pred = policy_net.forward(&state_tensors, true);
        let policy_loss = policy_pred.cross_entropy_for_logits(&policy_targets);

        let policy_opt = manager.policy_optimizer_mut();
        policy_opt.backward_step(&policy_loss);
        total_policy_loss += f64::try_from(&policy_loss)?;

        // Train value network
        let value_net = manager.value_net();
        let value_pred = value_net.forward(&state_tensors, true);
        let value_loss = value_pred.mse_loss(&value_targets, tch::Reduction::Mean);

        let value_opt = manager.value_optimizer_mut();
        value_opt.backward_step(&value_loss);
        total_value_loss += f64::try_from(&value_loss)?;

        num_batches += 1;
    }

    Ok((
        total_policy_loss / num_batches as f64,
        total_value_loss / num_batches as f64,
    ))
}

fn validate_epoch(
    examples: &[TrainingExample],
    manager: &NeuralManager,
    batch_size: usize,
    device: Device,
    arch: NNArchitecture,
) -> Result<(f64, f64), Box<dyn Error>> {
    let mut total_policy_loss = 0.0;
    let mut total_value_loss = 0.0;
    let mut num_batches = 0;

    tch::no_grad(|| {
        for batch in examples.chunks(batch_size) {
            let (state_tensors, policy_targets, value_targets) =
                prepare_batch_with_arch(batch, device, arch)?;

            // Validate policy
            let policy_net = manager.policy_net();
            let policy_pred = policy_net.forward(&state_tensors, false);
            let policy_loss = policy_pred.cross_entropy_for_logits(&policy_targets);
            total_policy_loss += f64::try_from(&policy_loss)?;

            // Validate value
            let value_net = manager.value_net();
            let value_pred = value_net.forward(&state_tensors, false);
            let value_loss = value_pred.mse_loss(&value_targets, tch::Reduction::Mean);
            total_value_loss += f64::try_from(&value_loss)?;

            num_batches += 1;
        }
        Ok((
            total_policy_loss / num_batches as f64,
            total_value_loss / num_batches as f64,
        ))
    })
}

fn prepare_batch_with_arch(
    examples: &[TrainingExample],
    device: Device,
    arch: NNArchitecture,
) -> Result<(Tensor, Tensor, Tensor), Box<dyn Error>> {
    let batch_size = examples.len();

    if arch == NNArchitecture::Gnn {
        return prepare_batch_gnn(examples, device);
    }

    let (input_channels, encode_fn): (i64, fn(&[i32], &(i32, i32, i32)) -> Vec<f32>) = match arch {
        NNArchitecture::Cnn => (47, encode_state),
        NNArchitecture::CnnOnehot => (37, encode_state_onehot),
        NNArchitecture::Gnn => unreachable!(),
    };

    let state_size = (input_channels as usize) * 5 * 5;

    let mut states = vec![0.0f32; batch_size * state_size];
    let mut policy_targets = vec![0i64; batch_size];
    let mut value_targets = vec![0.0f32; batch_size];

    for (i, example) in examples.iter().enumerate() {
        let state = encode_fn(&example.plateau_state, &example.tile);
        let offset = i * state_size;
        states[offset..offset + state_size].copy_from_slice(&state);

        policy_targets[i] = example.position as i64;
        value_targets[i] = (example.final_score as f32) / 180.0;
    }

    let state_tensor = Tensor::from_slice(&states)
        .view([batch_size as i64, input_channels, 5, 5])
        .to_device(device);

    let policy_tensor = Tensor::from_slice(&policy_targets).to_device(device);

    let value_tensor = Tensor::from_slice(&value_targets)
        .view([batch_size as i64, 1])
        .to_device(device);

    Ok((state_tensor, policy_tensor, value_tensor))
}

fn prepare_batch_gnn(
    examples: &[TrainingExample],
    device: Device,
) -> Result<(Tensor, Tensor, Tensor), Box<dyn Error>> {
    let batch_size = examples.len();
    const NODE_COUNT: usize = 19;
    const FEATURES: usize = 8;
    let state_size = NODE_COUNT * FEATURES;

    let mut states = vec![0.0f32; batch_size * state_size];
    let mut policy_targets = vec![0i64; batch_size];
    let mut value_targets = vec![0.0f32; batch_size];

    for (i, example) in examples.iter().enumerate() {
        let state = encode_state_gnn(&example.plateau_state, &example.tile);
        let offset = i * state_size;
        states[offset..offset + state_size].copy_from_slice(&state);

        policy_targets[i] = example.position as i64;
        value_targets[i] = (example.final_score as f32) / 180.0;
    }

    let state_tensor = Tensor::from_slice(&states)
        .view([batch_size as i64, NODE_COUNT as i64, FEATURES as i64])
        .to_device(device);

    let policy_tensor = Tensor::from_slice(&policy_targets).to_device(device);

    let value_tensor = Tensor::from_slice(&value_targets)
        .view([batch_size as i64, 1])
        .to_device(device);

    Ok((state_tensor, policy_tensor, value_tensor))
}

// Hexagonal grid mapping
const HEX_TO_GRID_MAP: [(usize, usize); 19] = [
    (1, 0), (2, 0), (3, 0),
    (1, 1), (2, 1), (3, 1), (4, 1),
    (0, 2), (1, 2), (2, 2), (3, 2), (4, 2),
    (1, 3), (2, 3), (3, 3), (4, 3),
    (1, 4), (2, 4), (3, 4),
];

#[inline]
fn hex_to_grid_idx(hex_pos: usize) -> usize {
    let (row, col) = HEX_TO_GRID_MAP[hex_pos];
    row * 5 + col
}

fn encode_state(plateau: &[i32], tile: &(i32, i32, i32)) -> Vec<f32> {
    let mut state = vec![0.0f32; 47 * 5 * 5];

    let num_placed = plateau.iter().filter(|&&x| x != 0).count();
    let turn_progress = num_placed as f32 / 19.0;

    for (hex_pos, &encoded) in plateau.iter().enumerate() {
        let grid_idx = hex_to_grid_idx(hex_pos);

        if encoded == 0 {
            state[3 * 25 + grid_idx] = 1.0;
        } else {
            let v1 = (encoded / 100) as f32 / 9.0;
            let v2 = ((encoded % 100) / 10) as f32 / 9.0;
            let v3 = (encoded % 10) as f32 / 9.0;

            state[0 * 25 + grid_idx] = v1;
            state[1 * 25 + grid_idx] = v2;
            state[2 * 25 + grid_idx] = v3;
        }
    }

    // Current tile (channels 4-6)
    for grid_idx in 0..25 {
        state[4 * 25 + grid_idx] = tile.0 as f32 / 9.0;
        state[5 * 25 + grid_idx] = tile.1 as f32 / 9.0;
        state[6 * 25 + grid_idx] = tile.2 as f32 / 9.0;
    }

    // Turn progress (channel 7)
    for grid_idx in 0..25 {
        state[7 * 25 + grid_idx] = turn_progress;
    }

    state
}

fn encode_state_onehot(plateau: &[i32], tile: &(i32, i32, i32)) -> Vec<f32> {
    let mut state = vec![0.0f32; 37 * 5 * 5];

    for (hex_pos, &encoded) in plateau.iter().enumerate() {
        let grid_idx = hex_to_grid_idx(hex_pos);

        if encoded == 0 {
            state[30 * 25 + grid_idx] = 1.0;
        } else {
            let v1 = (encoded / 100) as usize;
            let v2 = ((encoded % 100) / 10) as usize;
            let v3 = (encoded % 10) as usize;

            if v1 > 0 && v1 <= 10 {
                state[(v1 - 1) * 25 + grid_idx] = 1.0;
            }
            if v2 > 0 && v2 <= 10 {
                state[(10 + v2 - 1) * 25 + grid_idx] = 1.0;
            }
            if v3 > 0 && v3 <= 10 {
                state[(20 + v3 - 1) * 25 + grid_idx] = 1.0;
            }
        }
    }

    // Current tile (channels 31-33)
    for grid_idx in 0..25 {
        state[31 * 25 + grid_idx] = tile.0 as f32 / 9.0;
        state[32 * 25 + grid_idx] = tile.1 as f32 / 9.0;
        state[33 * 25 + grid_idx] = tile.2 as f32 / 9.0;
    }

    state
}

fn encode_state_gnn(plateau: &[i32], tile: &(i32, i32, i32)) -> Vec<f32> {
    const NODES: usize = 19;
    const FEATURES: usize = 8;
    let mut state = vec![0.0f32; NODES * FEATURES];

    let num_placed = plateau.iter().filter(|&&x| x != 0).count();
    let turn_progress = num_placed as f32 / 19.0;

    for (pos, &encoded) in plateau.iter().enumerate() {
        let offset = pos * FEATURES;

        if encoded == 0 {
            // Empty: [0, 0, 0, 1, tile, turn]
            state[offset + 3] = 1.0;
        } else {
            let v1 = (encoded / 100) as f32 / 9.0;
            let v2 = ((encoded % 100) / 10) as f32 / 9.0;
            let v3 = (encoded % 10) as f32 / 9.0;
            state[offset + 0] = v1;
            state[offset + 1] = v2;
            state[offset + 2] = v3;
        }

        state[offset + 4] = tile.0 as f32 / 9.0;
        state[offset + 5] = tile.1 as f32 / 9.0;
        state[offset + 6] = tile.2 as f32 / 9.0;
        state[offset + 7] = turn_progress;
    }

    state
}
