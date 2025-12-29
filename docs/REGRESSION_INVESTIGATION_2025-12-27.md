# Investigation: Régression de Performance 140 pts → 80 pts

**Date**: 2025-12-27
**Problème**: Même réseau de neurones qui produisait > 140 pts sur branche précédente produit maintenant ~80 pts
**Hypothèse**: Régression dans l'implémentation MCTS ou utilisation du réseau

---

## Option 1: Analyse de l'Implémentation MCTS Actuelle

### Configuration Actuelle (hyperparameters.rs)

#### Paramètres c_puct (Exploration/Exploitation)
```rust
c_puct_early: 4.2  // Tours 0-4 (haute exploration)
c_puct_mid: 3.8    // Tours 5-15
c_puct_late: 3.0   // Tours 16+ (exploitation)
```

#### Pruning Dynamique
```rust
prune_early: 0.05  // Garde top 95% des moves
prune_mid1: 0.10   // Tours 5-9: garde 90%
prune_mid2: 0.15   // Tours 10-14: garde 85%
prune_late: 0.20   // Tours 15+: garde 80%
```

#### Rollouts Adaptatifs
```rust
rollout_strong: 3   // Value > 0.7 (CNN confiant)
rollout_medium: 5   // 0.2 < Value <= 0.7
rollout_default: 7  // Moves neutres
rollout_weak: 9     // Value < -0.4 (exploration nécessaire)
```

#### Poids d'Évaluation (Pattern Rollouts V2)
```rust
weight_cnn: 0.65        // ⬆️ Augmenté de 0.60 (optimisé Phase 1)
weight_rollout: 0.25    // ⬆️ Augmenté de 0.20
weight_heuristic: 0.05  // ⬇️ Réduit de 0.10
weight_contextual: 0.05 // ⬇️ Réduit de 0.10
```

**Observation**: Les poids privilégient fortement le réseau CNN (65%) et les rollouts (25%). Si le réseau est faible, l'impact est amplifié!

#### Adaptive Simulations (Quick Win #1)
```rust
sim_mult_early: 0.67  // 100 sims (67% of base)
sim_mult_mid: 1.0     // 150 sims (base)
sim_mult_late: 1.67   // 250 sims (167% of base)
```

**Total simulations pour un jeu complet** (19 tours, base=150):
- Tours 0-4 (5 tours): 5 × 100 = 500 sims
- Tours 5-15 (11 tours): 11 × 150 = 1,650 sims
- Tours 16-18 (3 tours): 3 × 250 = 750 sims
- **Total: 2,900 simulations/game**

#### Temperature Annealing (Quick Win #2)
```rust
temp_initial: 1.8     // ⬆️ Optimisé de 1.5 (plus d'exploration early)
temp_final: 0.5       // Exploitation finale
temp_decay_start: 7   // ⬆️ Retardé de 5 (exploration prolongée)
temp_decay_end: 13    // ⬆️ Avancé de 15 (transition rapide)
```

**Impact de température sur UCB**:
```rust
exploration_param = temperature * c_puct * ln(total_visits) / (1 + visits)
ucb_score = combined_eval + exploration_param * sqrt(prior_prob)
```

### Points de Suspicion dans l'Implémentation

#### 1. RAVE Désactivé (ligne 329)
```rust
// RAVE disabled - incompatible with Pattern Rollouts heuristics
// Pattern Rollouts biases introduce false correlations in RAVE statistics
```

**Question**: RAVE était-il activé sur la branche > 140 pts?
- Si OUI → Désactivation pourrait expliquer la régression
- Configuration RAVE actuelle: `rave_k: 10.0` (conservatif)

#### 2. Progressive Widening (ligne 400-408)
```rust
let pw_config = ProgressiveWideningConfig::adaptive(current_turn, total_turns);
let max_actions = max_actions_to_explore(
    total_visits as usize,
    legal_moves.len(),
    &pw_config,
);
```

**Formule**: `k(n) = C × n^α`
- Limite le nombre d'actions explorées
- Si trop restrictif → manque les meilleurs moves

**Question**: Quel était le config PW sur branche précédente?

#### 3. Pruning Agressif (ligne 372-374)
```rust
let pruning_ratio = hyperparams.get_pruning_ratio(current_turn);
let value_threshold = min_value + (max_value - min_value) * pruning_ratio;
```

**Impact**:
- Early game: élimine bottom 5% (très conservatif)
- Late game: élimine bottom 20% (agressif)

**Risque**: Si ValueNet est bruité, peut éliminer le meilleur move!

