//! MCTS Node structures for Expectimax MCTS
//!
//! This module implements a two-level tree structure:
//! - Chance Nodes: represent stochastic tile draws
//! - Decision Nodes: represent deterministic position choices
//!
//! This properly models the stochastic nature of Take It Easy,
//! where tile draws are random but position placements are decisions.
//!
//! **Status**: Experimental code for Expectimax MCTS (Phase 3 testing)

#![allow(dead_code)]

use crate::game::deck::Deck;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::Plateau;
use crate::game::remove_tile_from_deck::replace_tile_in_deck;
use crate::game::tile::Tile;
use std::collections::HashMap;

/// Type of node in the Expectimax MCTS tree
#[derive(Debug, Clone)]
pub enum NodeType {
    /// Chance node: represents a random tile draw
    /// Contains all possible tiles that can be drawn and their probabilities
    Chance {
        available_tiles: Vec<Tile>,
        probabilities: Vec<f64>,
    },
    /// Decision node: represents choosing a position for a given tile
    /// Contains the tile to place and all legal positions
    Decision {
        tile: Tile,
        legal_positions: Vec<usize>,
    },
}

/// A node in the Expectimax MCTS tree
#[derive(Debug, Clone)]
pub struct MCTSNode {
    /// Type of this node (Chance or Decision)
    pub node_type: NodeType,

    /// Current board state
    pub plateau: Plateau,

    /// Remaining tiles in the deck
    pub deck: Deck,

    /// Number of times this node has been visited
    pub visit_count: usize,

    /// Sum of all values backpropagated through this node
    pub total_value: f64,

    /// Child nodes
    pub children: Vec<MCTSNode>,

    /// Index of parent node (if any) - used for backpropagation
    pub parent_index: Option<usize>,

    /// Current turn number (0-18)
    pub current_turn: usize,

    /// Total number of turns in the game
    pub total_turns: usize,
}

impl MCTSNode {
    /// Creates a new Chance Node
    ///
    /// # Arguments
    /// * `plateau` - Current board state
    /// * `deck` - Remaining tiles in the deck
    /// * `current_turn` - Current turn number
    /// * `total_turns` - Total turns in the game
    ///
    /// # Returns
    /// A Chance Node with uniform probabilities over available tiles
    pub fn new_chance_node(
        plateau: Plateau,
        deck: Deck,
        current_turn: usize,
        total_turns: usize,
    ) -> Self {
        let available_tiles = deck.tiles.clone();
        let total = available_tiles.len() as f64;

        // Uniform probability for each tile
        let probabilities: Vec<f64> = available_tiles
            .iter()
            .map(|_| 1.0 / total)
            .collect();

        MCTSNode {
            node_type: NodeType::Chance {
                available_tiles,
                probabilities,
            },
            plateau,
            deck,
            visit_count: 0,
            total_value: 0.0,
            children: Vec::new(),
            parent_index: None,
            current_turn,
            total_turns,
        }
    }

    /// Creates a new Decision Node
    ///
    /// # Arguments
    /// * `plateau` - Current board state
    /// * `deck` - Remaining tiles in the deck
    /// * `tile` - The tile to place
    /// * `current_turn` - Current turn number
    /// * `total_turns` - Total turns in the game
    ///
    /// # Returns
    /// A Decision Node for choosing where to place the given tile
    pub fn new_decision_node(
        plateau: Plateau,
        deck: Deck,
        tile: Tile,
        current_turn: usize,
        total_turns: usize,
    ) -> Self {
        let legal_positions = get_legal_moves(plateau.clone());

        MCTSNode {
            node_type: NodeType::Decision {
                tile,
                legal_positions,
            },
            plateau,
            deck,
            visit_count: 0,
            total_value: 0.0,
            children: Vec::new(),
            parent_index: None,
            current_turn,
            total_turns,
        }
    }

    /// Returns the average value of this node
    pub fn average_value(&self) -> f64 {
        if self.visit_count == 0 {
            0.0
        } else {
            self.total_value / self.visit_count as f64
        }
    }

