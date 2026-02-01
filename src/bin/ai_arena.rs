//! AI Arena - AI vs AI matches for model comparison and training data generation
//!
//! Runs matches between two AI models using the same deck/tiles to ensure
//! fair comparison. Can generate training data from winning model's moves.

use clap::Parser;
use csv::Writer;
use flexi_logger::Logger;
use rand::prelude::*;
use std::error::Error;
use std::fs::File;
use std::path::PathBuf;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::{mcts_find_best_position_for_tile_uct, mcts_find_best_position_for_tile_with_qnet};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::qvalue_net::QValueNet;
use take_it_easy::neural::{NeuralConfig, NeuralManager, QNetManager};
use take_it_easy::recording::encode_plateau;
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "ai-arena",
    about = "Run AI vs AI matches for comparison and training data generation"
)]
struct Args {
    /// Directory containing model A weights
    #[arg(long, default_value = "model_weights")]
    model_a: String,

    /// Directory containing model B weights
    #[arg(long, default_value = "model_weights_candidate")]
    model_b: String,

    /// Name for model A (for display)
    #[arg(long, default_value = "Production")]
    name_a: String,

    /// Name for model B (for display)
    #[arg(long, default_value = "Candidate")]
    name_b: String,

    /// Number of games to play
    #[arg(long, default_value_t = 100)]
    games: usize,

    /// Number of MCTS simulations per move
    #[arg(long, default_value_t = 150)]
    simulations: usize,

    /// Output CSV file for results
    #[arg(short, long, default_value = "data/arena_results.csv")]
    output: String,

    /// Generate training data from winner's moves
    #[arg(long, default_value_t = false)]
    generate_training_data: bool,

    /// Training data output file (if --generate-training-data)
    #[arg(long, default_value = "data/arena_training.csv")]
    training_output: String,

    /// Minimum score to include game in training data (for self-play)
    /// If set, saves ALL moves from games with score >= min_score (not just winners)
    #[arg(long)]
    min_score: Option<i32>,

    /// Neural network architecture
    #[arg(long, default_value = "CNN")]
    nn_architecture: String,

    /// Enable Q-Net Hybrid MCTS (like production)
    #[arg(long, default_value_t = true)]
    hybrid_mcts: bool,

    /// Path to Q-Net weights
    #[arg(long, default_value = "model_weights/qvalue_net.params")]
    qnet_path: String,

    /// Top-K positions for Q-Net pruning
    #[arg(long, default_value_t = 6)]
    top_k: usize,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Exploration rate: probability of taking a random move (0.0-1.0)
    /// Use this to discover new positions not seen during training
    #[arg(long, default_value_t = 0.0)]
    exploration_rate: f64,

    /// Start games from random mid-game positions (number of pre-placed tiles)
    /// 0 = start from empty board, 5 = start with 5 random tiles already placed
    #[arg(long, default_value_t = 0)]
    random_start: usize,

    /// Tournament mode: round-robin with multiple models
    #[arg(long, default_value_t = false)]
    tournament: bool,

    /// Additional model directories for tournament mode (comma-separated)
    #[arg(long)]
    extra_models: Option<String>,
}

#[derive(Debug, Clone)]
struct GameResult {
    game_id: usize,
    model_a_score: i32,
    model_b_score: i32,
    winner: String,
    seed: u64,
}

#[derive(Debug, Clone)]
struct MoveData {
    game_id: usize,
    turn: usize,
    model_name: String,
    plateau: Vec<i32>,
    tile: (i32, i32, i32),
    position: usize,
    final_score: i32,
    won: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🏟️  AI Arena");
    log::info!("Model A ({}): {}", args.name_a, args.model_a);
    log::info!("Model B ({}): {}", args.name_b, args.model_b);
    log::info!("Games: {}", args.games);
    log::info!("Simulations: {}", args.simulations);

