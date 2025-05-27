use crate::game::tile::Tile;

#[derive(Debug, Clone, PartialEq)]
pub struct Deck{
    pub(crate) tiles: Vec<Tile>,
}