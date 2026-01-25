//! Q-Value Network Trainer
//!
//! Trains a network to predict Q-values for all 19 positions simultaneously.
//! This is more effective than predicting a single value because:
//! 1. The network learns relative value of different positions
//! 2. All Q-value information is used during training
//! 3. Better generalization to unseen positions

use clap::Parser;
use csv::ReaderBuilder;
use flexi_logger::Logger;
use rand::prelude::*;
use rand::seq::SliceRandom;
use std::error::Error;
use std::fs::File;
use tch::{nn, nn::OptimizerConfig, Device, Kind, Tensor};

#[derive(Parser, Debug)]
#[command(name = "train-qvalue-net", about = "Train Q-value network on rollout data")]
struct Args {
    /// CSV file with Q-value data (from generate_qvalues)
    #[arg(short, long)]
    data: String,

    /// Number of training epochs
    #[arg(short, long, default_value_t = 100)]
    epochs: usize,

    /// Batch size
    #[arg(short, long, default_value_t = 64)]
    batch_size: usize,

    /// Learning rate
    #[arg(long, default_value_t = 0.0001)]
    lr: f64,

    /// Validation split
    #[arg(long, default_value_t = 0.15)]
    validation_split: f64,

    /// Early stopping patience
    #[arg(long, default_value_t = 15)]
    patience: usize,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

#[derive(Debug, Clone)]
struct QValueExample {
    plateau_state: Vec<i32>,
    tile: (i32, i32, i32),
    qvalues: [f32; 19],
    mask: [f32; 19], // 1.0 for valid positions, 0.0 for occupied
}

/// Q-Value Network: predicts 19 Q-values (one per position)
struct QValueNet {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    conv2: nn::Conv2D,
    bn2: nn::BatchNorm,
    conv3: nn::Conv2D,
    bn3: nn::BatchNorm,
    fc1: nn::Linear,
    fc2: nn::Linear,
    qvalue_head: nn::Linear, // Output: 19 Q-values
}

impl QValueNet {
    fn new(vs: &nn::VarStore, input_channels: i64) -> Self {
        let p = vs.root();

        let conv1 = nn::conv2d(&p / "conv1", input_channels, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn1 = nn::batch_norm2d(&p / "bn1", 64, Default::default());

        let conv2 = nn::conv2d(&p / "conv2", 64, 128, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn2 = nn::batch_norm2d(&p / "bn2", 128, Default::default());

        let conv3 = nn::conv2d(&p / "conv3", 128, 128, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn3 = nn::batch_norm2d(&p / "bn3", 128, Default::default());

        // 128 channels * 5 * 5 = 3200
        let fc1 = nn::linear(&p / "fc1", 128 * 5 * 5, 512, Default::default());
        let fc2 = nn::linear(&p / "fc2", 512, 256, Default::default());
        let qvalue_head = nn::linear(&p / "qvalue_head", 256, 19, Default::default());

        Self { conv1, bn1, conv2, bn2, conv3, bn3, fc1, fc2, qvalue_head }
    }

    fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let h = x.apply(&self.conv1).apply_t(&self.bn1, train).relu();
        let h = h.apply(&self.conv2).apply_t(&self.bn2, train).relu();
        let h = h.apply(&self.conv3).apply_t(&self.bn3, train).relu();

        let h = h.flat_view();
        let h = h.apply(&self.fc1).relu();
        let h = if train { h.dropout(0.3, train) } else { h };
        let h = h.apply(&self.fc2).relu();
        let h = if train { h.dropout(0.3, train) } else { h };

        h.apply(&self.qvalue_head) // Output: [batch, 19]
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🎯 Q-Value Network Trainer");
    log::info!("Data: {}", args.data);
    log::info!("Epochs: {}, Batch: {}, LR: {}", args.epochs, args.batch_size, args.lr);

    // Load data
    let examples = load_qvalue_data(&args.data)?;
    log::info!("Loaded {} examples", examples.len());

    // Shuffle and split
    let mut rng = rand::rngs::StdRng::seed_from_u64(args.seed);
    let mut shuffled = examples.clone();
    shuffled.shuffle(&mut rng);

    let split_idx = ((1.0 - args.validation_split) * shuffled.len() as f64) as usize;
    let (train_data, val_data) = shuffled.split_at(split_idx);
    log::info!("Split: {} train, {} val", train_data.len(), val_data.len());

    // Initialize network
    let device = Device::Cpu;
    let vs = nn::VarStore::new(device);
    let net = QValueNet::new(&vs, 47); // 47 channels input
    let mut optimizer = nn::Adam::default().build(&vs, args.lr)?;

    let mut best_val_loss = f64::MAX;
    let mut epochs_without_improvement = 0;

    log::info!("\n🏋️ Starting training...");

    for epoch in 1..=args.epochs {
        // Training
        let mut train_loss = 0.0;
        let mut train_batches = 0;

        for batch in train_data.chunks(args.batch_size) {
            let (states, targets, masks) = prepare_batch(batch, device);

            let pred = net.forward(&states, true);

            // === CROSS-ENTROPY LOSS for ranking ===
            // Targets are already softmax distributions, pred needs softmax
            let pred_masked = &pred * &masks + (&masks - 1.0) * 1e9;  // Mask invalid with -inf
            let pred_softmax = pred_masked.softmax(-1, Kind::Float);

            // Cross-entropy: -sum(target * log(pred))
            let loss = -(&targets * (pred_softmax + 1e-10).log())
                .sum(Kind::Float) / (batch.len() as f64);

            optimizer.zero_grad();
            loss.backward();
            optimizer.step();

            train_loss += loss.double_value(&[]);
            train_batches += 1;
        }
        train_loss /= train_batches as f64;

        // Validation
        let mut val_loss = 0.0;
        let mut val_batches = 0;

        for batch in val_data.chunks(args.batch_size) {
            let (states, targets, masks) = prepare_batch(batch, device);

            let pred = net.forward(&states, false);

            // Cross-entropy loss (same as training)
            let pred_masked = &pred * &masks + (&masks - 1.0) * 1e9;
            let pred_softmax = pred_masked.softmax(-1, Kind::Float);
            let loss = -(&targets * (pred_softmax + 1e-10).log())
                .sum(Kind::Float) / (batch.len() as f64);

            val_loss += loss.double_value(&[]);
            val_batches += 1;
        }
        val_loss /= val_batches as f64;

        if epoch % 5 == 0 || epoch == 1 {
            log::info!("Epoch {:3}/{} | Train: {:.6} | Val: {:.6}", epoch, args.epochs, train_loss, val_loss);
        }

        // Early stopping
        if val_loss < best_val_loss {
            best_val_loss = val_loss;
            epochs_without_improvement = 0;
            vs.save("model_weights/qvalue_net.params")?;
            log::info!("  ✅ New best model saved (val_loss: {:.6})", val_loss);
        } else {
            epochs_without_improvement += 1;
            if epochs_without_improvement >= args.patience {
                log::info!("⚠️ Early stopping at epoch {} (no improvement for {} epochs)", epoch, args.patience);
                break;
            }
        }
    }

    log::info!("\n🎉 Training complete! Best val loss: {:.6}", best_val_loss);

    Ok(())
}

fn load_qvalue_data(path: &str) -> Result<Vec<QValueExample>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut reader = ReaderBuilder::new().has_headers(true).from_reader(file);
    let mut examples = Vec::new();

    for result in reader.records() {
        let record = result?;

        // Parse plateau state (columns 2-20)
        let mut plateau_state = Vec::with_capacity(19);
        for i in 2..21 {
            plateau_state.push(record[i].parse()?);
        }

        // Parse tile (columns 21-23)
        let tile = (
            record[21].parse()?,
            record[22].parse()?,
            record[23].parse()?,
        );

        // Parse Q-values (columns 26-44 in 0-indexed, qvalue_0 through qvalue_18)
        let mut qvalues = [0.0f32; 19];
        let mut mask = [0.0f32; 19];
        for i in 0..19 {
            let qv: f32 = record[26 + i].parse().unwrap_or(-1.0);
            qvalues[i] = qv.max(0.0); // Clamp negative (invalid) to 0
            mask[i] = if qv >= 0.0 { 1.0 } else { 0.0 }; // Valid positions only
        }

        // === NORMALIZE Q-VALUES TO SOFTMAX DISTRIBUTION ===
        // This makes the network learn RANKING, not absolute values
        let temperature = 0.1;  // Low temperature = sharper ranking
        let valid_count = mask.iter().filter(|&&m| m > 0.0).count();

        if valid_count > 1 {
            // Find max for numerical stability
            let max_q = qvalues.iter()
                .zip(mask.iter())
                .filter(|(_, &m)| m > 0.0)
                .map(|(&q, _)| q)
                .fold(f32::NEG_INFINITY, f32::max);

            // Compute softmax
            let mut exp_sum = 0.0f32;
            for i in 0..19 {
                if mask[i] > 0.0 {
                    qvalues[i] = ((qvalues[i] - max_q) / temperature).exp();
                    exp_sum += qvalues[i];
                }
            }

            // Normalize
            if exp_sum > 0.0 {
                for i in 0..19 {
                    if mask[i] > 0.0 {
                        qvalues[i] /= exp_sum;
                    }
                }
            }
        }

        examples.push(QValueExample { plateau_state, tile, qvalues, mask });
    }

    Ok(examples)
}

fn prepare_batch(examples: &[QValueExample], device: Device) -> (Tensor, Tensor, Tensor) {
    let batch_size = examples.len();
    let state_size = 47 * 5 * 5;

    let mut states = vec![0.0f32; batch_size * state_size];
    let mut targets = vec![0.0f32; batch_size * 19];
    let mut masks = vec![0.0f32; batch_size * 19];

    for (i, ex) in examples.iter().enumerate() {
        let state = encode_state(&ex.plateau_state, &ex.tile);
        let offset = i * state_size;
        states[offset..offset + state_size].copy_from_slice(&state);

        let t_offset = i * 19;
        targets[t_offset..t_offset + 19].copy_from_slice(&ex.qvalues);
        masks[t_offset..t_offset + 19].copy_from_slice(&ex.mask);
    }

    let state_tensor = Tensor::from_slice(&states)
        .view([batch_size as i64, 47, 5, 5])
        .to_device(device);

    let target_tensor = Tensor::from_slice(&targets)
        .view([batch_size as i64, 19])
        .to_device(device);

    let mask_tensor = Tensor::from_slice(&masks)
        .view([batch_size as i64, 19])
        .to_device(device);

    (state_tensor, target_tensor, mask_tensor)
}

/// Hexagonal position to grid index mapping
const HEX_TO_GRID: [(usize, usize); 19] = [
    (1, 0), (2, 0), (3, 0),           // Col 0: positions 0-2
    (0, 1), (1, 1), (2, 1), (3, 1),   // Col 1: positions 3-6
    (0, 2), (1, 2), (2, 2), (3, 2), (4, 2), // Col 2: positions 7-11
    (0, 3), (1, 3), (2, 3), (3, 3),   // Col 3: positions 12-15
    (1, 4), (2, 4), (3, 4),           // Col 4: positions 16-18
];

fn hex_to_grid_idx(hex_pos: usize) -> usize {
    let (row, col) = HEX_TO_GRID[hex_pos];
    row * 5 + col
}

fn encode_state(plateau: &[i32], tile: &(i32, i32, i32)) -> Vec<f32> {
    let mut state = vec![0.0f32; 47 * 5 * 5];

    let num_placed = plateau.iter().filter(|&&x| x != 0).count();
    let turn_progress = num_placed as f32 / 19.0;

    for (hex_pos, &encoded) in plateau.iter().enumerate() {
        let grid_idx = hex_to_grid_idx(hex_pos);

        if encoded == 0 {
            state[3 * 25 + grid_idx] = 1.0; // Empty mask
        } else {
            let v1 = (encoded / 100) as f32 / 9.0;
            let v2 = ((encoded % 100) / 10) as f32 / 9.0;
            let v3 = (encoded % 10) as f32 / 9.0;

            state[grid_idx] = v1;
            state[25 + grid_idx] = v2;
            state[2 * 25 + grid_idx] = v3;
        }

        state[4 * 25 + grid_idx] = tile.0 as f32 / 9.0;
        state[5 * 25 + grid_idx] = tile.1 as f32 / 9.0;
        state[6 * 25 + grid_idx] = tile.2 as f32 / 9.0;
        state[7 * 25 + grid_idx] = turn_progress;
    }

    // Simplified: fill remaining channels with zeros (bag features would go here)
    state
}
