//! Test: MCTS sélectionne-t-il des positions NON-uniformes?
//!
//! Joue 50 parties avec MCTS et compte combien de fois chaque position
//! est sélectionnée. Si MCTS fonctionne bien, certaines positions
//! devraient être préférées (centre, etc.).

use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::{NeuralConfig, NeuralManager};
use take_it_easy::scoring::scoring::result;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Test: MCTS sélectionne-t-il des positions NON-uniformes?\n");

    // Initialize network (même si uniforme)
    let neural_config = NeuralConfig {
        input_dim: (9, 5, 5),
        nn_architecture: NNArchitecture::Cnn,
        ..Default::default()
    };
    let manager = NeuralManager::with_config(neural_config)?;

    let mut rng = StdRng::seed_from_u64(42);
    let num_games = 50;
    let turns_per_game = 19;
    let mcts_sims = 150;

    let mut position_counts: HashMap<usize, usize> = HashMap::new();
    let mut total_selections = 0;
    let mut scores = Vec::new();

    println!("🎮 Jouant {} parties avec MCTS (150 sims)...\n", num_games);

    for game_idx in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();

        for turn in 0..turns_per_game {
            let available = get_available_tiles(&deck);
            if available.is_empty() {
                break;
            }

            let chosen_tile = *available.choose(&mut rng).unwrap();
            deck = replace_tile_in_deck(&deck, &chosen_tile);

            let mcts_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                manager.policy_net(),
                manager.value_net(),
                mcts_sims,
                turn,
                turns_per_game,
                None, // Default hyperparameters
            );

            // Compter la position sélectionnée
            *position_counts.entry(mcts_result.best_position).or_insert(0) += 1;
            total_selections += 1;

            plateau.tiles[mcts_result.best_position] = chosen_tile;
        }

        let final_score = result(&plateau);
        scores.push(final_score);

        if (game_idx + 1) % 10 == 0 {
            println!("  Partie {}/{} complétée", game_idx + 1, num_games);
        }
    }

    println!("\n📊 Distribution des positions sélectionnées par MCTS:\n");

    // Calculer statistiques
    let expected_uniform = total_selections as f64 / 19.0;
    let mut sorted_positions: Vec<_> = position_counts.iter().collect();
    sorted_positions.sort_by_key(|(pos, _)| **pos);

    let mut chi_squared = 0.0;

    for (pos, count) in &sorted_positions {
        let count_val = **count as f64;
        let percentage = (count_val / total_selections as f64) * 100.0;
        let expected_pct = 100.0 / 19.0;
        let diff = count_val - expected_uniform;
        chi_squared += (diff * diff) / expected_uniform;

        let marker = if percentage > expected_pct * 1.5 {
            " ★ PRÉFÉRÉE"
        } else if percentage < expected_pct * 0.5 {
            " ⚠ ÉVITÉE"
        } else {
            ""
        };

        println!("  Position {:2}: {:4} fois ({:5.2}% | attendu: {:.2}%){}",
                 pos, count, percentage, expected_pct, marker);
    }

    // Vérifier positions manquantes
    for pos in 0..19 {
        if !position_counts.contains_key(&pos) {
            println!("  Position {:2}:    0 fois ( 0.00% | attendu: 5.26%) ⚠ JAMAIS", pos);
        }
    }

    println!("\n📈 Statistiques:");
    println!("  Total sélections: {}", total_selections);
    println!("  Attendu uniforme: {:.1} par position", expected_uniform);
    println!("  Chi-carré: {:.2}", chi_squared);

    if chi_squared < 10.0 {
        println!("  → Distribution QUASI-UNIFORME (chi² < 10)");
        println!("\n❌ PROBLÈME IDENTIFIÉ:");
        println!("  MCTS sélectionne uniformément → Données d'entraînement uniformes");
        println!("  → Policy network ne peut pas apprendre (pas de signal)");
    } else if chi_squared < 50.0 {
        println!("  → Distribution LÉGÈREMENT biaisée (chi² = {:.2})", chi_squared);
        println!("\n⚠️ Signal d'apprentissage FAIBLE");
    } else {
        println!("  → Distribution BIAISÉE (chi² = {:.2})", chi_squared);
        println!("\n✅ MCTS a des préférences claires");
        println!("  → Les données DEVRAIENT permettre l'apprentissage");
        println!("  → Problème ailleurs (architecture, LR, etc.)");
    }

    let mean_score = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
    println!("\n🎯 Performance MCTS:");
    println!("  Score moyen: {:.2} pts", mean_score);

    Ok(())
}
