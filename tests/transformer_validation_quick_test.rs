//! Quick validation test comparing Transformer vs baseline MCTS (reduced parameters for fast execution)

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::game_state::GameState;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::plateau_is_full::is_plateau_full;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::neural::manager::{NeuralConfig, NeuralManager};
use take_it_easy::neural::policy_value_net::{PolicyNet, ValueNet};
use take_it_easy::neural::transformer::hybrid_policy::{hybrid_policy, HybridConfig};
use take_it_easy::neural::transformer::mcts_integration::ParallelTransformerMCTS;
use take_it_easy::neural::transformer::{TransformerConfig, TransformerModel};
use take_it_easy::scoring::scoring::result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tch::{nn, Device};

/// Performance metrics
#[derive(Debug, Clone)]
struct PerformanceMetrics {
    scores: Vec<i32>,
    mean_score: f64,
    std_dev: f64,
    median_score: f64,
    min_score: i32,
    max_score: i32,
    completed_lines_avg: f64,
}

impl PerformanceMetrics {
    fn from_scores(scores: Vec<i32>) -> Self {
        let mean = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
        let variance = scores
            .iter()
            .map(|&s| {
                let diff = s as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / scores.len() as f64;
        let std_dev = variance.sqrt();

        let mut sorted = scores.clone();
        sorted.sort();
        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) as f64 / 2.0
        } else {
            sorted[sorted.len() / 2] as f64
        };

        Self {
            mean_score: mean,
            std_dev,
            median_score: median,
            min_score: *sorted.first().unwrap(),
            max_score: *sorted.last().unwrap(),
            scores,
            completed_lines_avg: 0.0,
        }
    }

    fn print_report(&self, name: &str) {
        println!("\n╔════════════════════════════════════════════════╗");
        println!("║ {} Performance Report", name);
        println!("╠════════════════════════════════════════════════╣");
        println!("║ Mean Score:     {:>8.2}                      ║", self.mean_score);
        println!("║ Std Dev:        {:>8.2}                      ║", self.std_dev);
        println!("║ Median:         {:>8.2}                      ║", self.median_score);
        println!("║ Range:          {:>4} - {:>4}                   ║", self.min_score, self.max_score);
        println!("║ Games:          {:>8}                        ║", self.scores.len());
        println!("║ Lines/Game:     {:>8.2}                      ║", self.completed_lines_avg);
        println!("╚════════════════════════════════════════════════╝");
    }
}

fn count_completed_lines(plateau: &Plateau) -> usize {
    let mut count = 0;

    let columns = vec![
        vec![0, 1, 2],
        vec![3, 4, 5, 6],
        vec![7, 8, 9, 10, 11],
        vec![12, 13, 14, 15],
        vec![16, 17, 18],
    ];

    let diagonals = vec![
        vec![0, 3, 7],
        vec![1, 4, 8, 12],
        vec![2, 5, 9, 13, 16],
        vec![6, 10, 14, 17],
        vec![11, 15, 18],
    ];

    for line in columns.iter().chain(diagonals.iter()) {
        if is_line_complete(plateau, line) {
            count += 1;
        }
    }

    count
}

fn is_line_complete(plateau: &Plateau, positions: &[usize]) -> bool {
    if positions.is_empty() {
        return false;
    }
    if positions.iter().any(|&pos| plateau.tiles[pos] == Tile(0, 0, 0)) {
        return false;
    }
    let first_tile = plateau.tiles[positions[0]];
    for band in [first_tile.0, first_tile.1, first_tile.2] {
        if band == 0 {
            continue;
        }
        if positions.iter().all(|&pos| {
            let tile = plateau.tiles[pos];
            tile.0 == band || tile.1 == band || tile.2 == band
        }) {
            return true;
        }
    }
    false
}

fn play_baseline_game(
    rng: &mut StdRng,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
) -> (i32, usize) {
    let mut deck = create_deck();
    let mut plateau = create_plateau_empty();
    let total_turns = 19;
    let mut current_turn = 0;

    while !is_plateau_full(&plateau) && current_turn < total_turns {
        let available_tiles = get_available_tiles(&deck);

        if available_tiles.is_empty() {
            break;
        }

        let chosen_tile = available_tiles[rng.random_range(0..available_tiles.len())];

        let mcts_result = mcts_find_best_position_for_tile_with_nn(
            &mut plateau,
            &mut deck,
            chosen_tile,
            policy_net,
            value_net,
            num_simulations,
            current_turn,
            total_turns,
        );

        plateau.tiles[mcts_result.best_position] = chosen_tile;
        deck = replace_tile_in_deck(&deck, &chosen_tile);
        current_turn += 1;
    }

    let score = result(&plateau);
    let lines = count_completed_lines(&plateau);
    (score, lines)
}

