//! Model Validation Binary
//!
//! Compares a candidate model against the production model to validate
//! improvements before deployment. Uses statistical testing to ensure
//! the improvement is significant.

use clap::Parser;
use flexi_logger::Logger;
use rand::prelude::*;
use std::error::Error;

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::{mcts_find_best_position_for_tile_uct, mcts_find_best_position_for_tile_with_qnet};
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::qvalue_net::QValueNet;
use take_it_easy::neural::{NeuralConfig, NeuralManager, QNetManager};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(
    name = "validate-new-model",
    about = "Validate candidate model against production before deployment"
)]
struct Args {
    /// Directory containing candidate model weights
    #[arg(long, default_value = "model_weights_candidate")]
    candidate: String,

    /// Directory containing production model weights
    #[arg(long, default_value = "model_weights")]
    production: String,

    /// Number of games to play for validation
    #[arg(long, default_value_t = 200)]
    games: usize,

    /// Number of MCTS simulations per move
    #[arg(long, default_value_t = 150)]
    simulations: usize,

    /// Minimum improvement threshold in points to approve
    #[arg(long, default_value_t = 3.0)]
    threshold: f64,

    /// Neural network architecture
    #[arg(long, default_value = "CNN")]
    nn_architecture: String,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Confidence level for t-test (0.95 = 95%)
    #[arg(long, default_value_t = 0.95)]
    confidence: f64,

    /// Enable Q-Net Hybrid MCTS (like production)
    #[arg(long, default_value_t = true)]
    hybrid_mcts: bool,

    /// Path to Q-Net weights
    #[arg(long, default_value = "model_weights/qvalue_net.params")]
    qnet_path: String,

    /// Top-K positions for Q-Net pruning
    #[arg(long, default_value_t = 6)]
    top_k: usize,
}

#[derive(Debug)]
struct ValidationResult {
    model_name: String,
    games_played: usize,
    mean_score: f64,
    std_dev: f64,
    min_score: i32,
    max_score: i32,
    scores: Vec<i32>,
}

