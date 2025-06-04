// tests/session_manager_integration_test.rs
// Premier test d'intégration pour session_manager

use std::sync::Arc;
use crate::services::session_manager::*;
use crate::generated::takeiteasygame::v1::Player;

#[cfg(test)]
mod session_manager_basic_integration {
    use super::*;

    #[tokio::test]
    async fn test_complete_session_workflow() {
        // 🏗️ ÉTAPE 1: Créer le manager
        let manager = new_session_manager();

        // 🏗️ ÉTAPE 2: Créer une session
        let session_code = create_session_with_manager(&manager, 4, "classic".to_string()).await;

        // ✅ Vérifier que la session existe
        assert!(!session_code.is_empty());
        assert_ne!(session_code, "ERROR");

        // 🏗️ ÉTAPE 3: Récupérer la session créée
        let session = get_session_by_code_with_manager(&manager, &session_code).await;
        assert!(session.is_some());

        let mut session = session.unwrap();
        let session_id = session.id.clone();

        // ✅ Vérifier les propriétés initiales
        assert_eq!(session.code, session_code);
        assert_eq!(session.max_players, 4);
        assert_eq!(session.game_mode, "classic");
        assert_eq!(session.players.len(), 0);
        assert_eq!(session.state, 0); // WAITING

        // 🏗️ ÉTAPE 4: Ajouter le premier joueur (Alice)
        let result = add_player_to_session(session.clone(), "Alice".to_string());
        assert!(result.is_ok());

        let (session_with_alice, alice_id) = result.unwrap();

        // ✅ Vérifier Alice
        assert_eq!(session_with_alice.players.len(), 1);
        assert!(session_with_alice.players.contains_key(&alice_id));

        let alice = &session_with_alice.players[&alice_id];
        assert_eq!(alice.name, "Alice");
        assert!(alice.is_ready); // Premier joueur auto-ready
        assert!(alice.is_connected);

        // 🏗️ ÉTAPE 5: Sauvegarder la session mise à jour
        let update_result = update_session_with_manager(&manager, session_with_alice.clone()).await;
        assert!(update_result.is_ok());

        // 🏗️ ÉTAPE 6: Ajouter un second joueur (Bob)
        let transform_result = transform_session_with_manager(&manager, &session_id, |session| {
            add_player_to_session(session, "Bob".to_string())
                .map(|(updated_session, _player_id)| updated_session)
        }).await;

        assert!(transform_result.is_ok());
        assert!(transform_result.unwrap().is_some());

        // 🏗️ ÉTAPE 7: Récupérer l'état final et vérifier
        let final_session = get_session_by_id_with_manager(&manager, &session_id).await;
        assert!(final_session.is_some());

        let final_session = final_session.unwrap();
        assert_eq!(final_session.players.len(), 2);

        // ✅ Vérifier que les deux joueurs sont présents
        let alice_exists = final_session.players.values().any(|p| p.name == "Alice" && p.is_ready);
        let bob_exists = final_session.players.values().any(|p| p.name == "Bob" && !p.is_ready);

        assert!(alice_exists, "Alice should exist and be ready");
        assert!(bob_exists, "Bob should exist and not be ready initially");

        println!("✅ Session workflow test completed successfully!");
    }

    #[tokio::test]
    async fn test_player_ready_triggers_game_start() {
        // 🏗️ Setup: Créer session avec 2 joueurs
        let manager = new_session_manager();
        let session_code = create_session_with_manager(&manager, 4, "classic".to_string()).await;
        let session = get_session_by_code_with_manager(&manager, &session_code).await.unwrap();
        let session_id = session.id.clone();

        // Ajouter Alice (auto-ready)
        let (session_with_alice, alice_id) = add_player_to_session(session, "Alice".to_string()).unwrap();
        update_session_with_manager(&manager, session_with_alice).await.unwrap();

        // Ajouter Bob
        let _transform_result = transform_session_with_manager(&manager, &session_id, |session| {
            add_player_to_session(session, "Bob".to_string())
                .map(|(updated_session, _player_id)| updated_session)
        }).await.unwrap().unwrap();

        // Récupérer l'ID de Bob depuis la session mise à jour
        let session_with_bob = get_session_by_id_with_manager(&manager, &session_id).await.unwrap();
        let bob_id = session_with_bob.players.values()
            .find(|p| p.name == "Bob")
            .unwrap()
            .id.clone();

        // 🏗️ ÉTAPE CRITIQUE: Bob se met ready
        let store = get_store_from_manager(&manager);
        let ready_result = transform_session_in_store(store, &session_id, |session| {
            set_player_ready_in_session(session, &bob_id, true)
        }).await;

        assert!(ready_result.is_ok());
        let session_result = ready_result.unwrap();
        assert!(session_result.is_some());
        let game_started = session_result.unwrap();

        // ✅ VÉRIFICATION: Le jeu doit avoir démarré
        assert!(game_started, "Game should start when both players are ready");

        // Vérifier l'état final de la session
        let final_session = get_session_by_id_with_manager(&manager, &session_id).await.unwrap();
        assert_eq!(final_session.state, 1); // IN_PROGRESS
        assert!(final_session.current_player_id.is_some());
        assert_eq!(final_session.turn_number, 1);

        println!("✅ Game start trigger test completed successfully!");
    }

