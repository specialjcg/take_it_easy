# Expectimax MCTS Implementation Plan

## Executive Summary

Implement Expectimax MCTS to properly model the stochastic tile drawing in Take It Easy, replacing the current MCTS algorithm which doesn't correctly handle randomness. Expected gain: **+4-7 pts → 143-146 pts**.

## Current MCTS Analysis

### Structure Actuelle (`src/mcts/algorithm.rs`)

```rust
fn mcts_core(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,  // ← Tuile DÉJÀ tirée
    evaluator: MctsEvaluator<'_>,
    num_simulations: usize,
    ...
) -> MCTSResult
```

**Problème** : Le MCTS actuel reçoit la tuile **DÉJÀ tirée** (`chosen_tile`). Il ne modélise PAS le tirage aléatoire.

### Flux Actuel

```
1. Tuile tirée aléatoirement (AVANT MCTS)
2. MCTS explore les 19 positions possibles
3. Pour chaque position :
   - Évalue avec CNN
   - Fait des simulations
   - Calcule UCB
4. Retourne meilleure position
```

**Limitation** : Le MCTS optimise pour UNE tuile donnée, mais ne tient pas compte de l'aléa du tirage suivant.

---

## Expectimax MCTS Architecture

### Nouveau Flux

```
1. MCTS démarre avec état plateau + deck (tuiles disponibles)
2. Chance Node : Modélise TOUS les tirages possibles
3. Pour chaque tirage possible (pondéré par probabilité) :
   - Decision Node : Explore les 19 positions
   - Évalue avec CNN
   - Calcule Expectimax (moyenne pondérée)
4. Retourne meilleure position MOYENNE sur tous les tirages
```

### Différence Clé

```
MCTS Actuel :
Tire Tile(1,5,9) → Explore 19 positions → Meilleure position pour Tile(1,5,9)

Expectimax MCTS :
Pour chaque tuile possible dans deck :
  - P(Tile(1,5,9)) = 3/27
  - P(Tile(2,6,7)) = 3/27
  - ...
Calcule : E[score] = Σ P(tuile) × meilleur_score(tuile)
```

---

## Implémentation Phase 1 : Chance Nodes

### Étape 1.1 : Nouveau Type de Nœud

**Fichier** : `src/mcts/node.rs` (NOUVEAU)

```rust
/// Type de nœud dans l'arbre Expectimax MCTS
#[derive(Debug, Clone)]
pub enum NodeType {
    /// Nœud de chance : représente un tirage aléatoire de tuile
    Chance {
        available_tiles: Vec<Tile>,
        probabilities: Vec<f64>,  // Probabilité de chaque tuile
    },
    /// Nœud de décision : choix de position pour une tuile donnée
    Decision {
        tile: Tile,
        legal_positions: Vec<usize>,
    },
}

#[derive(Debug, Clone)]
pub struct MCTSNode {
    pub node_type: NodeType,
    pub plateau: Plateau,
    pub deck: Deck,
    pub visit_count: usize,
    pub total_value: f64,
    pub children: Vec<MCTSNode>,
    pub parent_index: Option<usize>,
}

impl MCTSNode {
    /// Crée un Chance Node
    pub fn new_chance_node(plateau: Plateau, deck: Deck) -> Self {
        let available_tiles = deck.tiles.clone();
        let total = available_tiles.len() as f64;
        let probabilities = available_tiles.iter().map(|_| 1.0 / total).collect();

        MCTSNode {
            node_type: NodeType::Chance { available_tiles, probabilities },
            plateau,
            deck,
            visit_count: 0,
            total_value: 0.0,
            children: Vec::new(),
            parent_index: None,
        }
    }

    /// Crée un Decision Node
    pub fn new_decision_node(plateau: Plateau, deck: Deck, tile: Tile) -> Self {
        let legal_positions = get_legal_moves(plateau.clone());

        MCTSNode {
            node_type: NodeType::Decision { tile, legal_positions },
            plateau,
            deck,
            visit_count: 0,
            total_value: 0.0,
            children: Vec::new(),
            parent_index: None,
        }
    }

    /// Valeur moyenne du nœud
    pub fn average_value(&self) -> f64 {
        if self.visit_count == 0 {
            0.0
        } else {
            self.total_value / self.visit_count as f64
        }
    }
}
```