fn main() -> Result<(), Box<dyn Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    let args = Args::parse();

    log::info!("🔬 Model Validation");
    log::info!("Candidate: {}", args.candidate);
    log::info!("Production: {}", args.production);
    log::info!("Games: {}", args.games);
    log::info!("Simulations: {}", args.simulations);
    log::info!("Threshold: {} points", args.threshold);

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

    // Load production model
    log::info!("\n📦 Loading production model...");
    let prod_config = NeuralConfig {
        input_dim: nn_arch.input_dim(),
        nn_architecture: nn_arch,
        model_path: args.production.clone(),
        ..Default::default()
    };
    let prod_manager = NeuralManager::with_config(prod_config)?;
    log::info!("✅ Production model loaded");

    // Load candidate model
    log::info!("📦 Loading candidate model...");
    let cand_config = NeuralConfig {
        input_dim: nn_arch.input_dim(),
        nn_architecture: nn_arch,
        model_path: args.candidate.clone(),
        ..Default::default()
    };
    let cand_manager = NeuralManager::with_config(cand_config)?;
    log::info!("✅ Candidate model loaded");

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

    // Run validation games
    log::info!("\n🎮 Running validation games...");

    let prod_result = run_validation_games(
        "Production",
        &prod_manager,
        args.games,
        args.simulations,
        args.seed,
        qnet.as_ref(),
        args.top_k,
    )?;

    let cand_result = run_validation_games(
        "Candidate",
        &cand_manager,
        args.games,
        args.simulations,
        args.seed + 1000, // Different seed but reproducible
        qnet.as_ref(),
        args.top_k,
    )?;

    // Calculate improvement
    let improvement = cand_result.mean_score - prod_result.mean_score;

    // Perform t-test
    let (t_stat, p_value) = welch_t_test(&prod_result.scores, &cand_result.scores);

    // Determine if approved
    let significant = p_value < (1.0 - args.confidence);
    let approved = improvement >= args.threshold && significant;

    // Print results
    log::info!("\n═══════════════════════════════════════════════════════════════");
    log::info!("                     VALIDATION RESULTS");
    log::info!("═══════════════════════════════════════════════════════════════");
    log::info!("");
    log::info!(
        "  Production:  {:.2} ± {:.2} pts  (range: [{}, {}])",
        prod_result.mean_score,
        prod_result.std_dev,
        prod_result.min_score,
        prod_result.max_score
    );
    log::info!(
        "  Candidate:   {:.2} ± {:.2} pts  (range: [{}, {}])",
        cand_result.mean_score,
        cand_result.std_dev,
        cand_result.min_score,
        cand_result.max_score
    );
    log::info!("");
    log::info!(
        "  Improvement: {:+.2} pts",
        improvement
    );
    log::info!("  T-statistic: {:.4}", t_stat);
    log::info!("  P-value:     {:.6}", p_value);
    log::info!("  Significant: {} (at {:.0}% confidence)", significant, args.confidence * 100.0);
    log::info!("");

    if approved {
        log::info!("  Status: ✅ APPROVED");
        log::info!("");
        log::info!("═══════════════════════════════════════════════════════════════");
        log::info!("");
        log::info!("Deploy command:");
        log::info!("  cp -r {}/* {}/", args.candidate, args.production);
        log::info!("");
    } else {
        log::info!("  Status: ❌ REJECTED");
        log::info!("");
        if !significant {
            log::info!("  Reason: Improvement not statistically significant");
        } else if improvement < args.threshold {
            log::info!(
                "  Reason: Improvement ({:.2}) below threshold ({:.2})",
                improvement,
                args.threshold
            );
        }
        log::info!("");
        log::info!("═══════════════════════════════════════════════════════════════");
    }

    // Exit with appropriate code
    if approved {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

fn run_validation_games(
    name: &str,
    manager: &NeuralManager,
    num_games: usize,
    simulations: usize,
    seed: u64,
    qnet: Option<&QValueNet>,
    top_k: usize,
) -> Result<ValidationResult, Box<dyn Error>> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut scores = Vec::with_capacity(num_games);

    let policy_net = manager.policy_net();
    let value_net = manager.value_net();

    for game_idx in 0..num_games {
        if (game_idx + 1) % 50 == 0 || game_idx == 0 {
            log::info!("  {} - Game {}/{}", name, game_idx + 1, num_games);
        }

        let score = play_single_game(&policy_net, &value_net, simulations, &mut rng, qnet, top_k)?;
        scores.push(score);
    }

    let mean = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    let variance = scores.iter().map(|&s| (s as f64 - mean).powi(2)).sum::<f64>() / scores.len() as f64;
    let std_dev = variance.sqrt();

    Ok(ValidationResult {
        model_name: name.to_string(),
        games_played: num_games,
        mean_score: mean,
        std_dev,
        min_score: *scores.iter().min().unwrap_or(&0),
        max_score: *scores.iter().max().unwrap_or(&0),
        scores,
    })
}

fn play_single_game<R: Rng>(
    policy_net: &take_it_easy::neural::policy_value_net::PolicyNet,
    value_net: &take_it_easy::neural::policy_value_net::ValueNet,
    simulations: usize,
    rng: &mut R,
    qnet: Option<&QValueNet>,
    top_k: usize,
) -> Result<i32, Box<dyn Error>> {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();
    let total_turns = 19;

    for turn in 0..total_turns {
        // Get valid tiles
        let valid_tiles: Vec<Tile> = deck
            .tiles()
            .iter()
            .cloned()
            .filter(|t| *t != Tile(0, 0, 0))
            .collect();

        if valid_tiles.is_empty() {
            break;
        }

        // Draw random tile
        let tile_idx = rng.random_range(0..valid_tiles.len());
        let current_tile = valid_tiles[tile_idx];

        // Remove from deck
        if let Some(pos) = deck.tiles().iter().position(|t| *t == current_tile) {
            deck.tiles_mut()[pos] = Tile(0, 0, 0);
        }

        // Get legal moves
        let legal_moves = get_legal_moves(&plateau);
        if legal_moves.is_empty() {
            break;
        }

        // Use MCTS to find best position (with Q-net if available)
        let mut deck_clone = deck.clone();
        let mcts_result = if let Some(qvalue_net) = qnet {
            mcts_find_best_position_for_tile_with_qnet(
                &mut plateau,
                &mut deck_clone,
                current_tile,
                policy_net,
                value_net,
                qvalue_net,
                simulations,
                turn,
                total_turns,
                top_k,
                None,
            )
        } else {
            mcts_find_best_position_for_tile_uct(
                &mut plateau,
                &mut deck_clone,
                current_tile,
                policy_net,
                value_net,
                simulations,
                turn,
                total_turns,
                None,
                None,
            )
        };

        // Place the tile
        plateau.tiles[mcts_result.best_position] = current_tile;
    }

    Ok(result(&plateau))
}