#### 4. Rollout Strategy avec Lookahead (ligne 442-471)
```rust
// Pour chaque rollout:
// 1. Tirer une tuile hypothétique (T2)
// 2. Simuler TOUS les placements possibles de T2
// 3. Prendre le MAX score
```

**Complexité**: O(rollout_count × legal_moves)
- rollout_count = 3-9
- legal_moves = ~10-19

**Observation**: Stratégie très gourmande, peut introduire biais optimiste

#### 5. Évaluation Combinée (ligne 504-507)
```rust
let combined_eval =
    0.65 * normalized_value      // CNN (-1 à 1)
  + 0.25 * normalized_rollout    // Rollouts (-1 à 1)
  + 0.05 * normalized_heuristic  // Domain heuristics (-1 à 1)
  + 0.05 * contextual;           // Contextual boost (-1 à 1)
```

**Questions critiques**:
- Les normalisations sont-elles correctes?
- `normalized_value = value.clamp(-1.0, 1.0)` → Direct clamp, pas de scaling?
- `normalized_rollout = ((score / 200.0).clamp(0,1) * 2.0) - 1.0` → Assume score max = 200
- `normalized_heuristic = (eval / 30.0).clamp(-1,1)` → Assume heuristic max = 30

**Risque de régression**:
- Si ValueNet output change d'échelle → Mauvaise normalisation
- Si rollout scores changent → Normalisation incorrecte

---

## Option 2: Vérification de l'Utilisation du Réseau

### Encodage des États (tensor_conversion.rs)

#### Structure du Tenseur (8 canaux × 5×5)
```rust
Channel 0: tile.0 / 10.0        // Valeur 1 normalisée [0,1]
Channel 1: tile.1 / 10.0        // Valeur 2 normalisée [0,1]
Channel 2: tile.2 / 10.0        // Valeur 3 normalisée [0,1]
Channel 3: occupied (0 ou 1)    // Masque de présence
Channel 4: turn_normalized      // current_turn / total_turns
Channel 5: orientation_scores[0] // Score ligne horizontale [0,1]
Channel 6: orientation_scores[1] // Score diagonale 1 [0,1]
Channel 7: orientation_scores[2] // Score diagonale 2 [0,1]
```

**Output shape**: `[1, 8, 5, 5]` (batch=1, 8 channels, 5×5 grid)

#### Points à Vérifier

1. **Normalisation des valeurs de tuiles** (ligne 117-119)
```rust
features[grid_idx] = (tile.0 as f32 / 10.0).clamp(0.0, 1.0);
```

**Question**: Les tuiles ont des valeurs 0-9?
- Si oui → normalisation correcte
- Si non (ex: 1-9) → Mauvais scaling!

2. **Occupied mask** (ligne 120-121)
```rust
features[3 * GRID_SIZE * GRID_SIZE + grid_idx] =
    if *tile == Tile(0, 0, 0) { 0.0 } else { 1.0 };
```

**Question**: Tile(0,0,0) représente bien une case vide?

3. **Orientation scores** (ligne 159-199)
```rust
let mut counts = [0usize; 10];  // Compte occurrences par valeur
let max_count = counts.iter().copied().max().unwrap_or(0) as f32;
let ratio = (max_count / len).clamp(0.0, 1.0);
```

**Calcul du score**:
- Compte combien de fois la valeur la plus fréquente apparaît
- Divise par longueur de la ligne
- Exemple: ligne [5,5,5,7,9] → max_count=3, len=5 → ratio=0.6

**Question**: Ce calcul était-il identique sur branche > 140 pts?

### Forward Pass du Réseau (algorithm.rs)

#### Policy Network (ligne 232-233)
```rust
let policy_logits = policy_net.forward(&input_tensor, false);
let policy = policy_logits.log_softmax(-1, tch::Kind::Float).exp();
```

**Problème potentiel**: `log_softmax().exp()` = `softmax()` avec plus d'erreur numérique!
- Pourquoi ne pas juste `softmax()`?
- Peut introduire instabilité numérique

#### Value Network (ligne 256-259)
```rust
let pred_value = value_net
    .forward(&board_tensor_temp, false)
    .double_value(&[])
    .clamp(-1.0, 1.0);
```

**Questions**:
1. Le réseau produit-il déjà des valeurs [-1,1]? (tanh activation?)
2. Ou produit-il des valeurs non-bornées qui sont clampées?
3. **Clamp APRÈS ou AVANT extraction** de `.double_value()`?

**Risque**: Si le réseau output est unbounded et qu'on clamp, on peut perdre information

