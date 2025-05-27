use crate::game::deck::Deck;
use crate::game::plateau::Plateau;

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    pub plateau: Plateau,
    pub deck: Deck,
}