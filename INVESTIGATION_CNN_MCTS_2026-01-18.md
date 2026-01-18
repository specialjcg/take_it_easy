# Investigation CNN vs MCTS - Rapport Consolidé

**Date**: 2026-01-18
**Statut**: En cours - CNN contourné, prochaines étapes identifiées

---

## Résumé Exécutif

Le CNN détruisait les performances MCTS (12 pts vs 100+ pts). Après investigation approfondie, nous avons identifié **3 causes racines** et appliqué des corrections qui ramènent MCTS+NN à 99.4 pts.

| Métrique | Avant Fix | Après Fix |
|----------|-----------|-----------|
| Pure MCTS | ~100 pts | 103.3 pts |
| MCTS + CNN | **12 pts** | **99.4 pts** |
| Delta | -88 pts | -3.9 pts |

---

## Problème 1: Géométrie Cassée dans l'Encodage Tenseur

### Découverte
L'outil `debug_geometry.rs` a révélé que **seulement 5 sur 15 lignes de scoring forment des lignes DROITES** dans la grille tenseur 5×5.

```
SCORING LINES IN TENSOR GRID:
Dir1-Row0 [0, 1, 2]     → (1,0)→(2,0)→(3,0)     ✓ STRAIGHT (vertical)
Dir1-Row1 [3, 4, 5, 6]  → (1,1)→(2,1)→(3,1)→(4,1) ✓ STRAIGHT
...
Dir2-Diag0 [0, 3, 7]    → (1,0)→(1,1)→(0,2)     ✗ BROKEN (zigzag!)
Dir2-Diag1 [1, 4, 8, 12] → (2,0)→(2,1)→(1,2)→(1,3) ✗ BROKEN
...
SUMMARY: 5 straight lines, 10 broken lines
```

### Impact
- Les convolutions CNN ne peuvent **pas** détecter les patterns Dir2 et Dir3
- Le CNN est essentiellement aveugle à 2/3 de la géométrie du jeu
- Impossible d'apprendre les patterns de scoring correctement

### Solution Appliquée
Ajout de **30 canaux de features de lignes explicites** (47 canaux total):
- Channels 17-46: 15 lignes × 2 features (potentiel + compatibilité tile)
- Le CNN reçoit directement l'information sur l'état des lignes

**Fichiers modifiés**:
- `src/neural/tensor_conversion.rs`: CHANNELS = 47, fonction `compute_line_features()`
- `src/bin/supervised_trainer_csv.rs`: Encodage mis à jour
- `src/neural/manager.rs`: Default input_dim = (47, 5, 5)

---

## Problème 2: CNN Polluant les Estimations de Valeur

### Découverte
Le chemin `MctsEvaluator::Neural` calculait `value_estimates` à partir des prédictions CNN:

```rust
// AVANT (problématique):
let pred_value = value_net.forward(&board_tensor_temp, false);
value_estimates.insert(position, pred_value);
```

Alors que le chemin `MctsEvaluator::Pure` utilisait des rollouts réels:

```rust
// Pure (correct):
let avg_score = simulate_games_smart(...);
value_estimates.insert(position, avg_score);
```

### Impact
- Les mauvaises prédictions CNN contaminaient toute la sélection MCTS
- Même avec w_cnn=0, les value_estimates restaient fausses

### Solution Appliquée
Remplacer les prédictions CNN par des rollouts dans le chemin Neural:

```rust
// APRÈS (corrigé):
let mut total_simulated_score = 0.0;
for _ in 0..rollout_count {
    total_simulated_score += simulate_games_smart(...) as f64;
}
let value = ((avg_score / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;
value_estimates.insert(position, value);
```

**Fichier modifié**: `src/mcts/algorithm.rs` (lignes ~822-852)

---

## Problème 3: Filtrage et Tri des Moves par CNN

### Découverte
Les moves étaient filtrés et triés par CNN AVANT les rollouts:

```rust
// AVANT:
let mut moves_with_prior: Vec<_> = legal_moves
    .iter()
    .filter(|&&pos| value_estimates[&pos] >= value_threshold) // CNN filter!
    .map(|&pos| (pos, policy.i((0, pos as i64)).double_value(&[]))) // CNN prior!
    .collect();
moves_with_prior.sort_by(...); // Sort by CNN policy
let subset_moves = moves_with_prior.take(top_k); // Only top K
```

