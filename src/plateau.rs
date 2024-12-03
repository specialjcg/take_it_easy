use crate::test::Tile;

#[derive(Debug, Clone,PartialEq)]
pub(crate) struct Plateau{
    pub(crate) tiles: Vec<Tile>,
}