use crate::test::{Plateau, Tile};

pub fn placer_tile(plateau: &mut Plateau, tuile: Tile, position: usize) -> bool {
    if plateau.tiles[position] != Tile(0, 0, 0) {
        return false; // Case déjà occupée
    }
    plateau.tiles[position] = tuile;
    true
}