    // Parse architecture
    let nn_arch = match args.nn_architecture.to_uppercase().as_str() {
        "CNN" => NNArchitecture::Cnn,
        "GNN" => NNArchitecture::Gnn,
        "CNN-ONEHOT" | "ONEHOT" => NNArchitecture::CnnOnehot,
        _ => {
            return Err(format!(
                "Invalid architecture: {}",
                args.nn_architecture
            )
            .into())
        }
    };

    // Load models
    log::info!("\n📦 Loading models...");

    let model_a_config = NeuralConfig {
        input_dim: nn_arch.input_dim(),
        nn_architecture: nn_arch,
        model_path: args.model_a.clone(),
        ..Default::default()
    };
    let model_a = NeuralManager::with_config(model_a_config)?;
    log::info!("✅ Model A ({}) loaded", args.name_a);

    let model_b_config = NeuralConfig {
        input_dim: nn_arch.input_dim(),
        nn_architecture: nn_arch,
        model_path: args.model_b.clone(),
        ..Default::default()
    };
    let model_b = NeuralManager::with_config(model_b_config)?;
    log::info!("✅ Model B ({}) loaded", args.name_b);

    // Load Q-Net for hybrid MCTS
    let qnet: Option<QValueNet> = if args.hybrid_mcts {
        match QNetManager::new(&args.qnet_path) {
            Ok(qnet_manager) => {
                log::info!("✅ Q-Net loaded from {} (top-{})", args.qnet_path, args.top_k);
                Some(qnet_manager.into_net())
            }
            Err(e) => {
                log::warn!("⚠️ Failed to load Q-Net: {}, using basic MCTS", e);
                None
            }
        }
    } else {
        log::info!("ℹ️ Hybrid MCTS disabled, using basic MCTS");
        None
    };

    // Run arena matches
    log::info!("\n🎮 Running arena matches...\n");

    let mut results = Vec::new();
    let mut all_moves = Vec::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(args.seed);

    let mut a_wins = 0;
    let mut b_wins = 0;
    let mut ties = 0;

    for game_idx in 0..args.games {
        let game_seed = rng.random();

        let (game_result, moves) = play_arena_game(
            game_idx,
            &model_a,
            &model_b,
            &args.name_a,
            &args.name_b,
            args.simulations,
            game_seed,
            qnet.as_ref(),
            args.top_k,
            args.exploration_rate,
            args.random_start,
        )?;

        match game_result.winner.as_str() {
            w if w == args.name_a => a_wins += 1,
            w if w == args.name_b => b_wins += 1,
            _ => ties += 1,
        }

        if (game_idx + 1) % 10 == 0 {
            log::info!(
                "Game {:3}/{} | {}: {} | {}: {} | A:{:.1}% B:{:.1}%",
                game_idx + 1,
                args.games,
                args.name_a,
                game_result.model_a_score,
                args.name_b,
                game_result.model_b_score,
                100.0 * a_wins as f64 / (game_idx + 1) as f64,
                100.0 * b_wins as f64 / (game_idx + 1) as f64,
            );
        }

        results.push(game_result);

        if args.generate_training_data {
            all_moves.extend(moves);
        }
    }

    // Calculate statistics
    let a_scores: Vec<i32> = results.iter().map(|r| r.model_a_score).collect();
    let b_scores: Vec<i32> = results.iter().map(|r| r.model_b_score).collect();

    let a_mean = a_scores.iter().sum::<i32>() as f64 / a_scores.len() as f64;
    let b_mean = b_scores.iter().sum::<i32>() as f64 / b_scores.len() as f64;

    let a_std = (a_scores.iter().map(|&s| (s as f64 - a_mean).powi(2)).sum::<f64>() / a_scores.len() as f64).sqrt();
    let b_std = (b_scores.iter().map(|&s| (s as f64 - b_mean).powi(2)).sum::<f64>() / b_scores.len() as f64).sqrt();

