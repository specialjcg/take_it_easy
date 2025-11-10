use crate::game::create_deck::create_deck;
use crate::game::plateau::create_plateau_empty;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::remove_tile_from_deck::replace_tile_in_deck;
use crate::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::scoring::scoring::result;
use rand::{rng, Rng};

pub async fn evaluate_model(policy_net: &PolicyNet, value_net: &ValueNet, num_simulations: usize) {
    let mut scores = Vec::new();

    for _ in 0..10 {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();
        let total_turns = 19; // The number of moves in the game
        let mut current_turn = 0;
        while !is_plateau_full(&plateau) {
            let tile_index = rng().random_range(0..deck.tiles.len());
            let chosen_tile = deck.tiles[tile_index];
            let game_result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau,
                &mut deck,
                chosen_tile,
                policy_net,
                value_net,
                num_simulations,
                current_turn,
                total_turns,
                None,
            );
            let best_position = game_result.best_position;
            plateau.tiles[best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
            current_turn += 1; // Increment turn counter each time a tile is placed
        }

        let game_score = result(&plateau);
        scores.push(game_score);
    }

    let _avg_score: f64 = scores.iter().copied().sum::<i32>() as f64 / scores.len() as f64;
    // **Stop ping task**
}
