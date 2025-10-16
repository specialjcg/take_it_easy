# 🔍 Diagnostic: Pourquoi le Transformer n'apprend pas

## Résumé exécutif

Le Transformer entraîné sur 100 epochs n'a atteint que **14.55 points** (médiane 0) vs **133 points** pour la baseline MCTS.

Après analyse approfondie du code, **5 problèmes majeurs** ont été identifiés.

---

## ❌ Problème 1: Représentation d'état trop simpliste

### Constat
**Fichier**: `src/neural/transformer/game_state.rs:8-32`

```rust
fn to_tensor_features(&self) -> Vec<f32> {
    let mut features = Vec::with_capacity(64);
    // 19 tuiles × 3 valeurs = 57 features
    for tile in &self.plateau.tiles {
        features.extend_from_slice(&[
            tile.0 as f32 / 9.0,
            tile.1 as f32 / 9.0,
            tile.2 as f32 / 9.0,
        ]);
    }
    // Padding à 64...
}
```

### Problèmes:
1. **Perte d'information spatiale**: Les tuiles sont apla ties en un vecteur, perdant la structure hexagonale
2. **Pas de features de lignes**: Aucune information sur les lignes complètes ou potentielles
3. **Deck sous-représenté**: Une seule feature pour le deck (taille normalisée)
4. **Pas de contexte de jeu**: Tour actuel, score partiel, tuiles restantes spécifiques

### Impact:
Le Transformer ne voit qu'un vecteur plat de 64 nombres sans structure, rendant l'apprentissage de patterns spatiaux quasi-impossible.

---

## ❌ Problème 2: Taille de dataset insuffisante

### Constat
- **608 exemples** d'entraînement total
- Batch size: 16 → seulement **38 batches par epoch**
- Variance importante dans les scores (0-18 points)

### Contexte:
- Les Transformers nécessitent typiquement **10k-100k+ exemples**
- AlphaZero utilise des millions d'exemples
- Notre jeu a **27! ≈ 10^28** états possibles

### Impact:
Le modèle ne voit pas assez de diversité pour généraliser. Il surfit ou sous-fit constamment.

---

## ❌ Problème 3: Architecture inadaptée

### Constat
**Fichier**: `src/neural/transformer/mod.rs:30-34`

```rust
const NUM_LAYERS: i64 = 2;
const EMBEDDING_DIM: i64 = 64;
const NUM_HEADS: i64 = 2;
const FF_DIM: i64 = 128;
```

### Problèmes:
1. **Trop petit pour apprendre**: 2 couches, 64 dim → ~40k paramètres
2. **Reshape arbitraire**: Input [1, 5, 47] → reshape → [seq=4, dim=16]
3. **Pooling global**: `mean_dim` perd l'information de position

### Comparaison:
- AlphaZero ResNet: ~1M paramètres
- Notre policy/value CNN: ~100k paramètres
- Notre Transformer: ~40k paramètres ❌

### Impact:
Le modèle n'a pas assez de capacité pour capturer les patterns complexes du jeu.

---

## ❌ Problème 4: Loss function problématique

### Constat
**Fichier**: `src/neural/transformer/training.rs:205-230`

```rust
// Créer un target de boost binaire
let boost_target_batch = (&boosted_target_batch - &policy_target_batch)
    .abs()
    .gt(0.01) // Si différence > 1%, c'est une position boostée
    .to_kind(Kind::Float);

// Combiner les trois losses
let loss: Tensor = policy_loss + value_loss + boost_loss * 0.3;
```

### Problèmes:
1. **Détection de boost naïve**: Différence > 1% ne capture pas bien les vrais boosts (qui peuvent être x1000)
2. **Poids arbitraires**: 1.0 + 1.0 + 0.3 sans justification
3. **Pas de pondération par difficulté**: Toutes les positions comptent pareil
4. **Label smoothing sur policy**: Réduit le signal d'apprentissage

### Impact:
Le modèle reçoit des signaux d'apprentissage confus et contradictoires.

---

## ❌ Problème 5: Boosts presque identiques

### Constat
**Données inspectées**:
```
game_data_policy_raw_transformer.pt:
  Min: 0.0000, Max: 0.0902, Mean: 0.0526

game_data_policy_boosted_transformer.pt:
  Min: 0.0000, Max: 0.0648, Mean: 0.0526
```

### Problème:
Les politiques boostées sont **presque identiques** aux politiques raw !
- Max raw: 0.0902
- Max boosted: 0.0648 (plus petit ?!)

Cela suggère que :
1. **Les boosts ne sont pas appliqués** correctement lors de la sauvegarde
2. **Ou normalisation écrase les boosts** (softmax après boost ?)
3. **Ou les boosts sont trop faibles** dans MCTS

### Impact:
Le Transformer apprend à copier MCTS raw, pas à apprendre les heuristiques de boost qui donnent +20-40 points.

---

## ✅ Solutions recommandées (par priorité)

### 🥇 Priorité 1: Améliorer la représentation d'état

