// src/services/automatic_flow_test.rs - Test du flow automatique souhaité

#[cfg(test)]
mod tests {
    use super::super::game_manager::*;
    
    #[test]
    fn test_automatic_flow_as_requested() {
        println!("=== Test : Flow automatique comme demandé ===");
        
        // Créer un jeu avec 2 joueurs 
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
        
        // Démarrer le premier tour - propose une tuile
        game_state = start_new_turn(game_state).expect("Should start first turn");
        
        println!("\n2️⃣ Après proposition de la première tuile");
        let after_first_tile_status = get_all_players_status(&game_state);
        for (player, status) in &after_first_tile_status {
            println!("   {}: {:?}", player, status);
        }
        
        assert!(game_state.current_tile.is_some(), "Une tuile devrait être proposée");
        assert_eq!(game_state.waiting_for_players.len(), 2, "2 joueurs devraient pouvoir jouer");
        
        // Alice joue (indépendamment)
        let alice_move = PlayerMove {
            player_id: "alice".to_string(),
            position: 0,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        game_state = apply_player_move(game_state, alice_move).expect("Alice move should succeed");
        
        println!("\n3️⃣ Alice a joué (indépendamment)");
        let after_alice_status = get_all_players_status(&game_state);
        for (player, status) in &after_alice_status {
            println!("   {}: {:?}", player, status);
        }
        
        // Alice en attente, Bob peut encore jouer
        assert!(matches!(get_player_status(&game_state, "alice"), PlayerStatus::WaitingForOthers));
        assert!(matches!(get_player_status(&game_state, "bob"), PlayerStatus::CanPlay));
        assert_eq!(game_state.waiting_for_players.len(), 1, "Seul Bob devrait pouvoir jouer");
        
        // Bob joue aussi (indépendamment)
        let bob_move = PlayerMove {
            player_id: "bob".to_string(),
            position: 1,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        game_state = apply_player_move(game_state, bob_move).expect("Bob move should succeed");
        
        println!("\n4️⃣ Bob a aussi joué");
        let after_bob_status = get_all_players_status(&game_state);
        for (player, status) in &after_bob_status {
            println!("   {}: {:?}", player, status);
        }
        
        // Maintenant tous ont joué ce tour
        assert_eq!(game_state.waiting_for_players.len(), 0, "Personne ne devrait attendre");
        
        // Vérifier la fin du tour - devrait automatiquement proposer une nouvelle tuile
        game_state = check_turn_completion(game_state).expect("Turn should complete");
        
        println!("\n5️⃣ Après fin de tour - Nouvelle tuile AUTOMATIQUE");
        let after_turn_completion_status = get_all_players_status(&game_state);
        for (player, status) in &after_turn_completion_status {
            println!("   {}: {:?}", player, status);
        }
        
        // ✅ COMME DEMANDÉ : Une nouvelle tuile est automatiquement proposée
        assert!(game_state.current_tile.is_some(), "Une nouvelle tuile devrait être automatiquement proposée");
        assert_eq!(game_state.waiting_for_players.len(), 2, "Les 2 joueurs devraient pouvoir rejouer");
        assert_eq!(game_state.current_turn, 1, "Le tour devrait être incrémenté");
        
        // Les deux joueurs peuvent immédiatement jouer la nouvelle tuile
        assert!(matches!(get_player_status(&game_state, "alice"), PlayerStatus::CanPlay));
        assert!(matches!(get_player_status(&game_state, "bob"), PlayerStatus::CanPlay));
        
        println!("\n✅ Test réussi : Flow automatique fonctionne comme demandé !");
        println!("- Chaque joueur joue indépendamment dès qu'une tuile est proposée");
        println!("- Dès qu'un joueur joue, il passe en attente");
        println!("- Dès que TOUS ont joué → AUTOMATIQUEMENT nouvelle tuile");
        println!("- Aucune attente inutile, flow continu");
    }

    #[test]
    fn test_multiple_turns_automatic() {
        // Test sur plusieurs tours automatiques
        let players = vec!["player1".to_string(), "player2".to_string()];
        let mut game_state = create_take_it_easy_game("test".to_string(), players);
        game_state.player_plateaus.remove("mcts_ai");
        
        // Démarrer le jeu
        game_state = start_new_turn(game_state).unwrap();
        
        for turn in 0..3 {
            println!("=== Tour {} ===", turn);
            
            // Vérifier qu'une tuile est disponible
            assert!(game_state.current_tile.is_some());
            assert_eq!(game_state.waiting_for_players.len(), 2);
            
            // Player1 joue
            let move1 = PlayerMove {
                player_id: "player1".to_string(),
                position: turn * 2,  // Positions différentes
                tile: game_state.current_tile.unwrap(),
                timestamp: chrono::Utc::now().timestamp(),
            };
            game_state = apply_player_move(game_state, move1).unwrap();
            
            // Player2 joue
            let move2 = PlayerMove {
                player_id: "player2".to_string(),
                position: turn * 2 + 1,  // Positions différentes
                tile: game_state.current_tile.unwrap(),
                timestamp: chrono::Utc::now().timestamp(),
            };
            game_state = apply_player_move(game_state, move2).unwrap();
            
            // Fin de tour - devrait automatiquement proposer la tuile suivante
            let old_turn = game_state.current_turn;
            game_state = check_turn_completion(game_state).unwrap();
            
            if turn < 2 { // Pas le dernier tour
                assert_eq!(game_state.current_turn, old_turn + 1);
                assert!(game_state.current_tile.is_some(), "Nouvelle tuile automatique pour tour {}", turn + 1);
            }
        }
        
        println!("✅ Plusieurs tours automatiques fonctionnent parfaitement");
    }

    #[test]
    fn test_player_independence() {
        // Test l'indépendance : l'ordre n'importe pas
        let players = vec!["fast_player".to_string(), "slow_player".to_string()];
        let mut game_state = create_take_it_easy_game("test".to_string(), players);
        game_state.player_plateaus.remove("mcts_ai");
        
        game_state = start_new_turn(game_state).unwrap();
        
        // Fast player joue immédiatement
        let fast_move = PlayerMove {
            player_id: "fast_player".to_string(),
            position: 0,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        game_state = apply_player_move(game_state, fast_move).unwrap();
        
        // Fast player en attente, slow player peut encore jouer
        assert!(matches!(get_player_status(&game_state, "fast_player"), PlayerStatus::WaitingForOthers));
        assert!(matches!(get_player_status(&game_state, "slow_player"), PlayerStatus::CanPlay));
        
        // Slow player joue plus tard (l'ordre n'importe pas)
        let slow_move = PlayerMove {
            player_id: "slow_player".to_string(),
            position: 1,
            tile: game_state.current_tile.unwrap(),
            timestamp: chrono::Utc::now().timestamp() + 100, // Plus tard
        };
        game_state = apply_player_move(game_state, slow_move).unwrap();
        
        // Tour se termine et nouvelle tuile automatique
        game_state = check_turn_completion(game_state).unwrap();
        
        // Maintenant les deux peuvent rejouer
        assert!(matches!(get_player_status(&game_state, "fast_player"), PlayerStatus::CanPlay));
        assert!(matches!(get_player_status(&game_state, "slow_player"), PlayerStatus::CanPlay));
        
        println!("✅ Indépendance des joueurs confirmée");
    }
}