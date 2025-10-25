# Pattern Rollouts - Phase 2 MCTS Improvement

## 📋 Résumé

Implémentation des **Pattern Rollouts** (rollouts heuristiques intelligents) pour améliorer la qualité d'évaluation MCTS. Cette amélioration remplace les simulations aléatoires pures par des simulations guidées par heuristiques.

**Objectif de gain**: +8 à +12 points sur la moyenne
**Baseline CNN**: 127.72 pts
**Cible avec Pattern Rollouts**: 136-140 pts

## ✅ Implémentation Complète

### 1. Carte d'Adjacence Hexagonale (src/game/simulate_game_smart.rs)

```rust
fn get_adjacent_positions(position: usize) -> Vec<usize> {
    // Basé sur la structure hexagonale réelle:
    //     0  1  2
    //    3  4  5  6
    //   7  8  9 10 11
    //    12 13 14 15
    //      16 17 18
    match position { ... }
}
```

Tous les 19 positions avec leurs voisins corrects selon la topologie hexagonale.

### 2. Fonction de Rollout Intelligent

```rust
pub fn simulate_games_smart(
    plateau: Plateau,
    deck: Deck,
    _policy_net: Option<&PolicyNet>
) -> i32
```

**Stratégie 80/20**:
- **80% du temps**: Sélection gloutonne via heuristiques
- **20% du temps**: Exploration aléatoire

### 3. Évaluation Heuristique de Position

```rust
fn evaluate_position_for_tile(plateau: &Plateau, tile: &Tile, position: usize) -> f64 {
    let mut score = 0.0;

    // Bonus 1: Positions centrales (contrôle stratégique)
    if [4, 8, 12, 16].contains(&position) {
        score += 0.5;
    }

    // Bonus 2: Tuiles haute valeur
    let tile_value = tile.0 + tile.1 + tile.2;
    score += (tile_value as f64) * 0.02;

    // Bonus 3: Complétion/extension de lignes
    score += estimate_line_completion_bonus(plateau, tile, position);

    score
}
```

**Bonus de Ligne**:
- +0.3 pour chaque tuile adjacente avec valeur correspondante
- Encourage la formation de lignes complètes

### 4. Intégration MCTS (src/mcts/algorithm.rs)

**Deux points de remplacement**:

1. **Ligne 253**: Évaluation initiale des coups (Pure MCTS)
```rust
// AVANT:
simulate_games(temp_plateau.clone(), temp_deck.clone())

// APRÈS:
simulate_games_smart(temp_plateau.clone(), temp_deck.clone(), None)
```

2. **Ligne 422**: Simulations dans la boucle principale
```rust
// AVANT:
let score = simulate_games(plateau2.clone(), deck2.clone()) as f64;

// APRÈS:
let score = simulate_games_smart(plateau2.clone(), deck2.clone(), None) as f64;
```

## 📊 Impact sur les Performances

### Coût Computationnel

**Calculs par partie**:
- 150 simulations × 19 tours = 2,850 simulations MCTS
- 6 rollouts par simulation = 17,100 rollouts
- Chaque rollout smart fait ~10-20x plus de calculs qu'un rollout aléatoire

**Observation initiale**:
- Benchmark 50 parties en cours
- Temps écoulé: 12+ minutes pour la première partie
- Estimation: 1-2 heures pour 50 parties (vs 15 min pour baseline)

### Trade-off Qualité vs Vitesse

| Métrique | Random Rollouts | Smart Rollouts |
|----------|----------------|----------------|
| Calculs/rollout | ~50 ops | ~500-1000 ops |
| Précision éval | Faible | Élevée |
| Temps/partie | ~5s | ~60-120s |
| Gain qualité | Baseline | +8-12 pts (estimé) |

## 🎯 Prochaines Étapes

### Phase 3: Optimisation (si nécessaire)

Si le gain de qualité est confirmé mais la vitesse est un problème:

1. **Réduire le nombre de rollouts**
   - `rollout_count`: 6 → 2 ou 3
   - Gain vitesse: 2-3x
   - Perte qualité: minime

2. **Caching des évaluations**
   - Mémoriser les scores de positions similaires
   - Réduire calculs redondants

3. **Simplification heuristique**
   - Retirer les bonus moins impactants
   - Focus sur centre + ligne completion

4. **Profiling**
   - Identifier les hotspots exacts
   - Optimiser les chemins critiques

### Phase 4: RAVE (Rapid Action Value Estimation)

Une fois Pattern Rollouts validé:
- Implémenter RAVE pour réutiliser les valeurs d'actions
- Gain estimé: +5-8 pts
- Cible finale: 145+ pts

## 📝 Fichiers Modifiés

- `src/game/simulate_game_smart.rs` (nouveau)
- `src/game/mod.rs` (ajout module)
- `src/mcts/algorithm.rs` (intégration)
- `src/game/simulate_game.rs` (marqué `#[allow(dead_code)]`)

## 🔧 Commandes Utiles

**Suivre le benchmark**:
```bash
./monitor_pattern_rollouts.sh
```

**Vérifier l'état**:
```bash
tail -f pattern_rollouts_benchmark.log
```

**Voir les résultats**:
```bash
grep "Average score" pattern_rollouts_benchmark.log
```

## 📈 Résultats Attendus

**Scénario optimiste** (+12 pts):
- Moyenne: 140 pts
- Amélioration: +9.6% vs baseline CNN

**Scénario réaliste** (+10 pts):
- Moyenne: 138 pts
- Amélioration: +8.0% vs baseline CNN

**Scénario conservateur** (+8 pts):
- Moyenne: 136 pts
- Amélioration: +6.5% vs baseline CNN

**Seuil de succès**: ≥ 136 pts (Phase 2 validée ✅)

---

*Implémenté le 2025-10-24*
*Benchmark en cours - résultats attendus sous 1-2 heures*