### Étape 1.2 : Expansion des Chance Nodes

```rust
impl MCTSNode {
    /// Expand un Chance Node en créant un Decision Node par tuile possible
    pub fn expand_chance_node(&mut self) {
        if let NodeType::Chance { available_tiles, .. } = &self.node_type {
            for tile in available_tiles {
                let child = MCTSNode::new_decision_node(
                    self.plateau.clone(),
                    self.deck.clone(),
                    *tile
                );
                self.children.push(child);
            }
        }
    }

    /// Expand un Decision Node en créant un Chance Node par position possible
    pub fn expand_decision_node(&mut self) {
        if let NodeType::Decision { tile, legal_positions } = &self.node_type {
            for &position in legal_positions {
                let mut new_plateau = self.plateau.clone();
                let mut new_deck = self.deck.clone();

                // Jouer le coup
                new_plateau.tiles[position] = *tile;
                new_deck = replace_tile_in_deck(&new_deck, tile);

                // Créer Chance Node pour le prochain tirage
                let child = MCTSNode::new_chance_node(new_plateau, new_deck);
                self.children.push(child);
            }
        }
    }
}
```

---

## Implémentation Phase 2 : Expectimax Selection

### Étape 2.1 : Sélection avec Expectimax

**Fichier** : `src/mcts/selection.rs` (NOUVEAU)

```rust
use crate::mcts::node::{MCTSNode, NodeType};

/// Sélectionne le meilleur enfant selon Expectimax
pub fn select_best_child(node: &MCTSNode, c_puct: f64) -> Option<usize> {
    match &node.node_type {
        NodeType::Chance { probabilities, .. } => {
            // Pour Chance Node : sélection pondérée par probabilité
            select_chance_child(node, probabilities)
        }
        NodeType::Decision { .. } => {
            // Pour Decision Node : UCB1 classique
            select_decision_child(node, c_puct)
        }
    }
}

/// Sélection pour Chance Node (pondérée par probabilité)
fn select_chance_child(node: &MCTSNode, probabilities: &[f64]) -> Option<usize> {
    if node.children.is_empty() {
        return None;
    }

    let total_visits = node.visit_count as f64;
    let mut best_score = f64::NEG_INFINITY;
    let mut best_index = 0;

    for (i, child) in node.children.iter().enumerate() {
        // Score = valeur moyenne + bonus exploration pondéré par probabilité
        let avg_value = child.average_value();
        let exploration = (total_visits.ln() / (child.visit_count + 1) as f64).sqrt();
        let probability_weight = probabilities[i];

        let score = avg_value + probability_weight * exploration;

        if score > best_score {
            best_score = score;
            best_index = i;
        }
    }

    Some(best_index)
}

/// Sélection pour Decision Node (UCB1 standard)
fn select_decision_child(node: &MCTSNode, c_puct: f64) -> Option<usize> {
    if node.children.is_empty() {
        return None;
    }

    let total_visits = node.visit_count as f64;
    let mut best_ucb = f64::NEG_INFINITY;
    let mut best_index = 0;

    for (i, child) in node.children.iter().enumerate() {
        let avg_value = child.average_value();
        let exploration = c_puct * (total_visits.ln() / (child.visit_count + 1) as f64).sqrt();
        let ucb = avg_value + exploration;

        if ucb > best_ucb {
            best_ucb = ucb;
            best_index = i;
        }
    }

    Some(best_index)
}
```

### Étape 2.2 : Backpropagation avec Expectimax

```rust
/// Backpropage la valeur du nœud feuille vers la racine
pub fn backpropagate(nodes: &mut [MCTSNode], leaf_index: usize, value: f64) {
    let mut current_index = leaf_index;

    loop {
        let node = &mut nodes[current_index];
        node.visit_count += 1;

        // Pour Chance Node : espérance pondérée
        // Pour Decision Node : valeur directe
        let weighted_value = match &node.node_type {
            NodeType::Chance { probabilities, .. } => {
                // Calculer espérance sur les enfants
                let mut expectation = 0.0;
                for (i, child) in node.children.iter().enumerate() {
                    expectation += probabilities[i] * child.average_value();
                }
                expectation
            }
            NodeType::Decision { .. } => value,
        };

        node.total_value += weighted_value;

        // Remonter au parent
        match node.parent_index {
            Some(parent_idx) => current_index = parent_idx,
            None => break,  // Racine atteinte
        }
    }
}
```

