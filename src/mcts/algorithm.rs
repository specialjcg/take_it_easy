use std::collections::HashMap;
use tch::IndexOp;
use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::remove_tile_from_deck::replace_tile_in_deck;
use crate::game::simulate_game::simulate_games;
use crate::game::tile::Tile;
use crate::mcts::mcts_result::MCTSResult;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::tensor_conversion::convert_plateau_to_tensor;
use crate::scoring::scoring::result;
use crate::strategy::position_evaluation::enhanced_position_evaluation;
use crate::utils::random_index::random_index;

pub fn mcts_find_best_position_for_tile_with_nn(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
) -> MCTSResult {
    let legal_moves = get_legal_moves(plateau.clone());
    if legal_moves.is_empty() {
        return MCTSResult {
            best_position: 0,
            board_tensor: convert_plateau_to_tensor(
                plateau,
                &chosen_tile,
                deck,
                current_turn,
                total_turns,
            ),
            subscore: 0.0,
        };
    }

    let board_tensor =
        convert_plateau_to_tensor(plateau, &chosen_tile, deck, current_turn, total_turns);
    let policy_logits = policy_net.forward(&board_tensor, false);
    let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp(); // Log-softmax improves numerical stability

    let mut visit_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_scores: HashMap<usize, f64> = HashMap::new();
    let mut ucb_scores: HashMap<usize, f64> = HashMap::new();
    let mut total_visits: i32 = 0;

    for &position in &legal_moves {
        visit_counts.insert(position, 0);
        total_scores.insert(position, 0.0);
        ucb_scores.insert(position, f64::NEG_INFINITY);
    }

    let c_puct = if current_turn < 5 {
        4.2 // Plus d'exploitation en début de partie (positions critiques)
    } else if current_turn > 15 {
        3.0 // Plus d'exploration en fin de partie (adaptation)
    } else {
        3.8 // Équilibre pour le milieu de partie
    };

    // **Compute ValueNet scores for all legal moves**
    let mut value_estimates = HashMap::new();
    let mut min_value = f64::INFINITY;
    let mut max_value = f64::NEG_INFINITY;

    for &position in &legal_moves {
        let mut temp_plateau = plateau.clone();
        let mut temp_deck = deck.clone();

        temp_plateau.tiles[position] = chosen_tile;
        temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);
        let board_tensor_temp = convert_plateau_to_tensor(
            &temp_plateau,
            &chosen_tile,
            &temp_deck,
            current_turn,
            total_turns,
        );

        let pred_value = value_net
            .forward(&board_tensor_temp, false)
            .double_value(&[]);
        let pred_value = pred_value.clamp(-1.0, 1.0);

        // Track min and max for dynamic pruning
        min_value = min_value.min(pred_value);
        max_value = max_value.max(pred_value);

        value_estimates.insert(position, pred_value);
    }

    // **Dynamic Pruning Strategy**
    let value_threshold = if current_turn < 8 {
        min_value + (max_value - min_value) * 0.1 // Garder plus de candidats en début
    } else {
        min_value + (max_value - min_value) * 0.15 // Pruning moins agressif
    };

    for _ in 0..num_simulations {
        let mut moves_with_prior: Vec<_> = legal_moves
            .iter()
            .filter(|&&pos| value_estimates[&pos] >= value_threshold) // Prune weak moves
            .map(|&pos| (pos, policy.i((0, pos as i64)).double_value(&[])))
            .collect();

        moves_with_prior.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_k = usize::min(
            moves_with_prior.len(),
            ((total_visits as f64).sqrt() as usize).max(5),
        );

        let subset_moves: Vec<usize> = moves_with_prior
            .iter()
            .take(top_k)
            .map(|&(pos, _)| pos)
            .collect();

        for &position in &subset_moves {
            let mut temp_plateau = plateau.clone();
            let mut temp_deck = deck.clone();

            temp_plateau.tiles[position] = chosen_tile;
            temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

            let value_estimate = *value_estimates.get(&position).unwrap_or(&0.0);

            // **Improved Adaptive Rollout Strategy**
            let rollout_count = match value_estimate {
                x if x > 8.0 => 2, // Very strong move -> minimal rollouts
                x if x > 6.0 => 4, // Strong move -> fewer rollouts
                x if x > 4.0 => 6, // Decent move -> moderate rollouts
                _ => 8,            // Uncertain move -> more rollouts
            };

            let mut total_simulated_score = 0.0;

            for _ in 0..rollout_count {
                let mut lookahead_plateau = temp_plateau.clone();
                let mut lookahead_deck = temp_deck.clone();

                // 🔮 Étape 1.1 — Tirer une tuile hypothétique (T2)
                if lookahead_deck.tiles.is_empty() {
                    continue;
                }
                let tile2_index = random_index(lookahead_deck.tiles.len());
                let tile2 = lookahead_deck.tiles[tile2_index];

                // 🔍 Étape 1.2 — Simuler tous les placements possibles de cette tuile
                let second_moves = get_legal_moves(lookahead_plateau.clone());

                let mut best_score_for_tile2: f64 = 0.0;

                for &pos2 in &second_moves {
                    let mut plateau2 = lookahead_plateau.clone();
                    let mut deck2 = lookahead_deck.clone();

                    plateau2.tiles[pos2] = tile2;
                    deck2 = replace_tile_in_deck(&deck2, &tile2);

                    let score = simulate_games(plateau2.clone(), deck2.clone()) as f64;
                    best_score_for_tile2 = best_score_for_tile2.max(score);
                }

                total_simulated_score += best_score_for_tile2;
            }

            let simulated_score = total_simulated_score / rollout_count as f64;

            let visits = visit_counts.entry(position).or_insert(0);
            *visits += 1;
            total_visits += 1;

            let total_score = total_scores.entry(position).or_insert(0.0);
            *total_score += simulated_score as f64;

            let exploration_param = c_puct * (total_visits as f64).ln() / (1.0 + *visits as f64);
            let prior_prob = policy.i((0, position as i64)).double_value(&[]);
            let average_score = *total_score / (*visits as f64);
            // 🧪 Reduce weight of rollout average
            let enhanced_eval =
                enhanced_position_evaluation(&temp_plateau, position, &chosen_tile, current_turn);

            // Intégrer dans le calcul UCB
            let mut ucb_score = (average_score * 0.5)
                + exploration_param * (prior_prob.sqrt())
                + 0.25 * value_estimate.clamp(0.0, 2.0)
                + 0.1 * enhanced_eval; // Nouveau facteur d'évaluation

            // 🔥 Explicit Priority Logic HERE 🔥
            // 1️⃣ Ajoute cette fonction en dehors de ta mcts_find_best_position_for_tile_with_nn

            // 2️⃣ Intègre ceci dans ta boucle ucb_scores, juste après le boost fixe

            if chosen_tile.0 == 9 && [7, 8, 9, 10, 11].contains(&position) {
                ucb_score += 10000.0; // double boost
            } else if chosen_tile.0 == 5 && [3, 4, 5, 6, 12, 13, 14, 15].contains(&position) {
                ucb_score += 8000.0;
            } else if chosen_tile.0 == 1 && [0, 1, 2, 16, 17, 18].contains(&position) {
                ucb_score += 6000.0;
            }

            // 🔥 Alignment Priority Logic 🔥

            ucb_scores.insert(position, ucb_score);
        }
    }

    // Select the move with the highest UCB score
    let best_position = legal_moves
        .into_iter()
        .max_by(|&a, &b| {
            ucb_scores
                .get(&a)
                .unwrap_or(&f64::NEG_INFINITY)
                .partial_cmp(ucb_scores.get(&b).unwrap_or(&f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);

    // **NEW: Simulate the Rest of the Game to Get Final Score**
    let mut final_plateau = plateau.clone();
    let mut final_deck = deck.clone();
    final_plateau.tiles[best_position] = chosen_tile;
    final_deck = replace_tile_in_deck(&final_deck, &chosen_tile);

    while !is_plateau_full(&final_plateau) {
        let tile_index = random_index(final_deck.tiles.len());
        let random_tile = final_deck.tiles[tile_index];

        let available_moves = get_legal_moves(final_plateau.clone());
        if available_moves.is_empty() {
            break;
        }

        let random_position = available_moves[random_index(available_moves.len())];
        final_plateau.tiles[random_position] = random_tile;
        final_deck = replace_tile_in_deck(&final_deck, &random_tile);
    }

    let final_score = result(&final_plateau); // Get actual game score

    log::info!("🤖 Pos:{} Score:{}", best_position, final_score as i32);

    MCTSResult {
        best_position,
        board_tensor,
        subscore: final_score as f64, // Store real final score, not UCB score
    }
}