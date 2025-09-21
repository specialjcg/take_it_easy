// tests/ui_reactivity_regression_test.rs - Test de régression pour la réactivité UI
// Vérifie que enhance_game_state_with_images popule les available_positions

use std::collections::HashMap;
use serde_json::Value;

use take_it_easy::services::game_service::state_provider::enhance_game_state_with_images;
use take_it_easy::services::game_manager::{create_take_it_easy_game};
use take_it_easy::game::tile::Tile;

/// Test de régression pour éviter le bug de réactivité UI de 30 secondes
/// Ce test vérifie que enhance_game_state_with_images popule correctement available_positions
#[test]
fn test_enhance_game_state_populates_available_positions_immediately() {
    // Arrangement: Créer un état de jeu simple avec un joueur
    let session_id = "test_session".to_string();
    let player_id = "test_player".to_string();

    let game_state = create_take_it_easy_game(session_id, vec![player_id.clone()]);
    let raw_game_state_json = serde_json::to_string(&game_state).unwrap();

    // Action: Appeler enhance_game_state_with_images (c'est notre fix)
    let start_time = std::time::Instant::now();
    let enhanced_json = enhance_game_state_with_images(&raw_game_state_json);
    let duration = start_time.elapsed();

    // Assertion 1: L'opération doit être rapide (< 100ms)
    assert!(
        duration.as_millis() < 100,
        "enhance_game_state_with_images should be fast, took {}ms",
        duration.as_millis()
    );

    // Assertion 2: Le JSON enrichi doit être parsable
    let enhanced_state: Value = serde_json::from_str(&enhanced_json)
        .expect("Enhanced game state should be valid JSON");

    // Assertion 3: player_plateaus doit exister
    assert!(
        enhanced_state.get("player_plateaus").is_some(),
        "Enhanced state should have player_plateaus"
    );

    let player_plateaus = enhanced_state["player_plateaus"].as_object()
        .expect("player_plateaus should be an object");

    // Assertion 4: Notre joueur doit avoir un plateau
    assert!(
        player_plateaus.contains_key(&player_id),
        "Player {} should have a plateau", player_id
    );

    let player_plateau = &player_plateaus[&player_id];

    // Assertion 5: CRITIQUE - available_positions doit être populé immédiatement
    assert!(
        player_plateau.get("available_positions").is_some(),
        "Player plateau should have available_positions field after enhancement"
    );

    let available_positions = player_plateau["available_positions"].as_array()
        .expect("available_positions should be an array");

    // Assertion 6: Pour un nouveau plateau, toutes les positions devraient être disponibles
    assert!(
        available_positions.len() > 0,
        "Available positions should not be empty after enhancement"
    );

    assert!(
        available_positions.len() <= 19,
        "Available positions should not exceed board size (19)"
    );

    // Assertion 7: Les positions doivent être des entiers valides
    for position in available_positions {
        let pos_num = position.as_i64()
            .expect("Position should be a number");
        assert!(
            pos_num >= 0 && pos_num <= 18,
            "Position {} should be between 0 and 18", pos_num
        );
    }

    println!("✅ Test réussi: enhance_game_state_with_images popule {} positions en {}ms",
            available_positions.len(), duration.as_millis());
}

/// Test que les positions vides sont correctement identifiées
#[test]
fn test_empty_positions_are_detected_correctly() {
    // Arrangement: Créer un état avec quelques tuiles placées
    let session_id = "test_partial".to_string();
    let player_id = "test_player".to_string();

    let mut game_state = create_take_it_easy_game(session_id, vec![player_id.clone()]);

    // Placer quelques tuiles pour simuler un jeu en cours
    if let Some(plateau) = game_state.player_plateaus.get_mut(&player_id) {
        plateau.tiles[0] = Tile(1, 2, 3); // Position 0 occupée
        plateau.tiles[5] = Tile(4, 5, 6); // Position 5 occupée
        plateau.tiles[10] = Tile(7, 8, 9); // Position 10 occupée
    }

    let game_state_json = serde_json::to_string(&game_state).unwrap();

    // Action: Enrichir l'état
    let enhanced_json = enhance_game_state_with_images(&game_state_json);
    let enhanced_state: Value = serde_json::from_str(&enhanced_json).unwrap();

    // Assertion: Vérifier que seules les positions vides sont disponibles
    let available_positions = enhanced_state["player_plateaus"][&player_id]["available_positions"]
        .as_array().expect("Should have available_positions array");

    // Les positions 0, 5, 10 ne devraient PAS être disponibles
    let available_nums: Vec<i64> = available_positions.iter()
        .map(|v| v.as_i64().unwrap())
        .collect();

    assert!(!available_nums.contains(&0), "Position 0 should not be available (occupied)");
    assert!(!available_nums.contains(&5), "Position 5 should not be available (occupied)");
    assert!(!available_nums.contains(&10), "Position 10 should not be available (occupied)");

    // Les autres positions devraient être disponibles
    assert!(available_nums.contains(&1), "Position 1 should be available (empty)");
    assert!(available_nums.contains(&2), "Position 2 should be available (empty)");

    // Total: 19 - 3 = 16 positions disponibles
    assert_eq!(available_positions.len(), 16, "Should have 16 available positions (19 - 3 occupied)");

    println!("✅ Test partiel réussi: {} positions disponibles sur 19", available_positions.len());
}