---

## Implémentation Phase 3 : Intégration CNN

### Étape 3.1 : Évaluation avec CNN dans Decision Nodes

**Fichier** : `src/mcts/expectimax_algorithm.rs` (NOUVEAU)

```rust
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::mcts::node::{MCTSNode, NodeType};

pub struct ExpectimaxMCTS<'a> {
    policy_net: &'a PolicyNet,
    value_net: &'a ValueNet,
    c_puct: f64,
}

impl<'a> ExpectimaxMCTS<'a> {
    pub fn new(policy_net: &'a PolicyNet, value_net: &'a ValueNet, c_puct: f64) -> Self {
        Self { policy_net, value_net, c_puct }
    }

    /// Évalue un Decision Node avec le CNN
    pub fn evaluate_decision_node(&self, node: &MCTSNode) -> f64 {
        if let NodeType::Decision { tile, .. } = &node.node_type {
            // Créer tenseur pour le CNN
            let board_tensor = convert_plateau_to_tensor(
                &node.plateau,
                tile,
                &node.deck,
                0,  // current_turn (à passer en paramètre)
                19, // total_turns
            );

            // Prédire valeur avec ValueNet
            let value = self.value_net
                .forward(&board_tensor, false)
                .double_value(&[])
                .clamp(-1.0, 1.0);

            value
        } else {
            0.0  // Chance Node n'est pas évalué directement
        }
    }

    /// Simulation MCTS complète
    pub fn run_simulation(&mut self, root: &mut MCTSNode, num_simulations: usize) {
        for _ in 0..num_simulations {
            // 1. Sélection : descendre dans l'arbre
            let leaf_index = self.select_leaf(root);

            // 2. Expansion : créer nouveaux enfants
            self.expand_node(&mut root, leaf_index);

            // 3. Simulation : évaluer avec CNN
            let value = self.evaluate_decision_node(&root);

            // 4. Backpropagation : remonter la valeur
            backpropagate(&mut [root.clone()], leaf_index, value);
        }
    }

    /// Sélectionne un nœud feuille en descendant l'arbre
    fn select_leaf(&self, root: &MCTSNode) -> usize {
        let mut current = root.clone();
        let mut path = vec![0];  // Index du nœud courant

        loop {
            if current.children.is_empty() {
                return *path.last().unwrap();
            }

            match select_best_child(&current, self.c_puct) {
                Some(child_idx) => {
                    current = current.children[child_idx].clone();
                    path.push(child_idx);
                }
                None => return *path.last().unwrap(),
            }
        }
    }
}
```

---

## Implémentation Phase 4 : Variance Reduction

### Technique 1 : Sampling Importance Reweighting

```rust
/// Réduit la variance en pondérant les échantillons par leur probabilité
pub fn importance_reweighting(samples: &[(Tile, f64, f64)]) -> f64 {
    // samples = [(tile, value, probability)]
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;

    for (_, value, prob) in samples {
        weighted_sum += value * prob;
        weight_sum += prob;
    }

    if weight_sum > 0.0 {
        weighted_sum / weight_sum
    } else {
        0.0
    }
}
```

### Technique 2 : Progressive Widening

```rust
/// Limite le nombre d'enfants explorés pour réduire la variance
pub fn progressive_widening(node: &mut MCTSNode, alpha: f64, k: f64) {
    let max_children = (k * (node.visit_count as f64).powf(alpha)) as usize;

    match &node.node_type {
        NodeType::Chance { available_tiles, .. } => {
            // Ne crée des enfants que pour les k meilleures tuiles
            if node.children.len() < max_children && node.children.len() < available_tiles.len() {
                // Expand un nouvel enfant
                node.expand_chance_node();
            }
        }
        NodeType::Decision { legal_positions, .. } => {
            // Ne crée des enfants que pour les k meilleures positions
            if node.children.len() < max_children && node.children.len() < legal_positions.len() {
                node.expand_decision_node();
            }
        }
    }
}
```

---

## Timeline d'Implémentation

### Semaine 1 : Chance Nodes & Structure

**Jours 1-2** : Créer `src/mcts/node.rs`
- `NodeType` enum
- `MCTSNode` struct
- Tests unitaires

