use serde::{Deserialize, Serialize};
use crate::game::tile::Tile;

#[derive(Debug, Clone, PartialEq,Serialize,Deserialize)]
pub(crate) struct Plateau{
    pub(crate) tiles: Vec<Tile>,
}


pub(crate) fn create_plateau_empty() -> Plateau {
    Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    }
}
