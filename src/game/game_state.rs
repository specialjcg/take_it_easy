use crate::game::deck::Deck;
use crate::game::plateau::Plateau;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct GameState {
    pub plateau: Plateau,
    pub deck: Deck,
}