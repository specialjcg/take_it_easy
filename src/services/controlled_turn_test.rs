// src/services/controlled_turn_test.rs - Test de la logique contrôlée des tours

#[cfg(test)]
mod tests {
    use super::super::game_manager::*;
    
    #[test]
    fn test_controlled_turn_flow() {
        println!("=== Test : Flow contrôlé des tours ===");
        
        // Créer un jeu avec 2 joueurs (plus simple à tester)
        let players = vec![
            "alice".to_string(),
            "bob".to_string(),
        ];
        
        let mut game_state = create_take_it_easy_game("test_session".to_string(), players);
        
        // Retirer MCTS pour simplifier
        game_state.player_plateaus.remove("mcts_ai");
        
        println!("\n1️⃣ État initial : Pas de tuile");
        let initial_status = get_all_players_status(&game_state);
        for (player, status) in &initial_status {
            println!("   {}: {:?}", player, status);
        }
        
        // Vérifier qu'on peut démarrer un nouveau tour
        assert!(can_start_new_turn(&game_state), "Devrait pouvoir démarrer le premier tour");
        
        // Démarrer le premier tour
        game_state = start_new_turn(game_state).expect("Should start first turn");
        
        println!("\n2️⃣ Après proposition de la première tuile");
        let after_first_tile_status = get_all_players_status(&game_state);
        for (player, status) in &after_first_tile_status {
            println!("   {}: {:?}", player, status);
        }
        
        assert!(game_state.current_tile.is_some(), "Une tuile devrait être proposée");
        assert_eq!(game_state.waiting_for_players.len(), 2, "2 joueurs devraient pouvoir jouer");
        
        // Vérifier qu'on NE PEUT PAS proposer une nouvelle tuile maintenant
        assert!(!can_start_new_turn(&game_state), "Ne devrait PAS pouvoir proposer une nouvelle tuile");
        
        // Essayer de proposer une nouvelle tuile (devrait échouer)
        let result = start_new_turn(game_state.clone());
        assert!(result.is_err(), "Proposer une nouvelle tuile devrait échouer");
        println!("   ✅ Impossible de proposer une nouvelle tuile tant que des joueurs attendent");
        
        // Alice joue
        let alice_move = PlayerMove {
            player_id: "alice".to_string(),
            position: 0,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        game_state = apply_player_move(game_state, alice_move).expect("Alice move should succeed");
        
        println!("\n3️⃣ Alice a joué, Bob n'a pas encore joué");
        let after_alice_status = get_all_players_status(&game_state);
        for (player, status) in &after_alice_status {
            println!("   {}: {:?}", player, status);
        }
        
        // Vérifier qu'on NE PEUT TOUJOURS PAS proposer une nouvelle tuile
        assert!(!can_start_new_turn(&game_state), "Ne devrait toujours PAS pouvoir proposer une nouvelle tuile");
        assert_eq!(game_state.waiting_for_players.len(), 1, "Bob devrait encore pouvoir jouer");
        
        // Bob joue
        let bob_move = PlayerMove {
            player_id: "bob".to_string(),
            position: 1,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        game_state = apply_player_move(game_state, bob_move).expect("Bob move should succeed");
        
        println!("\n4️⃣ Alice et Bob ont tous les deux joué");
        let after_both_played_status = get_all_players_status(&game_state);
        for (player, status) in &after_both_played_status {
            println!("   {}: {:?}", player, status);
        }
        
        // Maintenant vérifier la fin du tour
        game_state = check_turn_completion(game_state).expect("Turn should complete");
        
        println!("\n5️⃣ Après vérification de fin de tour");
        let after_turn_completion_status = get_all_players_status(&game_state);
        for (player, status) in &after_turn_completion_status {
            println!("   {}: {:?}", player, status);
        }
        
        // ✅ NOUVEAU : Maintenant il ne devrait PAS y avoir de nouvelle tuile automatiquement
        assert!(game_state.current_tile.is_none(), "Aucune tuile ne devrait être proposée automatiquement");
        assert!(game_state.waiting_for_players.is_empty(), "Aucun joueur ne devrait attendre");
        assert_eq!(game_state.current_turn, 1, "Le tour devrait être incrémenté");
        
        // ✅ NOUVEAU : Maintenant on PEUT proposer une nouvelle tuile manuellement
        assert!(can_start_new_turn(&game_state), "Maintenant on devrait pouvoir proposer une nouvelle tuile");
        
        println!("\n6️⃣ Proposition manuelle d'une nouvelle tuile");
        game_state = start_new_turn(game_state).expect("Should start second turn");
        
        let after_second_tile_status = get_all_players_status(&game_state);
        for (player, status) in &after_second_tile_status {
            println!("   {}: {:?}", player, status);
        }
        
        assert!(game_state.current_tile.is_some(), "Une nouvelle tuile devrait être proposée");
        assert_eq!(game_state.waiting_for_players.len(), 2, "Les 2 joueurs devraient pouvoir rejouer");
        
        println!("\n✅ Test réussi : Contrôle des tours fonctionne correctement !");
        println!("- Une tuile n'est proposée QUE si tous les joueurs ont terminé le tour précédent");
        println!("- Les joueurs restent en attente jusqu'à ce que tous aient joué");
        println!("- Une nouvelle tuile doit être explicitement demandée");
    }

    #[test]  
    fn test_player_status_transitions_controlled() {
        // Test des transitions de statut avec la nouvelle logique
        let players = vec!["player1".to_string(), "player2".to_string()];
        let mut game_state = create_take_it_easy_game("test".to_string(), players);
        game_state.player_plateaus.remove("mcts_ai");
        
        // État initial : tous en attente de nouvelle tuile
        assert!(matches!(get_player_status(&game_state, "player1"), PlayerStatus::WaitingForNewTile));
        assert!(matches!(get_player_status(&game_state, "player2"), PlayerStatus::WaitingForNewTile));
        
        // Proposition d'une tuile
        game_state = start_new_turn(game_state).unwrap();
        assert!(matches!(get_player_status(&game_state, "player1"), PlayerStatus::CanPlay));
        assert!(matches!(get_player_status(&game_state, "player2"), PlayerStatus::CanPlay));
        
        // Player1 joue
        let move1 = PlayerMove {
            player_id: "player1".to_string(),
            position: 0,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        game_state = apply_player_move(game_state, move1).unwrap();
        
        // Player1 en attente, Player2 peut encore jouer
        assert!(matches!(get_player_status(&game_state, "player1"), PlayerStatus::WaitingForOthers));
        assert!(matches!(get_player_status(&game_state, "player2"), PlayerStatus::CanPlay));
        
        // Player2 joue aussi
        let move2 = PlayerMove {
            player_id: "player2".to_string(),
            position: 1,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        game_state = apply_player_move(game_state, move2).unwrap();
        
        // Fin de tour
        game_state = check_turn_completion(game_state).unwrap();
        
        // ✅ NOUVEAU : Tous en attente de nouvelle tuile (pas de tuile automatique)
        assert!(matches!(get_player_status(&game_state, "player1"), PlayerStatus::WaitingForNewTile));
        assert!(matches!(get_player_status(&game_state, "player2"), PlayerStatus::WaitingForNewTile));
    }
}