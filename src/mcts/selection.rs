//! Selection strategies for Expectimax MCTS
//!
//! This module implements selection policies for both Chance and Decision nodes:
//! - Chance nodes: probability-weighted exploration
//! - Decision nodes: UCB1 (Upper Confidence Bound) formula
//!
//! The key difference from standard MCTS is that Chance nodes use expectation
//! over tile probabilities instead of maximization.
//!
//! **Status**: Experimental code for Expectimax MCTS (Phase 3 testing)

#![allow(dead_code)]

use crate::mcts::node::{MCTSNode, NodeType};

/// Selects the best child node using the appropriate strategy
///
/// # Arguments
/// * `node` - The parent node
/// * `c_puct` - Exploration constant for UCB (used only for Decision nodes)
///
/// # Returns
/// Index of the best child to explore, or None if no children exist
pub fn select_best_child(node: &MCTSNode, c_puct: f64) -> Option<usize> {
    if node.children.is_empty() {
        return None;
    }

    match &node.node_type {
        NodeType::Chance { probabilities, .. } => select_chance_child(node, probabilities, c_puct),
        NodeType::Decision { .. } => select_decision_child(node, c_puct),
    }
}

/// Selects a child from a Chance Node using probability-weighted UCB
///
/// Formula: value + P(tile) × sqrt(ln(N_parent) / (1 + N_child))
///
/// # Arguments
/// * `node` - The Chance node
/// * `probabilities` - Probability of each tile
/// * `exploration_weight` - Weight for exploration bonus
///
/// # Returns
/// Index of the child with highest weighted score
fn select_chance_child(
    node: &MCTSNode,
    probabilities: &[f64],
    exploration_weight: f64,
) -> Option<usize> {
    if node.children.is_empty() || probabilities.is_empty() {
        return None;
    }

    let parent_visits = node.visit_count as f64;
    let mut best_score = f64::NEG_INFINITY;
    let mut best_index = 0;

    for (i, child) in node.children.iter().enumerate() {
        // Get probability for this tile (default to uniform if out of bounds)
        let probability = if i < probabilities.len() {
            probabilities[i]
        } else {
            1.0 / node.children.len() as f64
        };

        // Average value from this child
        let avg_value = child.average_value();

        // Exploration bonus weighted by probability
        // Higher probability tiles get more exploration
        let exploration_bonus = if parent_visits > 0.0 && child.visit_count > 0 {
            let exploration = (parent_visits.ln() / child.visit_count as f64).sqrt();
            probability * exploration_weight * exploration
        } else {
            // Unvisited nodes get high bonus
            probability * exploration_weight * 100.0
        };

        let score = avg_value + exploration_bonus;

        if score > best_score {
            best_score = score;
            best_index = i;
        }
    }

    Some(best_index)
}

/// Selects a child from a Decision Node using UCB1
///
/// Formula: value + c_puct × sqrt(ln(N_parent) / (1 + N_child))
///
/// # Arguments
/// * `node` - The Decision node
/// * `c_puct` - Exploration constant (typically 1.4 - 2.0)
///
/// # Returns
/// Index of the child with highest UCB score
fn select_decision_child(node: &MCTSNode, c_puct: f64) -> Option<usize> {
    if node.children.is_empty() {
        return None;
    }

    let parent_visits = node.visit_count as f64;
    let mut best_ucb = f64::NEG_INFINITY;
    let mut best_index = 0;

    for (i, child) in node.children.iter().enumerate() {
        let avg_value = child.average_value();

        // UCB1 formula
        let exploration = if parent_visits > 0.0 && child.visit_count > 0 {
            c_puct * (parent_visits.ln() / child.visit_count as f64).sqrt()
        } else {
            // Unvisited nodes get very high UCB
            c_puct * 1000.0
        };

        let ucb = avg_value + exploration;

        if ucb > best_ucb {
            best_ucb = ucb;
            best_index = i;
        }
    }

    Some(best_index)
}

/// Backpropagates a value from a leaf node to the root
///
/// For Chance nodes: updates with expected value weighted by child probabilities
/// For Decision nodes: updates with raw value
///
/// # Arguments
/// * `node` - The node to update (will recursively update parents)
/// * `value` - The value to backpropagate
pub fn backpropagate(node: &mut MCTSNode, value: f64) {
    node.visit_count += 1;

    // Update total value based on node type
    let weighted_value = match &node.node_type {
        NodeType::Chance { probabilities, .. } => {
            // For Chance nodes: calculate expectation over children
            if node.children.is_empty() {
                value
            } else {
                let mut expectation = 0.0;
                for (i, child) in node.children.iter().enumerate() {
                    let prob = if i < probabilities.len() {
                        probabilities[i]
                    } else {
                        1.0 / node.children.len() as f64
                    };
                    expectation += prob * child.average_value();
                }
                expectation
            }
        }
        NodeType::Decision { .. } => {
            // For Decision nodes: use value directly
            value
        }
    };

    node.total_value += weighted_value;
}