    #[tokio::test]
    async fn test_session_not_found_scenarios() {
        let manager = new_session_manager();

        // Test avec un ID inexistant
        let result = get_session_by_id_with_manager(&manager, "inexistant_id").await;
        assert!(result.is_none());

        // Test avec un code inexistant
        let result = get_session_by_code_with_manager(&manager, "NOCODE").await;
        assert!(result.is_none());

        // Test de transformation sur session inexistante
        let transform_result = transform_session_with_manager(&manager, "fake_id", |session| {
            Ok(session) // Cette fonction ne devrait jamais être appelée
        }).await;

        assert!(transform_result.is_ok());
        assert!(transform_result.unwrap().is_none()); // None car session n'existe pas

        println!("✅ Session not found scenarios test completed!");
    }

    #[tokio::test]
    async fn test_session_full_error() {
        let manager = new_session_manager();

        // Créer une session avec limite de 2 joueurs
        let session_code = create_session_with_manager(&manager, 2, "classic".to_string()).await;
        let session = get_session_by_code_with_manager(&manager, &session_code).await.unwrap();

        // Ajouter 2 joueurs (limite atteinte)
        let (session1, _) = add_player_to_session(session, "Alice".to_string()).unwrap();
        let (session2, _) = add_player_to_session(session1, "Bob".to_string()).unwrap();

        // Essayer d'ajouter un 3ème joueur
        let result = add_player_to_session(session2, "Charlie".to_string());

        // ✅ Doit échouer avec SESSION_FULL
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "SESSION_FULL");

        println!("✅ Session full error test completed!");
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let manager = Arc::new(new_session_manager());
        let session_code = create_session_with_manager(&manager, 10, "test".to_string()).await;
        let session = get_session_by_code_with_manager(&manager, &session_code).await.unwrap();
        let session_id = session.id.clone();

        // Lancer 5 opérations concurrentes d'ajout de joueurs
        let mut handles = vec![];

        for i in 0..5 {
            let manager_clone = manager.clone();
            let session_id_clone = session_id.clone();

            let handle = tokio::spawn(async move {
                transform_session_with_manager(&manager_clone, &session_id_clone, |session| {
                    add_player_to_session(session, format!("Player{}", i))
                        .map(|(updated_session, _)| updated_session)
                }).await
            });

            handles.push(handle);
        }

        // Attendre que toutes les opérations se terminent
        let results = futures::future::join_all(handles).await;

        // Vérifier que toutes ont réussi
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Task {} failed", i);
            assert!(result.as_ref().unwrap().is_ok(), "Transform {} failed", i);
        }

        // Vérifier l'état final
        let final_session = get_session_by_id_with_manager(&manager, &session_id).await.unwrap();
        assert_eq!(final_session.players.len(), 5);

        println!("✅ Concurrent operations test completed! Added {} players", final_session.players.len());
    }
}

// ============================================================================
// HELPER POUR LANCER CE TEST
// ============================================================================

#[cfg(test)]
mod test_runner {
    use super::*;

    #[tokio::test]
    async fn run_session_manager_integration_tests() {
        println!("🚀 Running Session Manager Integration Tests");
        println!("===========================================");

        // Ce test sert de point d'entrée documenté
        // Les vrais tests sont dans le module session_manager_basic_integration

        // Test simple pour vérifier que l'infrastructure fonctionne
        let manager = new_session_manager();
        let session_code = create_session_with_manager(&manager, 4, "test".to_string()).await;
        assert!(!session_code.is_empty());

        println!("✅ Session Manager integration tests infrastructure OK!");
    }
}