#### Prior Probability Extraction (ligne 486)
```rust
let prior_prob = policy.i((0, position as i64)).double_value(&[]);
```

**Vérifications**:
- Index (0, position) correct? (batch=0, action=position)
- Policy est-il bien softmax? (somme à 1.0)

---

## Option 3: Tests avec Différents Paramètres MCTS

### Tests Proposés

#### Test A: Retour aux Paramètres Simples (Baseline)
```rust
c_puct_early: 1.41  // √2 (classique)
c_puct_mid: 1.41
c_puct_late: 1.41
prune_early: 0.0    // Pas de pruning
prune_mid1: 0.0
prune_mid2: 0.0
prune_late: 0.0
weight_cnn: 1.0     // CNN seulement
weight_rollout: 0.0
weight_heuristic: 0.0
weight_contextual: 0.0
temp_initial: 1.0   // Pas de température
temp_final: 1.0
```

**Objectif**: Isoler l'effet du réseau seul, sans optimisations MCTS

**Hypothèse**: Si score remonte à > 140 → Optimisations MCTS sont le problème

#### Test B: Augmenter Poids CNN
```rust
weight_cnn: 0.9     // Augmenté de 0.65
weight_rollout: 0.05
weight_heuristic: 0.025
weight_contextual: 0.025
```

**Objectif**: Maximiser l'influence du réseau

**Hypothèse**: Si réseau est bon, augmenter son poids devrait améliorer scores

#### Test C: Désactiver Progressive Widening
```rust
// Dans algorithm.rs, ligne 420:
let top_k = moves_with_prior.len();  // Explorer TOUTES les actions
```

**Objectif**: Vérifier si PW élimine les meilleurs moves

**Hypothèse**: Si score remonte → PW trop restrictif

#### Test D: Désactiver Pruning
```rust
prune_early: 0.0
prune_mid1: 0.0
prune_mid2: 0.0
prune_late: 0.0
```

**Objectif**: Vérifier si pruning élimine meilleur move

**Hypothèse**: Si score remonte → ValueNet bruité, pruning contre-productif

#### Test E: Augmenter Simulations
```rust
sim_mult_early: 1.0   // 150 sims partout
sim_mult_mid: 1.0
sim_mult_late: 1.0
```

Tester avec `--simulations 500` ou `1000` (au lieu de 150)

**Objectif**: Vérifier si plus de simulations améliore

**Hypothèse**: Si oui → Actuel underfitted, besoin de plus de compute

#### Test F: Température Constante
```rust
temp_initial: 1.0
temp_final: 1.0
temp_decay_start: 100  // Jamais de decay
temp_decay_end: 100
```

**Objectif**: Désactiver annealing de température

**Hypothèse**: Si score remonte → Annealing nuit à la qualité

#### Test G: Grid Search sur c_puct
Tester avec 10 games chacun:
```
c_puct = [0.5, 1.0, 1.41, 2.0, 3.0, 4.0, 5.0]
```

**Objectif**: Trouver valeur optimale pour le réseau actuel

**Hypothèse**: c_puct actuel (4.2/3.8/3.0) pas optimal

---

## Plan d'Investigation

### Phase 1: Validation du Réseau (1 heure)

1. **Vérifier les poids du réseau**
```bash
md5sum model_weights/cnn/policy/policy.params
md5sum model_weights/cnn/value/value.params
```

2. **Tester forward pass manuel**
```rust
// Créer un plateau simple, vérifier que policy somme à 1.0
// Vérifier que value est dans [-1, 1]
```

3. **Comparer tensor encoding**
```rust
// Imprimer un tenseur encodé
// Vérifier que valeurs sont cohérentes
```

### Phase 2: Test Baseline CNN-Only (30 min)

**Test A**: Désactiver toutes optimisations
```bash
# Modifier hyperparameters.rs avec Test A config
cargo build --release --bin benchmark_progressive_widening
./target/release/benchmark_progressive_widening --games 20 --simulations 150
```

**Critère de succès**: Score > 100 pts → Le réseau fonctionne, problème = optimisations MCTS

### Phase 3: Tests Ciblés (2-3 heures)

Si Test A < 100 pts → Problème dans réseau ou encodage
Si Test A > 100 pts → Problème dans optimisations MCTS

**Arbre de décision**:
```
Test A < 100 pts?
  ├─ OUI → Vérifier:
  │   ├─ Normalisation des tenseurs
  │   ├─ Forward pass du réseau
  │   └─ Poids du réseau (comparaison md5)
  │
  └─ NON (> 100 pts) → Tester séquentiellement:
      ├─ Test D (sans pruning)
      ├─ Test C (sans PW)
      ├─ Test F (sans temperature)
      └─ Test B (CNN weight=0.9)
```