    /// Checks if this node is fully expanded
    pub fn is_fully_expanded(&self) -> bool {
        match &self.node_type {
            NodeType::Chance { available_tiles, .. } => {
                self.children.len() >= available_tiles.len()
            }
            NodeType::Decision { legal_positions, .. } => {
                self.children.len() >= legal_positions.len()
            }
        }
    }

    /// Checks if this node is a leaf (no children)
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Checks if this is a terminal node (game over)
    pub fn is_terminal(&self) -> bool {
        self.current_turn >= self.total_turns || self.deck.tiles.is_empty()
    }

    /// Expands a Chance Node by creating Decision Node children for each possible tile
    ///
    /// # Panics
    /// Panics if called on a Decision Node
    pub fn expand_chance_node(&mut self) {
        if let NodeType::Chance { available_tiles, .. } = &self.node_type {
            for &tile in available_tiles {
                let child = MCTSNode::new_decision_node(
                    self.plateau.clone(),
                    self.deck.clone(),
                    tile,
                    self.current_turn,
                    self.total_turns,
                );
                self.children.push(child);
            }
        } else {
            panic!("expand_chance_node called on Decision Node");
        }
    }

    /// Expands a Decision Node by creating Chance Node children for each legal position
    ///
    /// # Panics
    /// Panics if called on a Chance Node
    pub fn expand_decision_node(&mut self) {
        if let NodeType::Decision { tile, legal_positions } = &self.node_type {
            for &position in legal_positions {
                // Apply the move
                let mut new_plateau = self.plateau.clone();
                let mut new_deck = self.deck.clone();

                new_plateau.tiles[position] = *tile;
                new_deck = replace_tile_in_deck(&new_deck, tile);

                // Create Chance Node for next tile draw
                let child = MCTSNode::new_chance_node(
                    new_plateau,
                    new_deck,
                    self.current_turn + 1,
                    self.total_turns,
                );
                self.children.push(child);
            }
        } else {
            panic!("expand_decision_node called on Chance Node");
        }
    }

