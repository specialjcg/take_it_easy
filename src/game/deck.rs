use serde::{Deserialize, Serialize};
use crate::game::tile::Tile;

#[derive(Debug, Clone, PartialEq,Serialize,Deserialize)]
pub struct Deck{
    pub(crate) tiles: Vec<Tile>,
}