fn play_transformer_game(
    rng: &mut StdRng,
    evaluator: &ParallelTransformerMCTS,
) -> (i32, usize) {
    let mut deck = create_deck();
    let mut plateau = create_plateau_empty();

    while !is_plateau_full(&plateau) {
        let available_tiles = get_available_tiles(&deck);

        if available_tiles.is_empty() {
            break;
        }

        let chosen_tile = available_tiles[rng.random_range(0..available_tiles.len())];
        let legal_moves = get_legal_moves(plateau.clone());

        if legal_moves.is_empty() {
            break;
        }

        let game_state = GameState {
            plateau: plateau.clone(),
            deck: deck.clone(),
        };

        let (policy, _) = evaluator
            .parallel_predict_batch(&[&game_state])
            .ok()
            .and_then(|mut preds| preds.pop())
            .unwrap_or_else(|| (vec![1.0 / legal_moves.len() as f32; 19], 0.0));

        let best_position = legal_moves
            .iter()
            .max_by(|&&a, &&b| {
                policy
                    .get(a)
                    .unwrap_or(&0.0)
                    .partial_cmp(policy.get(b).unwrap_or(&0.0))
                    .unwrap()
            })
            .copied()
            .unwrap_or(legal_moves[0]);

        plateau.tiles[best_position] = chosen_tile;
        deck = replace_tile_in_deck(&deck, &chosen_tile);
    }

    let score = result(&plateau);
    let lines = count_completed_lines(&plateau);
    (score, lines)
}

fn play_hybrid_game(
    rng: &mut StdRng,
    evaluator: &ParallelTransformerMCTS,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    hybrid_config: &HybridConfig,
) -> (i32, usize) {
    let mut deck = create_deck();
    let mut plateau = create_plateau_empty();
    let total_turns = 19;
    let mut current_turn = 0;

    while !is_plateau_full(&plateau) && current_turn < total_turns {
        let available_tiles = get_available_tiles(&deck);

        if available_tiles.is_empty() {
            break;
        }

        let chosen_tile = available_tiles[rng.random_range(0..available_tiles.len())];

        let mcts_result = mcts_find_best_position_for_tile_with_nn(
            &mut plateau,
            &mut deck,
            chosen_tile,
            policy_net,
            value_net,
            num_simulations,
            current_turn,
            total_turns,
        );

        let game_state = GameState {
            plateau: plateau.clone(),
            deck: deck.clone(),
        };
        let (transformer_policy, _) = evaluator
            .parallel_predict_batch(&[&game_state])
            .ok()
            .and_then(|mut preds| preds.pop())
            .unwrap_or_else(|| (vec![1.0 / 19.0; 19], 0.0));

        let mcts_policy: Vec<f32> = (0..19)
            .map(|i| {
                mcts_result
                    .policy_distribution_boosted
                    .double_value(&[i])
                    as f32
            })
            .collect();

        let hybrid_policy_vec = hybrid_policy(&transformer_policy, &mcts_policy, hybrid_config);

        let legal_moves = get_legal_moves(plateau.clone());
        let best_position = legal_moves
            .iter()
            .max_by(|&&a, &&b| {
                hybrid_policy_vec[a]
                    .partial_cmp(&hybrid_policy_vec[b])
                    .unwrap()
            })
            .copied()
            .unwrap_or(mcts_result.best_position);

        plateau.tiles[best_position] = chosen_tile;
        deck = replace_tile_in_deck(&deck, &chosen_tile);
        current_turn += 1;
    }

    let score = result(&plateau);
    let lines = count_completed_lines(&plateau);
    (score, lines)
}

