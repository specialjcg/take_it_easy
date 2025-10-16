use crate::game::tile::Tile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    pub(crate) tiles: Vec<Tile>,
}
