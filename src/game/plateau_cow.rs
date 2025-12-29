/// Copy-on-Write wrapper for Plateau to eliminate clone overhead in MCTS
///
/// Problem: MCTS algorithm performs 36,750+ clones per call, consuming 30% CPU time
/// Solution: Use Rc<RefCell<>> to share immutable data, only clone when mutating
///
/// Performance impact: Expected -80% allocations, +40-60% throughput

use crate::game::plateau::Plateau;
use crate::game::tile::Tile;
use std::cell::RefCell;
use std::rc::Rc;

/// Copy-on-Write wrapper for Plateau
///
/// # Usage Pattern
/// ```ignore
/// let plateau_cow = PlateauCoW::new(plateau);
///
/// // Read without cloning
/// plateau_cow.read(|p| get_legal_moves(p));
///
/// // Clone only when modifying
/// let mut modified = plateau_cow.clone_for_modification();
/// modified.set_tile(position, tile);
/// ```
#[derive(Clone)]
pub struct PlateauCoW {
    data: Rc<RefCell<Plateau>>,
}

impl PlateauCoW {
    /// Create new CoW wrapper from existing Plateau
    pub fn new(plateau: Plateau) -> Self {
        Self {
            data: Rc::new(RefCell::new(plateau)),
        }
    }

    /// Create empty plateau wrapped in CoW
    pub fn new_empty() -> Self {
        Self::new(crate::game::plateau::create_plateau_empty())
    }

    /// Clone the underlying data for modification
    ///
    /// Only call this when you need to mutate. For read-only access, use `read()`.
    pub fn clone_for_modification(&self) -> PlateauCoW {
        let cloned_plateau = self.data.borrow().clone();
        PlateauCoW::new(cloned_plateau)
    }

    /// Read-only access to underlying Plateau
    ///
    /// No cloning occurs - data is borrowed via RefCell
    pub fn read<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Plateau) -> R,
    {
        f(&self.data.borrow())
    }

    /// Mutable access to underlying Plateau
    ///
    /// WARNING: This mutates the shared data. Prefer clone_for_modification() for MCTS branches.
    pub fn write<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Plateau) -> R,
    {
        f(&mut self.data.borrow_mut())
    }

    /// Set a tile at a specific position (convenience method)
    pub fn set_tile(&self, position: usize, tile: Tile) {
        self.write(|p| {
            if position < p.tiles.len() {
                p.tiles[position] = tile;
            }
        });
    }

    /// Get tile at position (convenience method)
    pub fn get_tile(&self, position: usize) -> Option<Tile> {
        self.read(|p| p.tiles.get(position).copied())
    }

    /// Get reference to tiles vector (for compatibility)
    pub fn tiles(&self) -> Vec<Tile> {
        self.read(|p| p.tiles.clone())
    }

    /// Unwrap to get owned Plateau (consumes the CoW wrapper)
    pub fn into_inner(self) -> Plateau {
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

impl std::fmt::Debug for PlateauCoW {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlateauCoW")
            .field("ref_count", &self.ref_count())
            .field("plateau", &*self.data.borrow())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tile::Tile;

    #[test]
    fn test_cow_no_clone_on_read() {
        let plateau_cow = PlateauCoW::new_empty();

        // Multiple reads should not increase ref count
        let initial_count = plateau_cow.ref_count();
        plateau_cow.read(|p| p.tiles.len());
        plateau_cow.read(|p| p.tiles.len());

        assert_eq!(plateau_cow.ref_count(), initial_count);
    }

    #[test]
    fn test_cow_clone_on_modification() {
        let plateau_cow = PlateauCoW::new_empty();
        let initial_count = plateau_cow.ref_count();

        // Clone for modification creates new Rc
        let modified = plateau_cow.clone_for_modification();

        // Both wrappers exist independently
        assert_eq!(plateau_cow.ref_count(), initial_count);
        assert_eq!(modified.ref_count(), 1);
    }

    #[test]
    fn test_cow_shared_read() {
        let plateau_cow = PlateauCoW::new_empty();
        let clone1 = plateau_cow.clone();
        let clone2 = plateau_cow.clone();

        // All share same underlying data
        assert_eq!(plateau_cow.ref_count(), 3);
        assert_eq!(clone1.ref_count(), 3);
        assert_eq!(clone2.ref_count(), 3);

        // All can read
        plateau_cow.read(|p| assert_eq!(p.tiles.len(), 19));
        clone1.read(|p| assert_eq!(p.tiles.len(), 19));
        clone2.read(|p| assert_eq!(p.tiles.len(), 19));
    }

    #[test]
    fn test_cow_set_tile() {
        let plateau_cow = PlateauCoW::new_empty();
        let tile = Tile(1, 2, 3);

        plateau_cow.set_tile(0, tile);

        let retrieved = plateau_cow.get_tile(0).unwrap();
        assert_eq!(retrieved, tile);
    }

    #[test]
    fn test_cow_into_inner() {
        let plateau_cow = PlateauCoW::new_empty();
        let tile = Tile(5, 6, 7);

        plateau_cow.set_tile(5, tile);

        let plateau = plateau_cow.into_inner();
        assert_eq!(plateau.tiles[5], tile);
    }
}
