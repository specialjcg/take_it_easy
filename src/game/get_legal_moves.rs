use crate::game::plateau::Plateau;
use crate::game::tile::Tile;

/// Returns indices of all empty positions on the plateau
/// Optimized to take a reference instead of ownership to avoid unnecessary clones
pub fn get_legal_moves(plateau: &Plateau) -> Vec<usize> {
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
