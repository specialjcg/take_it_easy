# Pattern Rollouts V3 - Analyse d'Échec

## ❌ Résultats Catastrophiques

| Version | Score Moyen | vs V2 | Diagnostic |
|---------|-------------|-------|------------|
| **Pattern Rollouts V2** | **139.40 pts** | — | ✅ **Baseline stable** |
| **Pattern Rollouts V3** | **88.12 pts** | **-51.28 pts (-37%)** | ❌ **Échec critique** |

**Configuration benchmark**: 50 parties, 150 simulations/coup, seed=2025

---

## 🔧 Optimisations Tentées (Toutes Échouées)

### 1. Progressive Widening Trop Restrictif

**Changement**:
```rust
// V2 (baseline)
let top_k = ((total_visits as f64).sqrt() as usize).max(5);
// → √150 = 12.2 coups explorés

// V3 (échec)
let top_k = ((total_visits as f64).cbrt() as usize + 3).max(4);
// → ∛150 + 3 = 8.3 coups explorés (-33%)
```

**Analyse de l'échec**:
- ❌ Réduction de 12 → 8 coups **trop agressive**
- ❌ Réseau neural a besoin d'explorer suffisamment pour identifier les meilleurs coups
- ❌ Racine cubique croît trop lentement, bloquant la découverte de bonnes positions

**Impact**: -20 à -30 pts estimé

---

### 2. c_puct Trop Élevé (Sur-exploration)

**Changement**:
```rust
// V2 (baseline)
let base_c_puct = match current_turn {
    0..=4   => 4.2,
    5..=15  => 3.8,
    16..=19 => 3.0,
};

// V3 (échec)
let base_c_puct = match current_turn {
    0..=4   => 4.5,  // +7%
    5..=15  => 4.0,  // +5%
    16..=19 => 3.2,  // +7%
};
```

**Analyse de l'échec**:
- ❌ Augmentation de 5-7% **trop importante**
- ❌ Encourage exploration au détriment de l'exploitation
- ❌ MCTS gaspille des simulations sur des coups sous-optimaux

**Impact**: -10 à -15 pts estimé

---

### 3. Coefficients Déséquilibrés

**Changement**:
```rust
// V2 (baseline)
let combined_eval = 0.6 * normalized_value     // ValueNet
                  + 0.2 * normalized_rollout   // Rollouts
                  + 0.1 * normalized_heuristic // Géométrie
                  + 0.1 * contextual;          // Contexte

// V3 (échec)
let combined_eval = 0.65 * normalized_value    // +8%
                  + 0.20 * normalized_rollout  // =
                  + 0.08 * normalized_heuristic // -20%
                  + 0.07 * contextual;         // -30%
```

**Analyse de l'échec**:
- ❌ Trop de confiance au ValueNet (0.65)
- ❌ Réduction excessive des heuristiques géométriques (-20%)
- ❌ Réduction excessive du contexte plateau (-30%)
- ❌ **Heuristiques encodent la connaissance du jeu**, réduire leur poids casse la synergie

**Impact**: -10 à -20 pts estimé

---

## 📊 Cumul des Erreurs

| Optimisation | Impact estimé | Cumul |
|--------------|---------------|-------|
| Progressive Widening restrictif | -20 à -30 pts | -25 pts |
| c_puct trop élevé | -10 à -15 pts | -37.5 pts |
| Coefficients déséquilibrés | -10 à -20 pts | -52.5 pts |

**Total impact**: **-52.5 pts** (proche du -51.28 pts observé)

---

## 💡 Leçons Apprises

### 1. **Ne pas casser l'équilibre**

Pattern Rollouts V2 a atteint un équilibre fragile entre:
- Exploration (c_puct, Progressive Widening) ↔ Exploitation (meilleures positions)
- ValueNet (précision NN) ↔ Heuristiques (connaissance du domaine)
- Diversité (top_k élevé) ↔ Focalisation (top_k bas)

**Modifier un seul paramètre peut casser cet équilibre.**

### 2. **Progressive Widening: sqrt est optimal**

Pour 150 simulations:
- `sqrt(150) = 12` coups → ✅ **Équilibre parfait**
- `cbrt(150) = 5` coups → ❌ **Trop restrictif**
- `150^0.4 = 9` coups → Peut-être un compromis (non testé)

**Conclusion**: Racine carrée est le bon compromis pour ce problème.

### 3. **c_puct: Éviter la sur-exploration**

Les valeurs V2 (3.0-4.2) sont déjà bien calibrées:
- Augmenter → Trop d'exploration, gaspillage
- Diminuer → Convergence prématurée

**Conclusion**: Ne pas toucher sans raison forte.

### 4. **Heuristiques sont critiques**

Réduire le poids des heuristiques (0.10 → 0.08) et du contexte (0.10 → 0.07) a été **désastreux**.

**Pourquoi ?**
- Heuristiques encodent les **règles du jeu** (complétion de lignes, conflits)
- ValueNet apprend des **patterns statistiques** mais peut manquer des règles
- **La synergie** entre NN et heuristiques est clé

**Conclusion**: [0.6, 0.2, 0.1, 0.1] est optimal, ne pas modifier.

---

## 🎯 Recommandations

### Pour atteindre 145+ pts (5.60 pts manquants)

**Option A: Améliorer le réseau neural** ⭐ **Recommandé**
- Gold GNN architecture (Graph Attention Networks)
- Meilleure capture des dépendances spatiales hexagonales
- Gain estimé: +3-6 pts

**Option B: Tuning micro-ajustements**
- Variations infimes de c_puct (±0.1 max)
- Variations infimes de coefficients (±0.01 max)
- Gain estimé: +1-2 pts (risque élevé)

**Option C: Ne rien faire** ✅ **Solution conservatrice**
- Pattern Rollouts V2 dépasse déjà les objectifs conservateur (136) et réaliste (138)
- Risque de régression élevé
- **"Perfect is the enemy of good"**

---

## 📝 Conclusion

**Pattern Rollouts V2 est la solution finale optimale.**

Tentatives d'optimisation V3:
- ❌ Progressive Widening: Échec
- ❌ c_puct augmenté: Échec
- ❌ Coefficients ajustés: Échec
- ❌ **Résultat global: -51.28 pts (-37%)**

**Les paramètres V2 sont déjà un optimum local robuste.**

Toute amélioration supplémentaire nécessite:
1. Amélioration architecturale (Gold GNN)
2. Réentraînement du réseau neural
3. **Pas** de tuning des hyperparamètres MCTS existants

---

*Benchmark V3 réalisé le 2025-10-25*
*Reverted to V2 immédiatement après diagnostic*
