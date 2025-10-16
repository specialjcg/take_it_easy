#[cfg(test)]
pub(crate) mod tests {
    use crate::game::plateau::Plateau;
    use crate::game::tile::Tile;
    pub fn create_mcts_node(state: GameState, parent: Option<*mut MCTSNode>) -> MCTSNode {
        MCTSNode {
            state,
            visits: 0,
            value: 0.0,
            children: Vec::new(),
            parent,
            prior_probabilities: None,
        }
    }
    pub fn choisir_et_placer(deck: &mut Deck, plateau: &mut Plateau) {
        let mut rng = rand::rng();

        // Répéter jusqu'à ce que le plateau soit plein
        while plateau.tiles.contains(&Tile(0, 0, 0)) {
            // Choisir une tuile aléatoirement dans le deck
            let deck_len = deck.tiles.len();
            if deck_len == 0 {
                break; // Plus de tuiles disponibles
            }
            let tile_index = rng.random_range(0..deck_len);
            let tuile = deck.tiles.remove(tile_index); // Retirer la tuile du deck

            // Choisir une position aléatoire dans le plateau
            let mut position;
            loop {
                position = rng.random_range(0..plateau.tiles.len());
                if plateau.tiles[position] == Tile(0, 0, 0) {
                    break; // Trouver une case vide
                }
            }

            // Placer la tuile dans le plateau
            plateau.tiles[position] = tuile;
        }
    }

    pub fn placer_tile(plateau: &mut Plateau, tuile: Tile, position: usize) -> bool {
        if plateau.tiles[position] != Tile(0, 0, 0) {
            return false; // Case déjà occupée
        }
        plateau.tiles[position] = tuile;
        true
    }

    pub fn create_game_state() -> GameState {
        let plateau = create_plateau_empty();
        let deck = create_deck();
        GameState { plateau, deck }
    }

