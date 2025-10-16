use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::game_state::GameState;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::transformer::game_state::GameStateFeatures;

#[test]
fn test_improved_features_size() {
    let state = GameState {
        plateau: create_plateau_empty(),
        deck: create_deck(),
    };

    let features = state.to_tensor_features();

    println!("\n📊 Test de la nouvelle représentation d'état:");
    println!("   Nombre de features: {}", features.len());
    println!("   Attendu: 256");

    assert_eq!(features.len(), 256, "Les features doivent faire exactement 256 éléments");
}

#[test]
fn test_improved_features_empty_board() {
    let state = GameState {
        plateau: create_plateau_empty(),
        deck: create_deck(),
    };

    let features = state.to_tensor_features();

    // Les 57 premières features (plateau) doivent être à 0
    let plateau_features: Vec<f32> = features.iter().take(57).copied().collect();
    assert!(plateau_features.iter().all(|&f| f == 0.0),
            "Plateau vide devrait avoir toutes les features à 0");

    // Les features de progression doivent indiquer 0 tuiles placées
    let tiles_placed_feature = features[57 + 45 + 45 + 1]; // Index de progression
    assert_eq!(tiles_placed_feature, 0.0, "Aucune tuile placée");

    println!("\n✅ Features du plateau vide correctes");
}

#[test]
fn test_improved_features_with_tiles() {
    let mut state = GameState {
        plateau: create_plateau_empty(),
        deck: create_deck(),
    };

    // Placer quelques tuiles
    state.plateau.tiles[0] = Tile(1, 5, 9);
    state.plateau.tiles[1] = Tile(1, 5, 9);
    state.plateau.tiles[8] = Tile(5, 1, 5);

    let features = state.to_tensor_features();

    // Vérifier que les features du plateau ne sont plus toutes à 0
    let plateau_features: Vec<f32> = features.iter().take(57).copied().collect();
    let non_zero = plateau_features.iter().filter(|&&f| f != 0.0).count();

    println!("\n📊 Test avec tuiles placées:");
    println!("   Features plateau non-nulles: {}/57", non_zero);
    println!("   Tuiles placées: 3");

    assert!(non_zero > 0, "Devrait avoir des features non-nulles");
    assert!(non_zero <= 9, "Devrait correspondre aux 3 tuiles × 3 bandes");

    // Vérifier la progression
    let tiles_placed_feature = features[57 + 45 + 45 + 1];
    assert!(tiles_placed_feature > 0.0, "Progression devrait être > 0");
    assert!(tiles_placed_feature <= 1.0, "Progression devrait être <= 1.0");

    println!("   Progression: {:.2}%", tiles_placed_feature * 100.0);
    println!("✅ Features avec tuiles correctes");
}

#[test]
fn test_improved_features_line_completion() {
    let mut state = GameState {
        plateau: create_plateau_empty(),
        deck: create_deck(),
    };

    // Compléter la première ligne horizontale avec des 5 sur la bande 0
    state.plateau.tiles[0] = Tile(5, 1, 1);
    state.plateau.tiles[1] = Tile(5, 2, 2);
    state.plateau.tiles[2] = Tile(5, 3, 3);

    let features = state.to_tensor_features();

    // Index des features de lignes complètes: 57 (après plateau) + index ligne 0
    let line_completion_start = 57;
    let line_0_complete_1 = features[line_completion_start + 0];
    let line_0_complete_5 = features[line_completion_start + 1];
    let line_0_complete_9 = features[line_completion_start + 2];

    println!("\n📊 Test de complétion de ligne:");
    println!("   Ligne 0 complète avec 1: {}", line_0_complete_1);
    println!("   Ligne 0 complète avec 5: {}", line_0_complete_5);
    println!("   Ligne 0 complète avec 9: {}", line_0_complete_9);

    assert_eq!(line_0_complete_1, 0.0, "Ligne 0 pas complète avec 1");
    assert_eq!(line_0_complete_5, 1.0, "Ligne 0 complète avec 5");
    assert_eq!(line_0_complete_9, 0.0, "Ligne 0 pas complète avec 9");

    println!("✅ Détection de ligne complète correcte");
}

#[test]
fn test_improved_features_score_tracking() {
    let mut state = GameState {
        plateau: create_plateau_empty(),
        deck: create_deck(),
    };

    // Compléter une ligne pour avoir un score
    state.plateau.tiles[0] = Tile(9, 1, 1);
    state.plateau.tiles[1] = Tile(9, 2, 2);
    state.plateau.tiles[2] = Tile(9, 3, 3);

    let features = state.to_tensor_features();

    // Index du score: 57 + 45 + 45
    let score_index = 57 + 45 + 45;
    let score_feature = features[score_index];

    println!("\n📊 Test de tracking du score:");
    println!("   Score normalisé: {:.4}", score_feature);
    println!("   Score estimé: {:.0}", score_feature * 200.0);

    // Ligne complète avec 9: 9 × 3 = 27 points
    assert!(score_feature > 0.0, "Score devrait être > 0");
    assert!(score_feature < 1.0, "Score devrait être < 1.0");

    let estimated_score = (score_feature * 200.0).round() as i32;
    assert_eq!(estimated_score, 27, "Score devrait être 27 (9×3)");

    println!("✅ Tracking du score correct");
}

#[test]
fn test_improved_features_strategic_positions() {
    let state = GameState {
        plateau: create_plateau_empty(),
        deck: create_deck(),
    };

    let features = state.to_tensor_features();

    // Index des positions stratégiques: 57 + 45 + 45 + 1 + 2 + 27 = 177
    let strategic_start = 177;

    // Position 8 (centrale) devrait avoir poids 1.0
    let pos_8_weight = features[strategic_start + 8];

    println!("\n📊 Test des poids stratégiques:");
    println!("   Position 8 (centrale): {:.2}", pos_8_weight);
    println!("   Position 0 (coin): {:.2}", features[strategic_start + 0]);
    println!("   Position 17 (faible): {:.2}", features[strategic_start + 17]);

    assert_eq!(pos_8_weight, 1.0, "Position 8 devrait avoir poids maximal");
    assert!(features[strategic_start + 0] < 1.0, "Position 0 devrait avoir poids plus faible");

    println!("✅ Poids stratégiques corrects");
}
