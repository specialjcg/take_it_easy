use crate::game::tile::Tile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    pub(crate) tiles: Vec<Tile>,
}

impl Deck {
    /// Get a reference to the tiles in the deck
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// Get a mutable reference to the tiles in the deck
    pub fn tiles_mut(&mut self) -> &mut Vec<Tile> {
        &mut self.tiles
    }
}
