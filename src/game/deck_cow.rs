/// Copy-on-Write wrapper for Deck to eliminate clone overhead in MCTS
///
/// Companion to PlateauCoW - same principle applied to Deck structure

use crate::game::deck::Deck;
use std::cell::RefCell;
use std::rc::Rc;

/// Copy-on-Write wrapper for Deck
#[derive(Clone)]
pub struct DeckCoW {
    data: Rc<RefCell<Deck>>,
}

impl DeckCoW {
    /// Create new CoW wrapper from existing Deck
    pub fn new(deck: Deck) -> Self {
        Self {
            data: Rc::new(RefCell::new(deck)),
        }
    }

    /// Clone the underlying data for modification
    ///
    /// Only call this when you need to mutate. For read-only access, use `read()`.
    pub fn clone_for_modification(&self) -> DeckCoW {
        let cloned_deck = self.data.borrow().clone();
        DeckCoW::new(cloned_deck)
    }

    /// Read-only access to underlying Deck
    pub fn read<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Deck) -> R,
    {
        f(&self.data.borrow())
    }

    /// Mutable access to underlying Deck
    pub fn write<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Deck) -> R,
    {
        f(&mut self.data.borrow_mut())
    }

    /// Get number of tiles remaining
    pub fn len(&self) -> usize {
        self.read(|d| d.tiles.len())
    }

    /// Unwrap to get owned Deck (consumes the CoW wrapper)
    pub fn into_inner(self) -> Deck {
        match Rc::try_unwrap(self.data) {
            Ok(refcell) => refcell.into_inner(),
            Err(rc) => rc.borrow().clone(),
        }
    }

    /// Get strong reference count (for debugging/metrics)
    pub fn ref_count(&self) -> usize {
        Rc::strong_count(&self.data)
    }
}

impl std::fmt::Debug for DeckCoW {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeckCoW")
            .field("ref_count", &self.ref_count())
            .field("tiles_count", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck_cow_no_clone_on_read() {
        let deck = crate::game::create_deck::create_deck();
        let deck_cow = DeckCoW::new(deck);
        let initial_count = deck_cow.ref_count();

        // Multiple reads should not increase ref count
        deck_cow.read(|d| d.tiles.len());
        deck_cow.read(|d| d.tiles.len());

        assert_eq!(deck_cow.ref_count(), initial_count);
    }

    #[test]
    fn test_deck_cow_shared_read() {
        let deck = crate::game::create_deck::create_deck();
        let deck_cow = DeckCoW::new(deck);
        let clone1 = deck_cow.clone();
        let clone2 = deck_cow.clone();

        // All share same underlying data
        assert_eq!(deck_cow.ref_count(), 3);

        // All can read
        let len = deck_cow.len();
        assert!(len > 0);
        assert_eq!(clone1.len(), len);
        assert_eq!(clone2.len(), len);
    }

    #[test]
    fn test_deck_cow_clone_for_modification() {
        let deck = crate::game::create_deck::create_deck();
        let deck_cow = DeckCoW::new(deck);
        let modified = deck_cow.clone_for_modification();

        // Independent wrappers
        assert_eq!(deck_cow.ref_count(), 1);
        assert_eq!(modified.ref_count(), 1);
    }
}