#[test]
#[ignore]
fn test_quick_validation() {
    println!("\n🎯 Quick Validation Test (5 games, 50 simulations)");
    println!("===================================================\n");

    let num_games = 5;
    let num_simulations = 50;

    println!("📦 Loading baseline MCTS model...");
    let neural_config = NeuralConfig {
        model_path: "model_weights".to_string(),
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config).expect("Failed to load baseline");
    let policy_net = manager.policy_net();
    let value_net = manager.value_net();

    println!("🤖 Initializing Transformer model...");
    let vs = nn::VarStore::new(Device::Cpu);
    let config = TransformerConfig::default();
    let model = TransformerModel::new(config, &vs.root()).expect("Failed to create transformer");
    let evaluator = ParallelTransformerMCTS::new(model);

    let hybrid_config = HybridConfig {
        alpha: 0.5,
        dynamic_alpha: false,
    };

    let mut baseline_scores = Vec::new();
    let mut baseline_lines = Vec::new();
    let mut transformer_scores = Vec::new();
    let mut transformer_lines = Vec::new();
    let mut hybrid_scores = Vec::new();
    let mut hybrid_lines = Vec::new();

    println!("\n🎮 Running {} games per approach...\n", num_games);

    for i in 0..num_games {
        let seed = 42 + i as u64;

        print!("Game {:>2}...", i + 1);

        let mut rng = StdRng::seed_from_u64(seed);
        let (score, lines) = play_baseline_game(&mut rng, policy_net, value_net, num_simulations);
        baseline_scores.push(score);
        baseline_lines.push(lines);

        let mut rng = StdRng::seed_from_u64(seed);
        let (score, lines) = play_transformer_game(&mut rng, &evaluator);
        transformer_scores.push(score);
        transformer_lines.push(lines);

        let mut rng = StdRng::seed_from_u64(seed);
        let (score, lines) = play_hybrid_game(
            &mut rng,
            &evaluator,
            policy_net,
            value_net,
            num_simulations,
            &hybrid_config,
        );
        hybrid_scores.push(score);
        hybrid_lines.push(lines);

        println!(
            " Baseline={:>3} ({} lines) | Transformer={:>3} ({} lines) | Hybrid={:>3} ({} lines)",
            baseline_scores[i],
            baseline_lines[i],
            transformer_scores[i],
            transformer_lines[i],
            hybrid_scores[i],
            hybrid_lines[i]
        );
    }

    let mut baseline_metrics = PerformanceMetrics::from_scores(baseline_scores);
    baseline_metrics.completed_lines_avg =
        baseline_lines.iter().sum::<usize>() as f64 / baseline_lines.len() as f64;

    let mut transformer_metrics = PerformanceMetrics::from_scores(transformer_scores);
    transformer_metrics.completed_lines_avg =
        transformer_lines.iter().sum::<usize>() as f64 / transformer_lines.len() as f64;

    let mut hybrid_metrics = PerformanceMetrics::from_scores(hybrid_scores);
    hybrid_metrics.completed_lines_avg =
        hybrid_lines.iter().sum::<usize>() as f64 / hybrid_lines.len() as f64;

    baseline_metrics.print_report("🔵 Baseline MCTS");
    transformer_metrics.print_report("🟢 Transformer");
    hybrid_metrics.print_report("🟣 Hybrid (α=0.5)");

    println!("\n📊 Statistical Comparisons");
    println!("══════════════════════════");
    println!(
        "Transformer vs Baseline: {:.1}% ({:+.2} points)",
        (transformer_metrics.mean_score / baseline_metrics.mean_score - 1.0) * 100.0,
        transformer_metrics.mean_score - baseline_metrics.mean_score
    );
    println!(
        "Hybrid vs Baseline:      {:.1}% ({:+.2} points)",
        (hybrid_metrics.mean_score / baseline_metrics.mean_score - 1.0) * 100.0,
        hybrid_metrics.mean_score - baseline_metrics.mean_score
    );
    println!(
        "Hybrid vs Transformer:   {:.1}% ({:+.2} points)",
        (hybrid_metrics.mean_score / transformer_metrics.mean_score - 1.0) * 100.0,
        hybrid_metrics.mean_score - transformer_metrics.mean_score
    );

    println!("\n✅ Quick validation completed!");
    println!("\nNote: Transformer is untrained (random initialization)");
    println!("Expected behavior:");
    println!("  - Baseline MCTS should score ~130-140 with boost heuristics");
    println!("  - Untrained Transformer should score poorly (~20-40)");
    println!("  - Hybrid should be between the two");
    println!("\nTo improve Transformer: Train on saved MCTS data (policy_raw, policy_boosted)");
}
