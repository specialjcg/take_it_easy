use crate::game::plateau::Plateau;
use crate::game::tile::Tile;

pub fn get_legal_moves(plateau: Plateau) -> Vec<usize> {
    plateau
        .tiles
        .iter()
        .enumerate()
        .filter_map(|(i, tile)| {
            if *tile == Tile(0, 0, 0) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}
