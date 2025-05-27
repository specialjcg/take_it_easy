use crate::game::plateau::Plateau;
use crate::game::tile::Tile;

pub fn is_plateau_full(plateau: &Plateau) -> bool {
    plateau.tiles.iter().all(|tile| *tile != Tile(0, 0, 0))
}