**Jours 3-4** : Expansion
- `expand_chance_node()`
- `expand_decision_node()`
- Tests unitaires

**Jour 5** : Revue de code & refactoring

### Semaine 2 : Expectimax Selection

**Jours 6-7** : Créer `src/mcts/selection.rs`
- `select_chance_child()`
- `select_decision_child()`
- Tests unitaires

**Jours 8-9** : Backpropagation
- `backpropagate()` avec expectation
- Tests unitaires

**Jour 10** : Revue de code

### Semaine 3 : Intégration & Benchmarking

**Jours 11-12** : Créer `src/mcts/expectimax_algorithm.rs`
- `ExpectimaxMCTS` struct
- Intégration CNN
- `run_simulation()`

**Jour 13** : Variance reduction
- Progressive widening
- Importance reweighting

**Jours 14-15** : Tests & benchmarking
- Test 10 games
- Benchmark 50 games × 150 sims
- Comparaison vs baseline (139.40 pts)

---

## Fichiers à Créer/Modifier

### Nouveaux Fichiers

1. `src/mcts/node.rs` - Structure des nœuds
2. `src/mcts/selection.rs` - Logique Expectimax
3. `src/mcts/expectimax_algorithm.rs` - Algorithme principal
4. `src/mcts/variance_reduction.rs` - Techniques de réduction de variance

### Fichiers à Modifier

1. `src/mcts/mod.rs` - Ajouter nouveaux modules
2. `src/mcts/algorithm.rs` - Ajouter flag `--use-expectimax`
3. `src/bin/compare_mcts.rs` - Ajouter option CLI

---

## Tests de Validation

### Test 1 : Chance Node Expansion

```rust
#[test]
fn test_chance_node_expansion() {
    let plateau = create_plateau_empty();
    let deck = create_full_deck();
    let mut node = MCTSNode::new_chance_node(plateau, deck);

    node.expand_chance_node();

    assert_eq!(node.children.len(), 27);  // 27 tuiles possibles
}
```

### Test 2 : Expectimax Selection

```rust
#[test]
fn test_expectimax_selection() {
    let mut root = create_test_tree();
    let child_idx = select_best_child(&root, 1.4);

    assert!(child_idx.is_some());
    assert!(child_idx.unwrap() < root.children.len());
}
```

### Test 3 : Benchmark vs Baseline

```bash
# Baseline (Pattern Rollouts V2)
cargo run --release --bin compare_mcts -- -g 50 -s 150 --nn-architecture cnn

# Expectimax MCTS
cargo run --release --bin compare_mcts -- -g 50 -s 150 --nn-architecture cnn --use-expectimax
```

**Critère de succès** : Score moyen ≥ 143 pts (+3.6 pts vs baseline)

---

## Risques & Mitigation

### Risque 1 : Explosion Combinatoire

**Problème** : 27 tuiles × 19 positions = 513 branches par coup

**Mitigation** :
- Progressive widening (limiter à k=5 meilleures tuiles)
- Pruning basé sur CNN value

### Risque 2 : Temps de Calcul

**Problème** : Expectimax peut être plus lent que MCTS classique

**Mitigation** :
- Caching des évaluations CNN
- Parallelisation des simulations
- Limiter profondeur d'arbre

### Risque 3 : Pas de Gain

**Problème** : Expectimax pourrait ne pas améliorer le score

**Mitigation** :
- Tests incrémentaux (valider chaque phase)
- Ablation study (tester avec/sans variance reduction)
- Fallback vers MCTS classique si régression

---

## Métriques de Succès

| Métrique | Baseline | Target Expectimax | Stretch Goal |
|----------|----------|-------------------|--------------|
| Score moyen | 139.40 | ≥ 143.0 (+3.6) | ≥ 146.0 (+6.6) |
| Écart-type | ~15 | ≤ 15 | ≤ 12 |
| Temps/coup | ~200ms | ≤ 300ms | ≤ 250ms |

---

## Prochaines Étapes

1. ✅ Plan d'implémentation créé
2. ⏳ Créer `src/mcts/node.rs`
3. ⏳ Implémenter Chance Nodes
4. ⏳ Implémenter Expectimax selection
5. ⏳ Intégrer CNN
6. ⏳ Benchmarker

**Début d'implémentation** : Maintenant
**Date cible** : 3 semaines (mi-novembre 2025)