    /// Expands this node by one child (for progressive widening)
    ///
    /// Returns true if a child was added, false if already fully expanded
    pub fn expand_one_child(&mut self) -> bool {
        if self.is_fully_expanded() {
            return false;
        }

        match &self.node_type {
            NodeType::Chance { available_tiles, .. } => {
                let next_index = self.children.len();
                if next_index < available_tiles.len() {
                    let tile = available_tiles[next_index];
                    let child = MCTSNode::new_decision_node(
                        self.plateau.clone(),
                        self.deck.clone(),
                        tile,
                        self.current_turn,
                        self.total_turns,
                    );
                    self.children.push(child);
                    true
                } else {
                    false
                }
            }
            NodeType::Decision { tile, legal_positions } => {
                let next_index = self.children.len();
                if next_index < legal_positions.len() {
                    let position = legal_positions[next_index];

                    let mut new_plateau = self.plateau.clone();
                    let mut new_deck = self.deck.clone();
                    new_plateau.tiles[position] = *tile;
                    new_deck = replace_tile_in_deck(&new_deck, tile);

                    let child = MCTSNode::new_chance_node(
                        new_plateau,
                        new_deck,
                        self.current_turn + 1,
                        self.total_turns,
                    );
                    self.children.push(child);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Gets statistics about this node for debugging
    pub fn stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();

        stats.insert("type".to_string(), match &self.node_type {
            NodeType::Chance { .. } => "Chance".to_string(),
            NodeType::Decision { .. } => "Decision".to_string(),
        });

        stats.insert("visits".to_string(), self.visit_count.to_string());
        stats.insert("avg_value".to_string(), format!("{:.3}", self.average_value()));
        stats.insert("children".to_string(), self.children.len().to_string());
        stats.insert("turn".to_string(), format!("{}/{}", self.current_turn, self.total_turns));

        match &self.node_type {
            NodeType::Chance { available_tiles, .. } => {
                stats.insert("tiles_available".to_string(), available_tiles.len().to_string());
            }
            NodeType::Decision { legal_positions, tile } => {
                stats.insert("positions_available".to_string(), legal_positions.len().to_string());
                stats.insert("tile".to_string(), format!("({},{},{})", tile.0, tile.1, tile.2));
            }
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::plateau::create_plateau_empty;

    fn create_test_deck() -> Deck {
        Deck {
            tiles: vec![
                Tile(1, 5, 9),
                Tile(2, 6, 7),
                Tile(3, 4, 8),
            ],
        }
    }

    #[test]
    fn test_new_chance_node() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let node = MCTSNode::new_chance_node(plateau, deck, 0, 19);

        assert!(matches!(node.node_type, NodeType::Chance { .. }));
        assert_eq!(node.visit_count, 0);
        assert_eq!(node.total_value, 0.0);
        assert_eq!(node.children.len(), 0);

        if let NodeType::Chance { available_tiles, probabilities } = &node.node_type {
            assert_eq!(available_tiles.len(), 3);
            assert_eq!(probabilities.len(), 3);
            assert!((probabilities[0] - 1.0/3.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_new_decision_node() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let tile = Tile(1, 5, 9);
        let node = MCTSNode::new_decision_node(plateau, deck, tile, 0, 19);

        assert!(matches!(node.node_type, NodeType::Decision { .. }));
        assert_eq!(node.visit_count, 0);
        assert_eq!(node.children.len(), 0);

        if let NodeType::Decision { tile: t, legal_positions } = &node.node_type {
            assert_eq!(*t, tile);
            assert_eq!(legal_positions.len(), 19); // All positions empty
        }
    }

    #[test]
    fn test_expand_chance_node() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let mut node = MCTSNode::new_chance_node(plateau, deck, 0, 19);

        assert_eq!(node.children.len(), 0);
        assert!(!node.is_fully_expanded());

        node.expand_chance_node();

        assert_eq!(node.children.len(), 3);
        assert!(node.is_fully_expanded());

        for child in &node.children {
            assert!(matches!(child.node_type, NodeType::Decision { .. }));
        }
    }

    #[test]
    fn test_expand_decision_node() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let tile = Tile(1, 5, 9);
        let mut node = MCTSNode::new_decision_node(plateau, deck, tile, 0, 19);

        assert_eq!(node.children.len(), 0);
        assert!(!node.is_fully_expanded());

        node.expand_decision_node();

        assert_eq!(node.children.len(), 19); // All positions legal
        assert!(node.is_fully_expanded());

        for child in &node.children {
            assert!(matches!(child.node_type, NodeType::Chance { .. }));
            assert_eq!(child.current_turn, 1); // Turn incremented
        }
    }

    #[test]
    fn test_expand_one_child() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let mut node = MCTSNode::new_chance_node(plateau, deck, 0, 19);

        assert_eq!(node.children.len(), 0);

        // Expand first child
        assert!(node.expand_one_child());
        assert_eq!(node.children.len(), 1);

        // Expand second child
        assert!(node.expand_one_child());
        assert_eq!(node.children.len(), 2);

        // Expand third child
        assert!(node.expand_one_child());
        assert_eq!(node.children.len(), 3);

        // Already fully expanded
        assert!(!node.expand_one_child());
        assert_eq!(node.children.len(), 3);
    }

    #[test]
    fn test_average_value() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let mut node = MCTSNode::new_chance_node(plateau, deck, 0, 19);

        assert_eq!(node.average_value(), 0.0);

        node.visit_count = 10;
        node.total_value = 75.0;

        assert!((node.average_value() - 7.5).abs() < 1e-6);
    }

    #[test]
    fn test_is_terminal() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();

        let node_early = MCTSNode::new_chance_node(plateau.clone(), deck.clone(), 5, 19);
        assert!(!node_early.is_terminal());

        let node_last = MCTSNode::new_chance_node(plateau.clone(), deck.clone(), 19, 19);
        assert!(node_last.is_terminal());

        let empty_deck = Deck { tiles: vec![] };
        let node_empty_deck = MCTSNode::new_chance_node(plateau, empty_deck, 5, 19);
        assert!(node_empty_deck.is_terminal());
    }
}