```rust
fn to_tensor_features(&self) -> Vec<f32> {
    let mut features = Vec::with_capacity(256);

    // 1. Tuiles du plateau (19 × 3 = 57)
    for tile in &self.plateau.tiles {
        features.push(tile.0 as f32 / 9.0);
        features.push(tile.1 as f32 / 9.0);
        features.push(tile.2 as f32 / 9.0);
    }

    // 2. Lignes complètes actuelles (10 lignes × 3 couleurs = 30)
    for line in get_all_lines() {
        let (complete_1, complete_5, complete_9) = check_line_completion(&self.plateau, line);
        features.push(if complete_1 { 1.0 } else { 0.0 });
        features.push(if complete_5 { 1.0 } else { 0.0 });
        features.push(if complete_9 { 1.0 } else { 0.0 });
    }

    // 3. Lignes potentielles (10 lignes × 3 couleurs = 30)
    for line in get_all_lines() {
        let (potential_1, potential_5, potential_9) = check_line_potential(&self.plateau, line);
        features.push(potential_1);
        features.push(potential_5);
        features.push(potential_9);
    }

    // 4. Score partiel actuel (1)
    features.push(current_score() / 200.0);

    // 5. Tour actuel (1)
    features.push(turn / 19.0);

    // 6. Tuiles disponibles par couleur (9 features)
    for color in 1..=9 {
        let count = count_tiles_with_color(&self.deck, color);
        features.push(count as f32 / 27.0);
    }

    // Total: 57 + 30 + 30 + 1 + 1 + 9 = 128 features
    features
}
```

### 🥈 Priorité 2: Générer beaucoup plus de données

```bash
# Générer 5000 parties
cargo run --release --bin take_it_easy -- --mode autotest --num-games 5000 --num-simulations 150
```

### 🥉 Priorité 3: Augmenter la capacité du modèle

```rust
const NUM_LAYERS: i64 = 4;        // 2 → 4
const EMBEDDING_DIM: i64 = 128;   // 64 → 128
const NUM_HEADS: i64 = 4;         // 2 → 4
const FF_DIM: i64 = 512;          // 128 → 512
```

### 4️⃣ Priorité 4: Vérifier les boosts dans MCTS

Inspecter `src/mcts/algorithm.rs` pour s'assurer que :
1. Les boosts sont correctement appliqués avant softmax
2. Les valeurs sont sauvegardées **après** boost mais **avant** normalisation
3. L'intensité du boost reflète la magnitude (x1000 pour ligne 9)

### 5️⃣ Priorité 5: Améliorer la loss function

```rust
// Pondération adaptative selon le tour
let turn_weight = 1.0 + (current_turn as f32 / 19.0) * 2.0;

// Loss de policy avec focus sur les boosts réels
let policy_loss = cross_entropy_with_weights(policy_logits, policy_target_batch, boost_weights);

// Loss de value avec normalisation adaptée
let value_loss = huber_loss(value_pred, value_target_batch);

// Boost loss avec détection plus robuste (seuil à 0.1 au lieu de 0.01)
let boost_target = (&boosted_target_batch - &policy_target_batch).abs().gt(0.1);
let boost_loss = focal_loss(boost_logits, boost_target, alpha=0.25, gamma=2.0);

// Combiner avec pondération dynamique
let loss = policy_loss * turn_weight + value_loss + boost_loss * 0.5;
```

---

## 📊 Métriques actuelles vs Objectif

| Métrique | Actuel | Objectif | Écart |
|----------|--------|----------|-------|
| Score moyen Transformer | 14.55 | >140 | -89.6% |
| Score médian | 0 | >130 | -100% |
| Exemples d'entraînement | 608 | 5000+ | -87.8% |
| Paramètres modèle | ~40k | ~200k | -80% |
| Features d'état | 64 | 128+ | -50% |
| Loss finale | 165.5 | <50 | +231% |

---

## 🎯 Plan d'action recommandé

**Phase 1 (Immédiat)**:
1. ✅ Diagnostic complet (fait)
2. ⏳ Améliorer `to_tensor_features()` avec 128 features structurées
3. ⏳ Vérifier que les boosts MCTS sont correctement sauvegardés

**Phase 2 (Court terme - 1-2h)**:
4. Générer 5000 parties supplémentaires
5. Augmenter l'architecture (4 layers, 128 dim)
6. Ré-entraîner 200 epochs

**Phase 3 (Moyen terme - 1 jour)**:
7. Implémenter les améliorations de loss function
8. Ajouter data augmentation (rotations, symétries)
9. Validation croisée avec hold-out set

**Phase 4 (Long terme - 1 semaine)**:
10. Self-play itératif (AlphaZero style)
11. Curriculum learning (commencer par fins de partie)
12. Ensemble de modèles

---

## 📚 Références

- AlphaZero: https://arxiv.org/abs/1712.01815
- Attention Is All You Need: https://arxiv.org/abs/1706.03762
- Focal Loss: https://arxiv.org/abs/1708.02002
- Deep RL avec sparse rewards: https://arxiv.org/abs/1707.06347

---

**Conclusion**: Le Transformer n'apprend pas principalement à cause de :
1. Représentation d'état trop pauvre (64 features plates)
2. Dataset trop petit (608 exemples)
3. Architecture sous-dimensionnée (~40k params)

**Action immédiate**: Améliorer `to_tensor_features()` pour inclure des features structurées (lignes, potentiels, contexte de jeu).