    // Print summary
    log::info!("\n═══════════════════════════════════════════════════════════════");
    log::info!("                      ARENA RESULTS");
    log::info!("═══════════════════════════════════════════════════════════════");
    log::info!("");
    log::info!(
        "  {} (A):  {:.2} ± {:.2} pts  |  {} wins ({:.1}%)",
        args.name_a,
        a_mean,
        a_std,
        a_wins,
        100.0 * a_wins as f64 / args.games as f64
    );
    log::info!(
        "  {} (B):  {:.2} ± {:.2} pts  |  {} wins ({:.1}%)",
        args.name_b,
        b_mean,
        b_std,
        b_wins,
        100.0 * b_wins as f64 / args.games as f64
    );
    log::info!("  Ties: {} ({:.1}%)", ties, 100.0 * ties as f64 / args.games as f64);
    log::info!("");
    log::info!("  Score difference: {:+.2} pts", b_mean - a_mean);
    log::info!("");
    log::info!("═══════════════════════════════════════════════════════════════");

    // Save results to CSV
    save_results_csv(&args.output, &results, &args.name_a, &args.name_b)?;
    log::info!("\n📁 Results saved to: {}", args.output);

    // Save training data if requested
    if args.generate_training_data && !all_moves.is_empty() {
        let training_moves: Vec<_> = if let Some(min_score) = args.min_score {
            // Self-play mode: save all moves from games with score >= min_score
            all_moves.into_iter().filter(|m| m.final_score >= min_score).collect()
        } else {
            // Normal mode: save only winner moves
            all_moves.into_iter().filter(|m| m.won).collect()
        };

        let mode_desc = if args.min_score.is_some() {
            format!("moves from games with score >= {}", args.min_score.unwrap())
        } else {
            "moves from winners".to_string()
        };

        save_training_data_csv(&args.training_output, &training_moves)?;
        log::info!("📁 Training data saved to: {} ({} {})",
                   args.training_output, training_moves.len(), mode_desc);
    }

    Ok(())
}