### Phase 4: Grid Search (optionnel, 4-5 heures)

Si aucun test simple ne fonctionne → Grid search complet

---

## Hypothèses Principales

### Hypothèse 1: Poids du Réseau Écrasés
**Probabilité**: Haute (30%)
**Vérification**: Comparer md5 des poids
**Solution**: Restaurer poids de la branche précédente

### Hypothèse 2: Normalisation Cassée
**Probabilité**: Moyenne (25%)
**Vérification**: Imprimer tenseurs et valeurs forward
**Solution**: Corriger les formules de normalisation

### Hypothèse 3: Optimisations MCTS Contre-Productives
**Probabilité**: Moyenne (20%)
**Vérification**: Test A (CNN-only baseline)
**Solution**: Désactiver optimisations problématiques

### Hypothèse 4: Progressive Widening Trop Restrictif
**Probabilité**: Moyenne (15%)
**Vérification**: Test C (désactiver PW)
**Solution**: Ajuster paramètres α, β

### Hypothèse 5: Pruning Élimine Meilleurs Moves
**Probabilité**: Faible (10%)
**Vérification**: Test D (désactiver pruning)
**Solution**: Réduire pruning ratios

---

## Actions Immédiates Recommandées

### 1. Vérifier Intégrité des Poids (5 min)
```bash
md5sum model_weights/cnn/policy/policy.params
md5sum model_weights/cnn/value/value.params
```

Si différent des poids de la branche > 140 pts → **TROUVER LES BONS POIDS**

### 2. Test Baseline CNN-Only (30 min)

Créer `hyperparameters_baseline.rs`:
```rust
impl Default for MCTSHyperparameters {
    fn default() -> Self {
        Self {
            c_puct_early: 1.41,
            c_puct_mid: 1.41,
            c_puct_late: 1.41,
            prune_early: 0.0,
            prune_mid1: 0.0,
            prune_mid2: 0.0,
            prune_late: 0.0,
            rollout_strong: 0,    // Pas de rollouts
            rollout_medium: 0,
            rollout_default: 0,
            rollout_weak: 0,
            weight_cnn: 1.0,      // CNN seulement
            weight_rollout: 0.0,
            weight_heuristic: 0.0,
            weight_contextual: 0.0,
            sim_mult_early: 1.0,
            sim_mult_mid: 1.0,
            sim_mult_late: 1.0,
            temp_initial: 1.0,
            temp_final: 1.0,
            temp_decay_start: 100,
            temp_decay_end: 100,
            rave_k: 0.0,  // RAVE désactivé
        }
    }
}
```

Benchmark avec 20-50 games pour avoir signal rapide.

### 3. Si Baseline Fonctionne (> 120 pts)

Réactiver optimisations une par une:
1. Temperature annealing
2. Adaptive simulations
3. Pruning (faible: 5%)
4. Progressive widening
5. Rollouts

Identifier laquelle casse les performances.

### 4. Si Baseline Ne Fonctionne Pas (< 100 pts)

Investiguer réseau:
```rust
// Test manual forward pass
let plateau = create_test_plateau();
let tensor = convert_plateau_to_tensor(&plateau, &tile, &deck, 0, 19);
println!("Tensor shape: {:?}", tensor.size());
println!("Tensor values (channel 0): {:?}", tensor.i((0, 0, .., ..)));

let policy = policy_net.forward(&tensor, false);
println!("Policy shape: {:?}", policy.size());
println!("Policy sum: {}", policy.sum(tch::Kind::Float).double_value(&[]));
println!("Policy values: {:?}", policy);

let value = value_net.forward(&tensor, false);
println!("Value: {}", value.double_value(&[]));
```

---

## Fichiers à Comparer avec Branche Précédente

1. `src/mcts/hyperparameters.rs` - Configuration MCTS
2. `src/mcts/algorithm.rs` - Implémentation core
3. `src/neural/tensor_conversion.rs` - Encodage états
4. `src/neural/policy_value_net.rs` - Architecture réseau
5. `model_weights/cnn/policy/policy.params` - Poids PolicyNet
6. `model_weights/cnn/value/value.params` - Poids ValueNet

---

**Prochaine Étape**: Quelle investigation voulez-vous que je lance en premier?

Options:
1. **Vérifier md5 des poids** (rapide, 5 min)
2. **Créer et tester config baseline** (30 min)
3. **Comparer code avec git diff** (si branche accessible)
4. **Test manuel forward pass** (debug réseau, 15 min)