### Impact
- Les bons moves étaient éliminés avant d'être évalués par rollouts
- Le CNN contrôlait quels moves MCTS pouvait même considérer

### Solution Appliquée
Désactiver le filtrage/tri CNN:

```rust
// APRÈS:
let subset_moves: Vec<usize> = legal_moves.clone();
```

**Fichier modifié**: `src/mcts/algorithm.rs` (lignes ~484-500 et ~995-1010)

---

## Problème 4: Poids CNN Trop Élevés

### Découverte
Même après les corrections ci-dessus, les poids adaptatifs donnaient encore trop d'influence au CNN.

### Solution Appliquée
Désactivation complète des poids CNN:

```rust
// src/mcts/hyperparameters.rs
weight_cnn: 0.00,        // CNN DISABLED
weight_rollout: 0.90,    // Rollouts primary

// get_turn_adaptive_weights()
(0.00, 0.90) // pour tous les tours
```

---

## État Actuel du Code

### Modifications Principales

| Fichier | Modification |
|---------|--------------|
| `tensor_conversion.rs` | 47 canaux avec features de lignes explicites |
| `algorithm.rs` | Rollouts pour value_estimates, pas de filtrage CNN |
| `hyperparameters.rs` | w_cnn=0.00 partout |
| `manager.rs` | Default input_dim=(47,5,5) |
| `supervised_trainer_csv.rs` | Encodage 47 canaux |
| `compare_mcts.rs` | Config 47 canaux |

### Fichiers de Debug Créés
- `src/bin/debug_geometry.rs`: Visualisation de la géométrie tenseur vs scoring

---

## Prochaines Étapes Recommandées

### Option A: Améliorer l'Architecture CNN
1. Ajouter des couches de convolution asymétriques (1×5 pour lignes verticales)
2. Utiliser des convolutions dilatées pour capturer les patterns zigzag
3. Ajouter des skip connections entre les features de lignes et la sortie

### Option B: Passer à une Architecture GNN
1. Les GNN respectent naturellement la topologie hexagonale
2. Message passing le long des 15 lignes de scoring
3. Plus adapté à la structure du problème

```
Nodes: 19 positions hexagonales
Edges: Connexions basées sur les lignes de scoring
Features: Valeurs des tuiles + état des lignes
```

### Option C: Architecture Attention/Transformer
1. Self-attention entre positions
2. Attention guidée par la géométrie des lignes
3. Peut capturer les dépendances à longue distance

### Option D: Entraînement Progressif
1. Commencer avec w_cnn très faible (0.01)
2. Augmenter progressivement si le CNN améliore les scores
3. Early stopping basé sur la performance MCTS réelle (pas juste loss)

---

## Métriques de Référence

### Scores de Base (2026-01-18)
- Random: ~50 pts
- Pure MCTS (100 sims): ~100 pts
- Pure MCTS (150 sims): ~105 pts
- Optimal théorique: ~200 pts

### Historique des Corrections

| Date | Modification | Score MCTS+NN |
|------|--------------|---------------|
| Initial | CNN avec encoding incorrect | 12 pts |
| Fix 1 | Features de lignes (47 ch) | 12 pts (pas d'amélioration) |
| Fix 2 | Désactivation filtrage CNN | 12 pts (pas d'amélioration) |
| Fix 3 | Rollouts pour value_estimates | **99.4 pts** |

---

## Commandes Utiles

```bash
# Test de performance
cargo run --release --bin compare_mcts -- --games 30 --simulations 100 --nn-architecture cnn

# Analyse géométrique
cargo run --release --bin debug_geometry

# Entraînement supervisé
cargo run --release --bin supervised_trainer_csv -- \
  --data supervised_130plus_filtered.csv \
  --epochs 150 --batch-size 64 \
  --policy-lr 0.001 --value-lr 0.0001 \
  --nn-architecture cnn
```

---

## Conclusion

Le CNN n'apporte actuellement **aucune valeur ajoutée** - il est entièrement contourné. Pour qu'il contribue positivement:

1. L'architecture doit respecter la géométrie hexagonale
2. L'entraînement doit produire des prédictions qui corrèlent avec les scores rollouts
3. L'intégration dans MCTS doit être progressive et validée empiriquement

Le travail de debug a identifié précisément pourquoi le CNN échouait. Les prochaines investigations devraient se concentrer sur une architecture alternative (GNN recommandé) plutôt que de forcer le CNN à apprendre des patterns qu'il ne peut pas naturellement capturer.
