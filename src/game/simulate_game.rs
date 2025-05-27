use rand::Rng;
use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::tile::Tile;
use crate::scoring::scoring::result;

pub fn simulate_games(plateau: Plateau, deck: Deck) -> i32 {
    let mut simulated_plateau = plateau.clone();
    let simulated_deck = deck.clone();
    let mut legal_moves = get_legal_moves(simulated_plateau.clone());

    // Filter out invalid tiles (0, 0, 0)
    let mut valid_tiles: Vec<Tile> = simulated_deck
        .tiles
        .iter()
        .cloned()
        .filter(|tile| *tile != Tile(0, 0, 0))
        .collect();

    let mut rng = rand::rng(); // Fixed: Use new API

    while !is_plateau_full(&simulated_plateau) {
        if legal_moves.is_empty() || valid_tiles.is_empty() {
            break;
        }

        // Fixed: Use new rand API
        let position_index = rng.random_range(0..legal_moves.len());
        let position = legal_moves.swap_remove(position_index); // Swap-remove for O(1) removal

        let tile_index = rng.random_range(0..valid_tiles.len());
        let chosen_tile = valid_tiles.swap_remove(tile_index); // Swap-remove for O(1) removal

        // Place the chosen tile
        simulated_plateau.tiles[position] = chosen_tile;
    }

    result(&simulated_plateau) // Compute and return the result
}