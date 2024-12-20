use crate::test::{Plateau, Tile};

pub(crate) fn create_plateau_empty() -> Plateau {
    Plateau {
        tiles: vec![Tile(0, 0, 0); 19],
    }
}