fn play_arena_game(
    game_id: usize,
    model_a: &NeuralManager,
    model_b: &NeuralManager,
    name_a: &str,
    name_b: &str,
    simulations: usize,
    seed: u64,
    qnet: Option<&QValueNet>,
    top_k: usize,
    exploration_rate: f64,
    random_start: usize,
) -> Result<(GameResult, Vec<MoveData>), Box<dyn Error>> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

    // Both models play with the same deck and tile order
    let deck = create_deck();
    let mut tile_order = Vec::new();

    // Pre-generate tile order
    {
        let mut temp_deck = deck.clone();
        for _ in 0..19 {
            let valid_tiles: Vec<Tile> = temp_deck
                .tiles()
                .iter()
                .cloned()
                .filter(|t| *t != Tile(0, 0, 0))
                .collect();

            if valid_tiles.is_empty() {
                break;
            }

            let tile_idx = rng.random_range(0..valid_tiles.len());
            let tile = valid_tiles[tile_idx];
            tile_order.push(tile);

            if let Some(pos) = temp_deck.tiles().iter().position(|t| *t == tile) {
                temp_deck.tiles_mut()[pos] = Tile(0, 0, 0);
            }
        }
    }

    // Play game for model A
    let mut moves_a = Vec::new();
    let mut plateau_a = create_plateau_empty();
    let mut deck_a = deck.clone();

    // Random start: pre-place tiles randomly (same for both models)
    let mut start_turn = 0;
    if random_start > 0 && random_start < 19 {
        let random_start = random_start.min(tile_order.len());
        for i in 0..random_start {
            let tile = tile_order[i];
            let legal = get_legal_moves(&plateau_a);
            if !legal.is_empty() {
                let pos = legal[rng.random_range(0..legal.len())];
                plateau_a.tiles[pos] = tile;
            }
            // Remove from deck
            if let Some(pos) = deck_a.tiles().iter().position(|t| *t == tile) {
                deck_a.tiles_mut()[pos] = Tile(0, 0, 0);
            }
        }
        start_turn = random_start;
    }

    let policy_a = model_a.policy_net();
    let value_a = model_a.value_net();

    for (turn, &tile) in tile_order.iter().enumerate().skip(start_turn) {
        let plateau_encoded = encode_plateau(&plateau_a.tiles);

        // Remove tile from deck
        if let Some(pos) = deck_a.tiles().iter().position(|t| *t == tile) {
            deck_a.tiles_mut()[pos] = Tile(0, 0, 0);
        }

        let legal_moves = get_legal_moves(&plateau_a);
        if legal_moves.is_empty() {
            break;
        }

        // Exploration: sometimes take a random move to discover new positions
        let chosen_position = if exploration_rate > 0.0 && rng.random::<f64>() < exploration_rate {
            // Random exploration move
            legal_moves[rng.random_range(0..legal_moves.len())]
        } else {
            // Normal MCTS move
            let mut deck_clone = deck_a.clone();
            let mcts_result = if let Some(qvalue_net) = qnet {
                mcts_find_best_position_for_tile_with_qnet(
                    &mut plateau_a,
                    &mut deck_clone,
                    tile,
                    &policy_a,
                    &value_a,
                    qvalue_net,
                    simulations,
                    turn,
                    19,
                    top_k,
                    None,
                )
            } else {
                mcts_find_best_position_for_tile_uct(
                    &mut plateau_a,
                    &mut deck_clone,
                    tile,
                    &policy_a,
                    &value_a,
                    simulations,
                    turn,
                    19,
                    None,
                    None,
                )
            };
            mcts_result.best_position
        };

        moves_a.push(MoveData {
            game_id,
            turn,
            model_name: name_a.to_string(),
            plateau: plateau_encoded,
            tile: (tile.0, tile.1, tile.2),
            position: chosen_position,
            final_score: 0, // Will be filled later
            won: false,     // Will be filled later
        });

        plateau_a.tiles[chosen_position] = tile;
    }

    let score_a = result(&plateau_a);

    // Play game for model B - start from same random position as A
    let mut moves_b = Vec::new();
    let mut plateau_b = plateau_a.clone(); // Start from same position if random_start > 0
    if random_start == 0 {
        plateau_b = create_plateau_empty();
    } else {
        // Reset to the random start position (before A played)
        plateau_b = create_plateau_empty();
        for i in 0..start_turn {
            let tile = tile_order[i];
            let legal = get_legal_moves(&plateau_b);
            if !legal.is_empty() {
                // Use same random position as A
                let mut temp_rng = rand::rngs::StdRng::seed_from_u64(seed);
                for _ in 0..i {
                    let _ = temp_rng.random_range(0..19); // Advance RNG
                }
                let legal_at_i = get_legal_moves(&plateau_b);
                let pos = legal_at_i[temp_rng.random_range(0..legal_at_i.len())];
                plateau_b.tiles[pos] = tile;
            }
        }
    }
    let mut deck_b = deck.clone();
    // Remove pre-placed tiles from deck_b
    for i in 0..start_turn {
        let tile = tile_order[i];
        if let Some(pos) = deck_b.tiles().iter().position(|t| *t == tile) {
            deck_b.tiles_mut()[pos] = Tile(0, 0, 0);
        }
    }

    let policy_b = model_b.policy_net();
    let value_b = model_b.value_net();

    for (turn, &tile) in tile_order.iter().enumerate().skip(start_turn) {
        let plateau_encoded = encode_plateau(&plateau_b.tiles);

        if let Some(pos) = deck_b.tiles().iter().position(|t| *t == tile) {
            deck_b.tiles_mut()[pos] = Tile(0, 0, 0);
        }

        let legal_moves = get_legal_moves(&plateau_b);
        if legal_moves.is_empty() {
            break;
        }

        // Exploration: sometimes take a random move
        let chosen_position = if exploration_rate > 0.0 && rng.random::<f64>() < exploration_rate {
            legal_moves[rng.random_range(0..legal_moves.len())]
        } else {
            let mut deck_clone = deck_b.clone();
            let mcts_result = if let Some(qvalue_net) = qnet {
                mcts_find_best_position_for_tile_with_qnet(
                    &mut plateau_b,
                    &mut deck_clone,
                    tile,
                    &policy_b,
                    &value_b,
                    qvalue_net,
                    simulations,
                    turn,
                    19,
                    top_k,
                    None,
                )
            } else {
                mcts_find_best_position_for_tile_uct(
                    &mut plateau_b,
                    &mut deck_clone,
                    tile,
                    &policy_b,
                    &value_b,
                    simulations,
                    turn,
                    19,
                    None,
                    None,
                )
            };
            mcts_result.best_position
        };

        moves_b.push(MoveData {
            game_id,
            turn,
            model_name: name_b.to_string(),
            plateau: plateau_encoded,
            tile: (tile.0, tile.1, tile.2),
            position: chosen_position,
            final_score: 0,
            won: false,
        });

        plateau_b.tiles[chosen_position] = tile;
    }

    let score_b = result(&plateau_b);

    // Determine winner
    let winner = if score_a > score_b {
        name_a.to_string()
    } else if score_b > score_a {
        name_b.to_string()
    } else {
        "Tie".to_string()
    };

    // Update move data with final scores and win status
    for move_data in &mut moves_a {
        move_data.final_score = score_a;
        move_data.won = score_a > score_b;
    }

    for move_data in &mut moves_b {
        move_data.final_score = score_b;
        move_data.won = score_b > score_a;
    }

    let mut all_moves = moves_a;
    all_moves.extend(moves_b);

    Ok((
        GameResult {
            game_id,
            model_a_score: score_a,
            model_b_score: score_b,
            winner,
            seed,
        },
        all_moves,
    ))
}

