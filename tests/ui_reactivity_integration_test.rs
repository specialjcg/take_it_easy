// tests/ui_reactivity_integration_test.rs - Test d'intégration end-to-end pour réactivité UI
// Test TDD pour mesurer si placement première tuile < 1s

use std::time::{Duration, Instant};
use tokio::time::sleep;
use tonic::Request;

use take_it_easy::generated::takeiteasygame::v1::*;
use take_it_easy::services::game_service::GameServiceImpl;
use take_it_easy::services::session_manager::SessionManager;
use take_it_easy::neural::policy_value_net::{PolicyNet, ValueNet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tch::nn::VarStore;

#[tokio::test]
async fn test_first_tile_placement_speed_under_1_second() {
    // ============================================================================
    // SETUP - Créer un service de jeu complet
    // ============================================================================

    let session_manager = Arc::new(SessionManager::new().expect("Failed to create session manager"));

    // Créer VarStore pour les réseaux de neurones
    let vs = VarStore::new(tch::Device::Cpu);
    let policy_net = Arc::new(Mutex::new(PolicyNet::new(&vs.root(), (19, 3, 3))));
    let value_net = Arc::new(Mutex::new(ValueNet::new(&vs.root(), (19, 3, 3))));

    let game_service = GameServiceImpl::new(
        session_manager.clone(),
        policy_net.clone(),
        value_net.clone(),
        50 // Réduire les simulations pour test
    );

    // ============================================================================
    // ÉTAPE 1 - Créer une session et démarrer un tour (setup initial)
    // ============================================================================

    // Simuler la création de session (simplifiée pour test)
    let session_id = "test-session-123".to_string();

    // Démarrer un tour pour avoir une tuile disponible
    let start_turn_request = Request::new(StartTurnRequest {
        session_id: session_id.clone(),
    });

    let start_turn_response = game_service.start_turn(start_turn_request).await
        .expect("Failed to start turn");

    // Vérifier qu'on a bien une tuile
    assert!(start_turn_response.get_ref().success, "Start turn should succeed");
    assert!(!start_turn_response.get_ref().announced_tile.is_empty(), "Should have announced tile");

    // Attendre que le système soit complètement initialisé
    sleep(Duration::from_millis(100)).await;

    // ============================================================================
    // ÉTAPE 2 - TEST PRINCIPAL : Mesurer temps de réponse au premier placement
    // ============================================================================

    let start_time = Instant::now();

    // Simuler le placement d'une tuile par le joueur humain
    let make_move_request = Request::new(MakeMoveRequest {
        session_id: session_id.clone(),
        player_id: "TestPlayer".to_string(),
        move_data: r#"{"position": 9}"#.to_string(), // Position centrale du plateau
        timestamp: chrono::Utc::now().timestamp(),
    });

    let move_response = game_service.make_move(make_move_request).await
        .expect("Failed to make move");

    let response_time = start_time.elapsed();

    // ============================================================================
    // ASSERTIONS - Le test DOIT échouer initialement (RED)
    // ============================================================================

    println!("⏱️  Temps de réponse premier placement: {:?}", response_time);

    // 🔴 RED: Ce test DOIT échouer au début avec le système actuel
    assert!(response_time < Duration::from_millis(1000),
            "❌ ÉCHEC ATTENDU: Placement première tuile trop lent: {:?} (doit être < 1000ms)",
            response_time);

    // Vérifier que la réponse est un succès
    match move_response.get_ref().result.as_ref() {
        Some(make_move_response::Result::Success(success)) => {
            assert!(!success.is_game_over, "Game should not be over after first move");
            println!("✅ Mouvement accepté avec temps: {:?}", response_time);
        }
        Some(make_move_response::Result::Error(error)) => {
            panic!("❌ Erreur lors du placement: {} - {}", error.code, error.message);
        }
        None => {
            panic!("❌ Réponse vide du serveur");
        }
    }
}

#[tokio::test]
async fn test_immediate_feedback_without_waiting_mcts() {
    // ============================================================================
    // TEST SPÉCIFIQUE : Vérifier que l'UI n'attend pas MCTS pour confirmer
    // ============================================================================

    let session_manager = Arc::new(SessionManager::new().expect("Failed to create session manager"));

    // Créer VarStore pour les réseaux de neurones
    let vs = VarStore::new(tch::Device::Cpu);
    let policy_net = Arc::new(Mutex::new(PolicyNet::new(&vs.root(), (19, 3, 3))));
    let value_net = Arc::new(Mutex::new(ValueNet::new(&vs.root(), (19, 3, 3))));

    let game_service = GameServiceImpl::new(
        session_manager.clone(),
        policy_net,
        value_net,
        300 // MCTS normal lent pour forcer le test
    );

    // Créer un état de jeu avec tuile disponible
    let session_id = "immediate-test-456".to_string();

    let start_turn_request = Request::new(StartTurnRequest {
        session_id: session_id.clone(),
    });

    game_service.start_turn(start_turn_request).await
        .expect("Failed to start turn");

    // ============================================================================
    // MESURE CRITIQUE : Temps de feedback immédiat
    // ============================================================================

    let start_time = Instant::now();

    let make_move_request = Request::new(MakeMoveRequest {
        session_id: session_id.clone(),
        player_id: "ImmediatePlayer".to_string(),
        move_data: r#"{"position": 5}"#.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    });

    let move_response = game_service.make_move(make_move_request).await
        .expect("Failed to make move");

    let immediate_response_time = start_time.elapsed();

    // ============================================================================
    // CRITÈRE SUCCESS : Feedback en moins de 100ms
    // ============================================================================

    println!("⚡ Temps feedback immédiat: {:?}", immediate_response_time);

    // 🎯 GREEN TARGET: Réponse immédiate < 100ms
    assert!(immediate_response_time < Duration::from_millis(100),
            "Feedback immédiat doit être < 100ms, était: {:?}",
            immediate_response_time);

    // Vérifier que le mouvement est accepté avec statut "PROCESSING"
    match move_response.get_ref().result.as_ref() {
        Some(make_move_response::Result::Success(success)) => {
            // Vérifier que MCTS est en cours de traitement
            assert!(success.mcts_response.contains("PROCESSING") ||
                    success.mcts_response.contains("Move accepted"),
                    "Response should indicate processing or acceptance");
            println!("✅ Feedback immédiat reçu: {:?}", immediate_response_time);
        }
        _ => {
            panic!("❌ Pas de réponse de succès immédiate");
        }
    }
}

#[tokio::test]
async fn test_tile_placement_availability_state() {
    // ============================================================================
    // TEST ÉTAT : Vérifier que les clics sont autorisés dès qu'une tuile existe
    // ============================================================================

    let session_manager = Arc::new(SessionManager::new().expect("Failed to create session manager"));

    // Créer VarStore pour les réseaux de neurones
    let vs = VarStore::new(tch::Device::Cpu);
    let policy_net = Arc::new(Mutex::new(PolicyNet::new(&vs.root(), (19, 3, 3))));
    let value_net = Arc::new(Mutex::new(ValueNet::new(&vs.root(), (19, 3, 3))));

    let game_service = GameServiceImpl::new(session_manager, policy_net, value_net, 50);

    let session_id = "state-test-789".to_string();

    // Démarrer tour
    let start_turn_request = Request::new(StartTurnRequest {
        session_id: session_id.clone(),
    });

    let start_response = game_service.start_turn(start_turn_request).await
        .expect("Failed to start turn");

    // ============================================================================
    // VÉRIFICATIONS D'ÉTAT CRITIQUE
    // ============================================================================

    // 1. Une tuile doit être annoncée
    assert!(start_response.get_ref().success);
    assert!(!start_response.get_ref().announced_tile.is_empty());

    // 2. Les joueurs en attente ne doivent PAS empêcher les clics
    let waiting_players = &start_response.get_ref().waiting_for_players;
    println!("🔍 Joueurs en attente: {:?}", waiting_players);

    // 3. Le mouvement doit être possible IMMÉDIATEMENT après start_turn
    let immediate_move_request = Request::new(MakeMoveRequest {
        session_id: session_id.clone(),
        player_id: "StateTestPlayer".to_string(),
        move_data: r#"{"position": 12}"#.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    });

    let move_result = game_service.make_move(immediate_move_request).await;

    // LE MOUVEMENT DOIT RÉUSSIR même si d'autres joueurs attendent
    assert!(move_result.is_ok(),
            "❌ Mouvement devrait être autorisé immédiatement après start_turn");

    match move_result.unwrap().get_ref().result.as_ref() {
        Some(make_move_response::Result::Success(_)) => {
            println!("✅ Mouvement autorisé immédiatement");
        }
        Some(make_move_response::Result::Error(error)) => {
            panic!("❌ Mouvement refusé: {} - {}", error.code, error.message);
        }
        None => {
            panic!("❌ Pas de réponse");
        }
    }
}