    pub fn apply_move(mut game_state: GameState, tile: Tile, position: usize) -> Option<GameState> {
        if !placer_tile(&mut game_state.plateau, tile, position) {
            return None; // Invalid move
        } else {
            placer_tile(&mut game_state.plateau, tile, position);
            let new_plateau = game_state.plateau.clone();
            let mut new_deck = game_state.deck.clone();
            new_deck = replace_tile_in_deck(&new_deck, &tile);

            Some(GameState {
                plateau: new_plateau,
                deck: new_deck,
            })
        }
    }
    pub fn select_ucb1<'a>(node: &'a mut MCTSNode, exploration: f64) -> &'a mut MCTSNode {
        let total_visits = node.visits as f64;

        node.children
            .iter_mut()
            .map(|child| {
                let ucb_score = if child.visits == 0 {
                    f64::INFINITY // Prioritize unvisited nodes
                } else if total_visits > 0.0 {
                    let exploitation = child.value / child.visits as f64;
                    let exploration_term =
                        exploration * ((total_visits.ln() / child.visits as f64).sqrt());
                    exploitation + exploration_term
                } else {
                    0.0 // No valid exploration term if parent has zero visits
                };
                (ucb_score, child)
            })
            .inspect(|(ucb_score, child)| {
                println!(
                    "Child: visits={}, value={}, UCB1 score={}",
                    child.visits, child.value, ucb_score
                );
            })
            .max_by(|(score_a, _), (score_b, _)| {
                score_a
                    .partial_cmp(score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("No children to select")
            .1
    }
    pub fn expand(mut root: MCTSNode, new_state: GameState) -> MCTSNode {
        let new_node = MCTSNode {
            state: new_state.clone(),
            visits: 0,
            value: 0.0,
            children: Vec::new(),
            parent: None,
            prior_probabilities: None,
        };
        root.children.push(new_node);
        root
    }
    pub fn backpropagate(node: &mut MCTSNode, score: f64) {
        // Start from the current node
        let mut current: *mut MCTSNode = node;

        unsafe {
            // Traverse up the tree
            while let Some(parent) = (*current).parent {
                // Update the current node
                (*current).visits += 1;
                (*current).value += score;

                // Move to the parent node
                current = parent;
            }

            // Update the root node
            (*current).visits += 1;
            (*current).value += score;
        }
    }
    use crate::game::create_deck::create_deck;
    use crate::game::deck::Deck;
    use crate::game::game_state::GameState;
    use crate::game::get_legal_moves::get_legal_moves;
    use crate::game::plateau::create_plateau_empty;
    use crate::game::plateau_is_full::is_plateau_full;
    use crate::game::remove_tile_from_deck::replace_tile_in_deck;
    use crate::game::simulate_game::simulate_games;
    use crate::mcts::mcts_node::MCTSNode;
    use crate::neural::tensor_conversion::convert_plateau_to_tensor;
    use crate::scoring::scoring::result;
    use rand::Rng;

    #[test]
    fn test_placement_tuile_valide_take_it_easy() {
        let mut plateau: Plateau = create_plateau_empty();
        let shuffled_deck: Deck = create_deck();
        let tuile = shuffled_deck.tiles[5].clone();
        assert!(placer_tile(&mut plateau, tuile.clone(), 1));
        assert_eq!(plateau.tiles[1], tuile);
    }

    #[test]
    fn test_is_plateau_full() {
        let mut plateau = create_plateau_empty();
        assert!(!is_plateau_full(&plateau)); // Initially, plateau is empty

        for i in 0..plateau.tiles.len() {
            plateau.tiles[i] = Tile(1, 2, 3); // Fill the plateau
        }
        assert!(is_plateau_full(&plateau)); // Plateau should now be full
    }
    #[test]
    fn test_get_legal_moves_main() {
        let mut plateau = create_plateau_empty();
        let legal_moves = get_legal_moves(plateau.clone());
        assert_eq!(legal_moves.len(), plateau.tiles.len()); // All positions should be legal initially

        plateau.tiles[0] = Tile(1, 2, 3); // Fill one position
        let legal_moves = get_legal_moves(plateau.clone());
        assert_eq!(legal_moves.len(), plateau.tiles.len() - 1); // One less legal move
        assert!(!legal_moves.contains(&0)); // Position 0 should no longer be legal
    }
    #[test]
    fn test_simulate_games() {
        let plateau = create_plateau_empty();
        let deck = create_deck();
        let _num_simulations = 10;

        let avg_score = simulate_games(plateau, deck);
        assert!(avg_score >= 0); // Score should be non-negative
    }
    #[test]
    fn test_convert_plateau_to_tensor() {
        let plateau = create_plateau_empty();
        let tile = Tile(1, 2, 3);
        let deck = create_deck();

        let tensor = convert_plateau_to_tensor(
            &plateau, &tile, &deck, /* usize */ 0, 19, /* usize */
        );
        assert_eq!(tensor.size(), vec![1, 5, 47, 1]); // Ensure the tensor has the correct shape
    }

    #[test]
    fn test_placement_tuile_not_valide_take_it_easy() {
        let mut plateau: Plateau = create_plateau_empty();
        let deck_sfuffle: Deck = create_deck();
        let tile = deck_sfuffle.tiles[5].clone();
        assert!(placer_tile(&mut plateau, tile.clone(), 1));
        assert_eq!(plateau.tiles[1], tile);
        let tile = deck_sfuffle.tiles[5].clone();
        assert!(!placer_tile(&mut plateau, tile.clone(), 1));
    }
    #[test]
    fn test_choir_aleatorytile() {
        // Crée un deck
        let deck_shuffle: Deck = create_deck();

        // Génère un index aléatoire
        let mut rng = rand::rng();
        let index = rng.random_range(0..deck_shuffle.tiles.len());

        // Sélectionne une tuile aléatoire
        let tuile = deck_shuffle.tiles[index].clone();

        // Vérifie que la tuile existe dans le deck
        assert!(deck_shuffle.tiles.contains(&tuile));
        println!("Tuile choisie aléatoirement : {:?}", tuile);
    }

    #[test]
    fn test_retirer_tuile_aleatoire_du_deck() {
        use rand::Rng; // Pour générer un indice aléatoire

        // Crée un deck initial
        let deck_shuffle: Deck = create_deck();

        // Génère un indice aléatoire
        let mut rng = rand::rng();
        let index = rng.random_range(0..deck_shuffle.tiles.len());

        // Récupère la tuile choisie aléatoirement
        let tuile_choisie = deck_shuffle.tiles[index].clone();

        // Supprime la tuile du deck
        let nouveau_deck = replace_tile_in_deck(&deck_shuffle, &tuile_choisie);

        // Vérifie que la nouvelle taille du deck est réduite de 1
        assert_eq!(
            27 - nouveau_deck
                .tiles
                .iter()
                .filter(|&&tile| tile == Tile(0, 0, 0))
                .count(),
            26
        );

        // Vérifie que la tuile choisie n'est plus présente dans le nouveau deck
        assert!(!nouveau_deck.tiles.contains(&tuile_choisie));

        println!("Tuile retirée : {:?}", tuile_choisie);
        println!("Taille du deck initial : {}", deck_shuffle.tiles.len());
        println!("Taille du nouveau deck : {}", nouveau_deck.tiles.len());
    }
    #[test]
    fn test_remplir_plateau_take_it_easy() {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        // Remplir le plateau
        choisir_et_placer(&mut deck, &mut plateau);

        // Vérifier que le plateau est plein
        assert!(!plateau.tiles.contains(&Tile(0, 0, 0)));
        println!("Deck restant : {:?}", deck.tiles);
        println!("Plateau final : {:?}", plateau.tiles);
        // Vérifier que le deck est vide ou contient moins de tuiles
        assert!(deck.tiles.len() + plateau.tiles.len() == 27);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_first_3_plateau_3_1() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 0);
        placer_tile(&mut plateau, deck.tiles[1].clone(), 1);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 2);
        let point = result(&plateau);
        assert_eq!(point, 3);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_first_3_plateau_3_2() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[9].clone(), 0);
        placer_tile(&mut plateau, deck.tiles[10].clone(), 1);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 2);
        let point = result(&plateau);
        assert_eq!(point, 15);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_2_column_plateau_4_2() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[9].clone(), 3);
        placer_tile(&mut plateau, deck.tiles[10].clone(), 4);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 5);
        placer_tile(&mut plateau, deck.tiles[12].clone(), 6);
        println!("{:?}", plateau.tiles);

        let point = result(&plateau);
        assert_eq!(point, 20);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_column_center_plateau_5_2() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[9].clone(), 7);
        placer_tile(&mut plateau, deck.tiles[10].clone(), 8);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 9);
        placer_tile(&mut plateau, deck.tiles[12].clone(), 10);
        placer_tile(&mut plateau, deck.tiles[13].clone(), 11);
        println!("{:?}", plateau.tiles);

        let point = result(&plateau);
        assert_eq!(point, 25);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_column_4_plateau_4_2() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[9].clone(), 12);
        placer_tile(&mut plateau, deck.tiles[10].clone(), 13);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 14);
        placer_tile(&mut plateau, deck.tiles[12].clone(), 15);
        println!("{:?}", plateau.tiles);

        let point = result(&plateau);
        assert_eq!(point, 20);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_last_column_3_plateau_3_1() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 16);
        placer_tile(&mut plateau, deck.tiles[1].clone(), 17);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 18);
        let point = result(&plateau);
        assert_eq!(point, 3);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_first_diag_plateau_0_3_7() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 0);
        placer_tile(&mut plateau, deck.tiles[4].clone(), 3);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 7);
        let point = result(&plateau);
        assert_eq!(point, 6);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_second_diag_plateau_1_4_8_12() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 1);
        placer_tile(&mut plateau, deck.tiles[4].clone(), 4);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 8);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 12);
        let point = result(&plateau);
        assert_eq!(point, 8);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_third_diag_plateau_2_5_9_13_16() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 2);
        placer_tile(&mut plateau, deck.tiles[4].clone(), 5);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 9);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 13);
        placer_tile(&mut plateau, deck.tiles[13].clone(), 16);
        let point = result(&plateau);
        assert_eq!(point, 10);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_fourth_diag_plateau_6_10_14_17() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 6);
        placer_tile(&mut plateau, deck.tiles[4].clone(), 10);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 14);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 17);

        let point = result(&plateau);
        assert_eq!(point, 8);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_last_diag_plateau_11_15_18() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 11);
        placer_tile(&mut plateau, deck.tiles[4].clone(), 15);
        placer_tile(&mut plateau, deck.tiles[5].clone(), 18);

        let point = result(&plateau);
        assert_eq!(point, 6);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_firdt_diag_left_plateau_7_12_16() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 7);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 12);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 16);

        let point = result(&plateau);
        assert_eq!(point, 9);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_second_diag_left_plateau_3_8_13_17() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 3);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 8);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 13);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 17);

        let point = result(&plateau);
        assert_eq!(point, 12);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_third_diag_left_plateau_0_4_9_14_18() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 0);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 4);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 9);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 14);
        placer_tile(&mut plateau, deck.tiles[11].clone(), 18);

        let point = result(&plateau);
        assert_eq!(point, 15);
    }

    #[test]
    fn test_remplir_plateau_take_it_easy_count_fourth_diag_left_plateau_1_5_10_15() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 1);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 5);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 10);
        placer_tile(&mut plateau, deck.tiles[9].clone(), 15);

        let point = result(&plateau);
        assert_eq!(point, 12);
    }
    #[test]
    fn test_remplir_plateau_take_it_easy_count_last_diag_left_plateau_2_6_11() {
        let deck = create_deck();
        let mut plateau = create_plateau_empty();
        placer_tile(&mut plateau, deck.tiles[0].clone(), 2);
        placer_tile(&mut plateau, deck.tiles[2].clone(), 6);
        placer_tile(&mut plateau, deck.tiles[3].clone(), 11);

        let point = result(&plateau);
        assert_eq!(point, 9);
    }

    #[test]
    fn test_get_legal_moves() {
        let mut state = create_game_state();
        placer_tile(&mut state.plateau, state.deck.tiles[0].clone(), 2);
        placer_tile(&mut state.plateau, state.deck.tiles[2].clone(), 6);
        placer_tile(&mut state.plateau, state.deck.tiles[3].clone(), 11);
        // At the beginning, all positions are empty
        let moves: Vec<usize> = get_legal_moves(state.plateau.clone());
        let expectedly: usize = state
            .plateau
            .tiles
            .iter()
            .map(|tile| if tile == &Tile(0, 0, 0) { 1 } else { 0 })
            .sum();

        assert_eq!(moves.len(), expectedly);
    }

    #[test]
    fn test_apply_move() {
        let state = create_game_state();

        let tile = state.deck.tiles[0].clone();
        let position = 2;

        let new_state = apply_move(state, tile, position);

        // Ensure the tile is placed correctly
        assert_eq!(new_state.clone().unwrap().plateau.tiles[position], tile);

        // Ensure the tile is removed from the deck
        assert!(!new_state.clone().unwrap().deck.tiles.contains(&tile));
    }

    #[test]
    fn test_simulate() {
        let state = create_game_state();
        let score = simulate(state);

        // Ensure the simulation produces a valid score
        assert!(score >= 0);
    }
    fn simulate(game_state: GameState) -> i32 {
        let mut simulated_state = game_state.clone();

        // Randomly choose and place tiles until the plateau is full
        // Randomly choose and place tiles until the plateau is full
        while simulated_state.plateau.tiles.contains(&Tile(0, 0, 0)) {
            let legal_moves = get_legal_moves(simulated_state.plateau.clone());
            if legal_moves.is_empty() {
                break; // No more moves possible
            }

            let mut rng = rand::rng();
            let position = legal_moves[rng.random_range(0..legal_moves.len())].clone();
            let tile_index = rng.random_range(0..simulated_state.deck.tiles.len());
            let chosen_tile = simulated_state.deck.tiles[tile_index].clone();
            simulated_state = apply_move(simulated_state, chosen_tile, position).unwrap();
        }

        // Return the result of the completed plateau
        result(&simulated_state.plateau)
    }
    #[test]
    fn test_is_game_over() {
        let mut state = create_game_state();

        // Game is not over at the beginning
        assert!(!is_game_over(state.clone()));
        let mut position = 0;
        while state.plateau.tiles.contains(&Tile(0, 0, 0)) {
            if let Some(empty_tile) = state
                .deck
                .tiles
                .iter()
                .filter(|tile| **tile != Tile(0, 0, 0))
                .collect::<Vec<_>>()
                .first()
            {
                if let Some(new_state) = apply_move(state.clone(), **empty_tile, position) {
                    state = new_state;
                }
            }
            position += 1;
        }

        assert!(is_game_over(state.clone()));
    }
    pub fn is_game_over(game_state: GameState) -> bool {
        !game_state.plateau.tiles.contains(&Tile(0, 0, 0))
    }
    #[test]
    fn test_node_initialization() {
        let initial_state = create_game_state();
        let node = MCTSNode {
            state: initial_state.clone(),
            visits: 0,
            value: 0.0,
            children: Vec::new(),
            parent: None,
            prior_probabilities: None,
        };

        assert_eq!(node.visits, 0);
        assert_eq!(node.value, 0.0);
        assert!(node.children.is_empty());
    }
    #[test]
    fn test_selection_with_ucb1() {
        let mut root = MCTSNode {
            state: create_game_state(),
            visits: 10,
            value: 0.0,
            children: vec![
                MCTSNode {
                    state: create_game_state(),
                    visits: 1,
                    value: 5.0,
                    children: vec![],
                    parent: None,
                    prior_probabilities: None,
                },
                MCTSNode {
                    state: create_game_state(),
                    visits: 2,
                    value: 8.0,
                    children: vec![],
                    parent: None,
                    prior_probabilities: None,
                },
            ],
            parent: None,
            prior_probabilities: None,
        };

        let total_visits = root.visits as f64;
        let exploration = 1.4;

        // Calculate expected UCB1 scores
        let ucb1_child_1 = 5.0 / 1.0 + exploration * ((total_visits.ln() / 1.0).sqrt());
        let ucb1_child_2 = 8.0 / 2.0 + exploration * ((total_visits.ln() / 2.0).sqrt());

        // Select the child with the higher UCB1 score
        let expected_visits = if ucb1_child_1 > ucb1_child_2 { 1 } else { 2 };

        let selected = select_ucb1(&mut root, exploration);
        assert_eq!(selected.visits, expected_visits); // Ensure correct child selected
    }

    #[test]
    fn test_node_expansion() {
        let mut root = MCTSNode {
            state: create_game_state(),
            visits: 0,
            value: 0.0,
            children: Vec::new(),
            parent: None,
            prior_probabilities: None,
        };

        let next_state = create_game_state(); // Define a valid next state
        root = expand(root, next_state.clone());

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].state, next_state);
    }

    #[test]
    fn test_backpropagate_with_parent() {
        let mut root = create_mcts_node(create_game_state(), None);

        let child = create_mcts_node(create_game_state(), Some(&mut root as *mut _));
        root.children.push(child);

        let leaf = create_mcts_node(create_game_state(), Some(&mut root.children[0] as *mut _));
        root.children[0].children.push(leaf);

        println!(
            "Before backpropagation: root = {:?}, child = {:?}, leaf = {:?}",
            root, root.children[0], root.children[0].children[0]
        );

        let leaf_mut = &mut root.children[0].children[0];
        backpropagate(leaf_mut, 10.0);

        println!(
            "After backpropagation: root = {:?}, child = {:?}, leaf = {:?}",
            root, root.children[0], root.children[0].children[0]
        );

        assert_eq!(root.visits, 1);
        assert_eq!(root.value, 10.0);

        assert_eq!(root.children[0].visits, 1);
        assert_eq!(root.children[0].value, 10.0);

        assert_eq!(root.children[0].children[0].visits, 1);
        assert_eq!(root.children[0].children[0].value, 10.0);
    }
    #[test]
    fn test_initialize_root_node() {
        let root_state = create_game_state();
        let root_node = create_mcts_node(root_state.clone(), None);

        assert_eq!(root_node.visits, 0);
        assert_eq!(root_node.value, 0.0);
        assert!(root_node.children.is_empty());
        assert!(root_node.parent.is_none());
    }
    #[test]
    fn test_selection_step_with_ucb1() {
        let mut root = create_mcts_node(create_game_state(), None);
        root.visits = 10; // Ensure the root has non-zero visits

        // Child 1: Unvisited node
        let mut child1 = create_mcts_node(create_game_state(), Some(&mut root as *mut _));
        child1.visits = 0;
        child1.value = 0.0;

        // Child 2: Explored node
        let mut child2 = create_mcts_node(create_game_state(), Some(&mut root as *mut _));
        child2.visits = 2;
        child2.value = 8.0;

        // Child 3: Another explored node
        let mut child3 = create_mcts_node(create_game_state(), Some(&mut root as *mut _));
        child3.visits = 1;
        child3.value = 5.0;

        root.children.push(child1);
        root.children.push(child2);
        root.children.push(child3);

        // Debugging: Print the state of the root and children
        println!("Root: {:?}", root);
        for (i, child) in root.children.iter().enumerate() {
            println!("Child {}: {:?}", i + 1, child);
        }

        // Select the best node using UCB1
        let selected = select_ucb1(&mut root, 1.4);

        // Debugging: Print the selected node
        println!("Selected node: {:?}", selected);

        // The unvisited node (child1) should be selected due to infinite UCB1 score
        assert_eq!(selected.visits, 0);
    }

    #[test]
    fn test_expansion_step() {
        let mut root = create_mcts_node(create_game_state(), None);
        let next_state = create_game_state(); // Simulate the next state

        root = expand(root, next_state.clone());

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].state, next_state);
    }
    #[test]
    fn test_simulation_step() {
        let initial_state = create_game_state();
        let score = simulate(initial_state);

        assert!(score >= 0); // Ensure the score is valid
    }
    #[test]
    fn test_backpropagation_step() {
        let mut root = create_mcts_node(create_game_state(), None);
        let child = create_mcts_node(create_game_state(), Some(&mut root as *mut _));
        root.children.push(child);

        let leaf = create_mcts_node(create_game_state(), Some(&mut root.children[0] as *mut _));
        root.children[0].children.push(leaf);

        let leaf_mut = &mut root.children[0].children[0];
        backpropagate(leaf_mut, 10.0);

        assert_eq!(root.visits, 1);
        assert_eq!(root.value, 10.0);
        assert_eq!(root.children[0].visits, 1);
        assert_eq!(root.children[0].value, 10.0);
        assert_eq!(root.children[0].children[0].visits, 1);
        assert_eq!(root.children[0].children[0].value, 10.0);
    }
    #[test]
    fn test_run_mcts() {
        let mut root = create_mcts_node(create_game_state(), None);
        let exploration = 1.4;

        for _ in 0..100 {
            // Selection
            let mut selected_node: &mut MCTSNode = &mut root;
            while !selected_node.children.is_empty() {
                selected_node = select_ucb1(selected_node, exploration);
            }

            // Expansion
            let legal_moves = get_legal_moves(selected_node.state.plateau.clone());
            if !legal_moves.is_empty() {
                let mut rng = rand::rng();
                let position = legal_moves[rng.random_range(0..legal_moves.len())];
                let tile_index = rng.random_range(0..selected_node.state.deck.tiles.len());
                let chosen_tile = selected_node.state.deck.tiles[tile_index];
                let deck_size_before = selected_node
                    .state
                    .deck
                    .tiles
                    .iter()
                    .filter(|tile| **tile != Tile(0, 0, 0))
                    .collect::<Vec<_>>()
                    .len();
                println!("Deck size before expansion: {}", deck_size_before);

                if let Some(new_state) =
                    apply_move(selected_node.state.clone(), chosen_tile, position)
                {
                    expand(selected_node.clone(), new_state.clone());

                    let deck_size_after = new_state
                        .deck
                        .tiles
                        .iter()
                        .filter(|tile| **tile != Tile(0, 0, 0))
                        .collect::<Vec<_>>()
                        .len();
                    println!("Deck size after expansion: {}", deck_size_after);

                    assert_eq!(
                        deck_size_after,
                        deck_size_before - 1,
                        "Deck size did not decrement correctly!"
                    );
                }
            }

            // Simulation
            let simulated_score = simulate(selected_node.state.clone());

            // Backpropagation
            backpropagate(selected_node, simulated_score.into());
        }

        // Verify root node is updated
        assert!(root.visits > 0);
        assert!(root.value > 0.0);
    }
}