/// Performs a full tree traversal from root to leaf using selection
///
/// # Arguments
/// * `root` - The root node
/// * `c_puct` - Exploration constant
///
/// # Returns
/// Path from root to selected leaf (indices of children at each level)
pub fn select_leaf_path(root: &MCTSNode, c_puct: f64) -> Vec<usize> {
    let mut path = Vec::new();
    let mut current = root;

    loop {
        // If leaf or terminal, stop
        if current.is_leaf() || current.is_terminal() {
            break;
        }

        // Select best child
        match select_best_child(current, c_puct) {
            Some(child_idx) => {
                path.push(child_idx);
                current = &current.children[child_idx];
            }
            None => break,
        }
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::deck::Deck;
    use crate::game::plateau::create_plateau_empty;
    use crate::game::tile::Tile;

    fn create_test_deck() -> Deck {
        Deck {
            tiles: vec![Tile(1, 5, 9), Tile(2, 6, 7), Tile(3, 4, 8)],
        }
    }

    #[test]
    fn test_select_best_child_empty() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let node = MCTSNode::new_chance_node(plateau, deck, 0, 19);

        assert_eq!(select_best_child(&node, 1.4), None);
    }

    #[test]
    fn test_select_chance_child_unvisited() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let mut node = MCTSNode::new_chance_node(plateau, deck, 0, 19);

        node.expand_chance_node();
        assert_eq!(node.children.len(), 3);

        // Should select first child (all unvisited, equal probability)
        let selected = select_best_child(&node, 1.4);
        assert!(selected.is_some());
        assert!(selected.unwrap() < 3);
    }

    #[test]
    fn test_select_decision_child_ucb() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let tile = Tile(1, 5, 9);
        let mut node = MCTSNode::new_decision_node(plateau, deck, tile, 0, 19);

        node.expand_decision_node();
        node.visit_count = 10;

        // Manually set different visit counts for children
        node.children[0].visit_count = 5;
        node.children[0].total_value = 25.0; // avg = 5.0

        node.children[1].visit_count = 2;
        node.children[1].total_value = 10.0; // avg = 5.0

        // Child 1 should be selected (lower visit count → higher exploration)
        let selected = select_best_child(&node, 1.4).unwrap();
        // Could be 1 or another unvisited child with even higher UCB
        assert!(selected < node.children.len());
    }

    #[test]
    fn test_backpropagate_decision_node() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let tile = Tile(1, 5, 9);
        let mut node = MCTSNode::new_decision_node(plateau, deck, tile, 0, 19);

        assert_eq!(node.visit_count, 0);
        assert_eq!(node.total_value, 0.0);

        backpropagate(&mut node, 10.0);

        assert_eq!(node.visit_count, 1);
        assert_eq!(node.total_value, 10.0);
        assert!((node.average_value() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_backpropagate_chance_node() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let mut node = MCTSNode::new_chance_node(plateau, deck, 0, 19);

        node.expand_chance_node();

        // Set values for children
        node.children[0].visit_count = 1;
        node.children[0].total_value = 6.0; // avg = 6.0

        node.children[1].visit_count = 1;
        node.children[1].total_value = 9.0; // avg = 9.0

        node.children[2].visit_count = 1;
        node.children[2].total_value = 12.0; // avg = 12.0

        backpropagate(&mut node, 0.0);

        assert_eq!(node.visit_count, 1);
        // Expectation: (6 + 9 + 12) / 3 = 9.0
        assert!((node.total_value - 9.0).abs() < 1e-6);
    }

    #[test]
    fn test_select_leaf_path() {
        let plateau = create_plateau_empty();
        let deck = create_test_deck();
        let mut root = MCTSNode::new_chance_node(plateau, deck, 0, 19);

        // Empty tree → empty path
        let path = select_leaf_path(&root, 1.4);
        assert!(path.is_empty());

        // Expand root
        root.expand_chance_node();
        root.visit_count = 10;

        // Path should go to one of the children
        let path = select_leaf_path(&root, 1.4);
        assert_eq!(path.len(), 1);
        assert!(path[0] < 3);
    }
}