fn save_results_csv(
    path: &str,
    results: &[GameResult],
    name_a: &str,
    name_b: &str,
) -> Result<(), Box<dyn Error>> {
    // Create directory if needed
    if let Some(parent) = PathBuf::from(path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(path)?;
    let mut writer = Writer::from_writer(file);

    // Write header
    writer.write_record(&[
        "game_id",
        &format!("{}_score", name_a),
        &format!("{}_score", name_b),
        "winner",
        "seed",
    ])?;

    // Write data
    for result in results {
        writer.write_record(&[
            result.game_id.to_string(),
            result.model_a_score.to_string(),
            result.model_b_score.to_string(),
            result.winner.clone(),
            result.seed.to_string(),
        ])?;
    }

    writer.flush()?;
    Ok(())
}

fn save_training_data_csv(path: &str, moves: &[MoveData]) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = PathBuf::from(path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(path)?;
    let mut writer = Writer::from_writer(file);

    // Write header (compatible with supervised_trainer_csv format)
    let mut header = vec!["game_id".to_string(), "turn".to_string(), "player_type".to_string()];
    for i in 0..19 {
        header.push(format!("plateau_{}", i));
    }
    header.extend(vec![
        "tile_0".to_string(),
        "tile_1".to_string(),
        "tile_2".to_string(),
        "position".to_string(),
        "final_score".to_string(),
        "human_won".to_string(), // Using this field to indicate if this model won
    ]);
    writer.write_record(&header)?;

    // Write data
    for move_data in moves {
        let mut record = vec![
            move_data.game_id.to_string(),
            move_data.turn.to_string(),
            move_data.model_name.clone(),
        ];

        for &p in &move_data.plateau {
            record.push(p.to_string());
        }

        record.extend(vec![
            move_data.tile.0.to_string(),
            move_data.tile.1.to_string(),
            move_data.tile.2.to_string(),
            move_data.position.to_string(),
            move_data.final_score.to_string(),
            if move_data.won { "1" } else { "0" }.to_string(),
        ]);

        writer.write_record(&record)?;
    }

    writer.flush()?;
    Ok(())
}
