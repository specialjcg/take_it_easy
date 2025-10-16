use crate::game::tile::Tile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plateau {
    pub tiles: Vec<Tile>,
}

pub fn create_plateau_empty() -> Plateau {
    Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    }
}
