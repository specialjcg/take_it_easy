// src/services/independent_flow_test.rs - Test du flow indépendant des joueurs

#[cfg(test)]
mod tests {
    use super::super::game_manager::*;
    use crate::game::tile::Tile;

    #[test]
    fn test_independent_player_flow() {
        // Créer un jeu avec 3 joueurs
        let players = vec![
            "alice".to_string(),
            "bob".to_string(),
            "charlie".to_string(),
        ];
        
        let mut game_state = create_take_it_easy_game("test_session".to_string(), players);
        
        // Démarrer un nouveau tour (tire une tuile)
        game_state = start_new_turn(game_state).expect("Should start new turn");
        
        println!("=== Après tirage d'une tuile ===");
        let initial_status = get_all_players_status(&game_state);
        for (player, status) in &initial_status {
            println!("{}: {:?}", player, status);
        }
        
        // Vérifier que tous les joueurs peuvent jouer immédiatement
        assert!(can_player_play_immediately(&game_state, "alice"));
        assert!(can_player_play_immediately(&game_state, "bob"));
        assert!(can_player_play_immediately(&game_state, "charlie"));
        assert!(can_player_play_immediately(&game_state, "mcts_ai"));
        
        assert_eq!(get_players_who_can_play(&game_state).len(), 4); // 3 humains + MCTS
        assert_eq!(get_players_waiting_for_others(&game_state).len(), 0);
        
        // Alice joue en premier
        let alice_move = PlayerMove {
            player_id: "alice".to_string(),
            position: 0, // Position valide (plateau vide)
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        game_state = apply_player_move(game_state, alice_move).expect("Alice move should succeed");
        
        println!("\n=== Après qu'Alice ait joué ===");
        let after_alice_status = get_all_players_status(&game_state);
        for (player, status) in &after_alice_status {
            println!("{}: {:?}", player, status);
        }
        
        // Vérifier le statut après qu'Alice ait joué
        assert!(!can_player_play_immediately(&game_state, "alice")); // Alice en attente
        assert!(can_player_play_immediately(&game_state, "bob"));     // Bob peut encore jouer
        assert!(can_player_play_immediately(&game_state, "charlie")); // Charlie peut encore jouer
        
        assert_eq!(get_players_who_can_play(&game_state).len(), 3); // Bob, Charlie, MCTS
        assert_eq!(get_players_waiting_for_others(&game_state).len(), 1); // Alice
        
        // Bob joue ensuite
        let bob_move = PlayerMove {
            player_id: "bob".to_string(),
            position: 1,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        game_state = apply_player_move(game_state, bob_move).expect("Bob move should succeed");
        
        println!("\n=== Après qu'Alice et Bob aient joué ===");
        let after_bob_status = get_all_players_status(&game_state);
        for (player, status) in &after_bob_status {
            println!("{}: {:?}", player, status);
        }
        
        // Vérifier le statut après que Bob ait joué
        assert!(!can_player_play_immediately(&game_state, "alice"));  // Alice en attente
        assert!(!can_player_play_immediately(&game_state, "bob"));    // Bob en attente
        assert!(can_player_play_immediately(&game_state, "charlie")); // Charlie peut encore jouer
        
        assert_eq!(get_players_who_can_play(&game_state).len(), 2); // Charlie, MCTS
        assert_eq!(get_players_waiting_for_others(&game_state).len(), 2); // Alice, Bob
        
        // Charlie joue en dernier (sans MCTS pour simplifier)
        let charlie_move = PlayerMove {
            player_id: "charlie".to_string(),
            position: 2,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        game_state = apply_player_move(game_state, charlie_move).expect("Charlie move should succeed");
        
        // Retirer MCTS pour simplifier le test
        game_state.waiting_for_players.retain(|id| id != "mcts_ai");
        game_state.player_plateaus.remove("mcts_ai");
        
        // Vérifier la fin du tour
        game_state = check_turn_completion(game_state).expect("Turn should complete");
        
        println!("\n=== Après fin du tour (tous ont joué) ===");
        let final_status = get_all_players_status(&game_state);
        for (player, status) in &final_status {
            println!("{}: {:?}", player, status);
        }
        
        // Un nouveau tour devrait avoir commencé automatiquement
        assert!(game_state.current_tile.is_some(), "New tile should be drawn");
        assert_eq!(game_state.current_turn, 1, "Should be turn 1");
        
        // Tous les joueurs peuvent rejouer immédiatement
        assert!(can_player_play_immediately(&game_state, "alice"));
        assert!(can_player_play_immediately(&game_state, "bob"));
        assert!(can_player_play_immediately(&game_state, "charlie"));
        
        println!("\n✅ Test réussi : Flow indépendant des joueurs fonctionne correctement !");
        println!("- Les joueurs peuvent jouer dès qu'une tuile est disponible");
        println!("- Chaque joueur passe en attente après avoir joué");
        println!("- Un nouveau tour commence automatiquement quand tous ont joué");
    }

    #[test]
    fn test_player_status_transitions() {
        let players = vec!["player1".to_string()];
        let mut game_state = create_take_it_easy_game("test".to_string(), players);
        
        // État initial : pas de tuile
        assert!(matches!(get_player_status(&game_state, "player1"), PlayerStatus::WaitingForNewTile));
        
        // Après tirage d'une tuile
        game_state = start_new_turn(game_state).unwrap();
        assert!(matches!(get_player_status(&game_state, "player1"), PlayerStatus::CanPlay));
        
        // Après que le joueur ait joué
        let move_data = PlayerMove {
            player_id: "player1".to_string(),
            position: 0,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        game_state = apply_player_move(game_state, move_data).unwrap();
        
        // Retirer MCTS pour que le tour se termine
        game_state.waiting_for_players.retain(|id| id != "mcts_ai");
        game_state.player_plateaus.remove("mcts_ai");
        
        // Si c'est le dernier joueur, le statut devrait passer à CanPlay (nouveau tour)
        // ou WaitingForOthers si d'autres n'ont pas joué
        if game_state.waiting_for_players.is_empty() {
            game_state = check_turn_completion(game_state).unwrap();
            // Nouveau tour commencé
            assert!(matches!(get_player_status(&game_state, "player1"), PlayerStatus::CanPlay));
        }
    }
}