/// Welch's t-test for unequal variances
fn welch_t_test(sample1: &[i32], sample2: &[i32]) -> (f64, f64) {
    let n1 = sample1.len() as f64;
    let n2 = sample2.len() as f64;

    let mean1: f64 = sample1.iter().sum::<i32>() as f64 / n1;
    let mean2: f64 = sample2.iter().sum::<i32>() as f64 / n2;

    let var1: f64 = sample1.iter().map(|&x| (x as f64 - mean1).powi(2)).sum::<f64>() / (n1 - 1.0);
    let var2: f64 = sample2.iter().map(|&x| (x as f64 - mean2).powi(2)).sum::<f64>() / (n2 - 1.0);

    let se = ((var1 / n1) + (var2 / n2)).sqrt();

    if se == 0.0 {
        return (0.0, 1.0);
    }

    let t = (mean2 - mean1) / se;

    // Welch-Satterthwaite degrees of freedom
    let v1 = var1 / n1;
    let v2 = var2 / n2;
    let df = (v1 + v2).powi(2) / (v1.powi(2) / (n1 - 1.0) + v2.powi(2) / (n2 - 1.0));

    // Approximate p-value using normal distribution for large samples
    // For more accurate results with small samples, use a proper t-distribution
    let p_value = if df > 30.0 {
        // Use normal approximation for large df
        2.0 * (1.0 - normal_cdf(t.abs()))
    } else {
        // Use t-distribution approximation
        2.0 * (1.0 - student_t_cdf(t.abs(), df))
    };

    (t, p_value)
}

/// Standard normal CDF approximation
fn normal_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Error function approximation (Abramowitz and Stegun)
fn erf(x: f64) -> f64 {
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

/// Student's t-distribution CDF approximation
fn student_t_cdf(t: f64, df: f64) -> f64 {
    let x = df / (df + t * t);
    let beta = incomplete_beta(df / 2.0, 0.5, x);
    1.0 - 0.5 * beta
}

/// Incomplete beta function approximation (for t-distribution)
fn incomplete_beta(a: f64, b: f64, x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    if x == 1.0 {
        return 1.0;
    }

    // Use continued fraction approximation
    let bt = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();

    if x < (a + 1.0) / (a + b + 2.0) {
        bt * beta_cf(a, b, x) / a
    } else {
        1.0 - bt * beta_cf(b, a, 1.0 - x) / b
    }
}

/// Continued fraction for incomplete beta
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    let max_iter = 100;
    let eps = 1e-10;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;

    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < 1e-30 {
        d = 1e-30;
    }
    d = 1.0 / d;
    let mut h = d;

    for m in 1..=max_iter {
        let m = m as f64;
        let m2 = 2.0 * m;

        // Even step
        let mut aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < 1e-30 {
            d = 1e-30;
        }
        c = 1.0 + aa / c;
        if c.abs() < 1e-30 {
            c = 1e-30;
        }
        d = 1.0 / d;
        h *= d * c;

        // Odd step
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < 1e-30 {
            d = 1e-30;
        }
        c = 1.0 + aa / c;
        if c.abs() < 1e-30 {
            c = 1e-30;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;

        if (del - 1.0).abs() < eps {
            break;
        }
    }

    h
}

/// Log gamma function (Lanczos approximation)
fn ln_gamma(x: f64) -> f64 {
    let coeffs = [
        76.18009172947146,
        -86.50532032941677,
        24.01409824083091,
        -1.231739572450155,
        0.1208650973866179e-2,
        -0.5395239384953e-5,
    ];

    let y = x;
    let tmp = x + 5.5;
    let tmp = tmp - (x + 0.5) * tmp.ln();

    let mut ser = 1.000000000190015;
    for (j, &coeff) in coeffs.iter().enumerate() {
        ser += coeff / (y + 1.0 + j as f64);
    }

    -tmp + (2.5066282746310005 * ser / x).ln()
}
