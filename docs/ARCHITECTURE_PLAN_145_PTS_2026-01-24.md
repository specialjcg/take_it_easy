# Plan d'Architecture pour > 145 pts
**Date:** 2026-01-24
**Auteur:** Claude Opus 4.5
**Status:** Plan d'action structuré

---

## Résumé Exécutif

**Objectif:** Atteindre un score moyen > 145 pts de manière reproductible.

**État actuel:**
- Q-Net Hybrid: **125.14 pts** (meilleur actuel)
- Pattern Rollouts V2 historique: **139.40 pts** (avec bons poids CNN)
- UCT MCTS peak: **149.36 pts** (atteint historiquement)
- Gap à combler: **~20 pts**

**Diagnostic principal:** Les poids CNN sont cassés/non-entraînés (policy uniforme, value constante). La performance actuelle vient à 100% des rollouts et heuristiques.

**Solution:** Architecture hybride combinant Q-Net pruning + CNN restauré + Pattern Rollouts V2.

---

## 🏗️ ARCHITECTURE CIBLE

### Vue d'Ensemble

```
┌─────────────────────────────────────────────────────────────────┐
│                    MCTS HYBRIDE OPTIMISÉ                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐       │
│  │   Q-NET     │ ──► │  TOP-K      │ ──► │  CNN MCTS   │       │
│  │  (Pruning)  │     │  POSITIONS  │     │  (Guidance) │       │
│  └─────────────┘     └─────────────┘     └─────────────┘       │
│         │                   │                   │               │
│         │    ┌──────────────┘                   │               │
│         │    │                                  │               │
│         ▼    ▼                                  ▼               │
│  ┌─────────────────────────────────────────────────────┐       │
│  │            PATTERN ROLLOUTS V2                       │       │
│  │  - Évaluation réelle des lignes                      │       │
│  │  - Détection de conflits                             │       │
│  │  - Scaling quadratique                               │       │
│  └─────────────────────────────────────────────────────┘       │
│                           │                                     │
│                           ▼                                     │
│  ┌─────────────────────────────────────────────────────┐       │
│  │            ÉVALUATION COMBINÉE                       │       │
│  │  w_cnn * CNN_value + w_rollout * Pattern_score       │       │
│  │  + w_heuristic * Domain_score + w_contextual * Ctx   │       │
│  └─────────────────────────────────────────────────────┘       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Composants Clés

| Composant | Rôle | Contribution Estimée |
|-----------|------|---------------------|
| **Q-Net Pruning** | Filtre top-6 positions (turns 0-9) | +20 pts (validé) |
| **CNN Policy** | Guide exploration MCTS | +10-15 pts (si restauré) |
| **Pattern Rollouts V2** | Heuristiques ligne/conflit | +12 pts (validé) |
| **Value Network** | Évaluation position | +5-8 pts (si restauré) |

---

## 📊 ANALYSE DES PERFORMANCES HISTORIQUES

### Timeline des Scores

```
Baseline random:         9.75 pts
Pure MCTS:              104-116 pts    (+100 pts)
UCT MCTS:               149.36 pts     (+35 pts)
Pattern V2 + CNN:       139.40 pts     (poids fonctionnels)
Q-Net Hybrid:           125.14 pts     (poids cassés)
CNN seul:               12.75 pts      ❌ (poids cassés)
```

### Root Cause des Écarts

| Problème | Impact | Status |
|----------|--------|--------|
| Poids CNN cassés | -27 pts | ❌ Non résolu |
| Policy uniforme | Guidance nulle | ❌ Non résolu |
| Value constante | Évaluation inutile | ❌ Non résolu |
| Expert data uniforme | Training impossible | ❌ Bug identifié |
| Q-Net training MSE | Ranking cassé | ✅ Corrigé (softmax+CE) |

---

## 🎯 PLAN D'ACTION EN PHASES

### PHASE 0: Diagnostic & Baseline (1-2h)
**Priorité:** CRITIQUE
**Objectif:** Confirmer l'état actuel et établir baseline reproductible

```bash
# 1. Vérifier état des poids CNN
./target/release/test_network_forward
# Expected: Policy NON-uniforme (max > 0.10), Value variable

# 2. Benchmark baseline actuel
./target/release/compare_mcts_hybrid --games 100 --simulations 150
# Current: ~125 pts (Q-Net Hybrid)

# 3. Benchmark Pure MCTS pour référence
./target/release/compare_mcts --strategy pure --games 100
# Expected: ~105 pts
```

**Critères de succès:**
- [ ] État des poids CNN documenté
- [ ] Baseline Q-Net Hybrid: 125 pts ± 5
- [ ] Baseline Pure MCTS: 105 pts ± 5

---

### PHASE 1: Restauration CNN (Priorité CRITIQUE)
**Durée:** 1-2 jours
**Objectif:** Restaurer les poids CNN fonctionnels
**Gain attendu:** +15-25 pts (125 → 140-150 pts)

#### Option 1A: Récupérer poids depuis branche historique ⭐ RECOMMANDÉ

```bash
# 1. Identifier la branche avec > 140 pts
git log --oneline --all --decorate | grep -i "140\|pattern\|rollout"

# 2. Copier les poids
git show <commit>:model_weights/cnn/policy/policy.params > policy_good.params
git show <commit>:model_weights/cnn/value/value.params > value_good.params

# 3. Vérifier MD5 (doit être différent)
md5sum model_weights/cnn/*/policy.params  # Current (cassé)
md5sum policy_good.params                  # Historique (bon)

# 4. Remplacer et tester
cp policy_good.params model_weights/cnn/policy/policy.params
./target/release/compare_mcts_hybrid --games 50
# Expected: 135-145 pts
```

#### Option 1B: Ré-entraînement supervisé (si 1A échoue)

**Prérequis:** Corriger le bug de génération de données expert

```rust
// src/bin/expert_data_generator.rs - FIX REQUIRED
// Bug: best_position est uniforme au lieu de refléter MCTS

// AVANT (bugué):
let best_position = random_legal_move();

// APRÈS (corrigé):
let mcts_result = mcts_find_best_position_for_tile_with_nn(...);
let best_position = mcts_result.best_position;
```

```bash
# 1. Générer nouvelles données expert (après fix)
./target/release/expert_data_generator \
    --games 500 \
    --mcts-sims 500 \
    --output expert_data_fixed.json

# 2. Vérifier distribution non-uniforme
python3 -c "
import json
from collections import Counter
data = json.load(open('expert_data_fixed.json'))
positions = [m['best_position'] for g in data for m in g['moves']]
print(Counter(positions).most_common(5))
# Expected: Centre (8,9,10) > Edges (0,18)
"

# 3. Entraîner
./target/release/supervised_trainer_csv \
    --input expert_data_fixed.json \
    --epochs 50 \
    --lr 0.001
```

**Critères de succès Phase 1:**
- [ ] Policy distribution NON-uniforme (max prob > 0.15)
- [ ] Value varie selon l'état (Δ > 0.1 entre vide et rempli)
- [ ] Benchmark: 135+ pts

---

### PHASE 2: Intégration Q-Net + CNN Restauré
**Durée:** 1 jour
**Objectif:** Combiner Q-Net pruning avec CNN fonctionnel
**Gain attendu:** +5-10 pts supplémentaires

```rust
// src/mcts/algorithm.rs - Hybrid Optimisé

pub fn mcts_hybrid_optimized(
    plateau: &mut Plateau,
    deck: &mut Deck,
    tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    qvalue_net: &QValueNet,
    config: &HybridConfig,
) -> MCTSResult {
    let turn = count_placed_tiles(plateau);
    let empty_count = 19 - turn;

    // Phase 1: Q-Net pruning (early game)
    let candidate_positions = if turn < config.prune_threshold
                               && empty_count > config.top_k + 2 {
        qvalue_net.get_top_positions(&plateau.tiles, &tile, config.top_k)
    } else {
        get_all_legal_moves(plateau)
    };

    // Phase 2: CNN-guided MCTS sur positions filtrées
    let mut node_scores = HashMap::new();

    for &pos in &candidate_positions {
        // Pattern Rollouts V2 avec guidance CNN
        let score = evaluate_position_hybrid(
            plateau, deck, tile, pos,
            policy_net, value_net,
            config.num_simulations,
        );
        node_scores.insert(pos, score);
    }

    // Phase 3: Sélection avec température
    select_best_with_temperature(&node_scores, config.temperature)
}
```

**Configuration optimisée:**

```rust
pub struct HybridConfig {
    pub top_k: usize,           // 6 (validé)
    pub prune_threshold: usize, // 10 (validé)
    pub num_simulations: usize, // 150
    pub temperature: f64,       // 0.5 (late game)

    // Poids évaluation
    pub w_cnn: f64,             // 0.35 (si CNN restauré)
    pub w_rollout: f64,         // 0.50
    pub w_heuristic: f64,       // 0.10
    pub w_contextual: f64,      // 0.05
}
```

**Critères de succès Phase 2:**
- [ ] Hybrid optimisé compile sans erreur
- [ ] Benchmark: 140+ pts
- [ ] Win rate vs Pure: > 75%

---

### PHASE 3: Dirichlet Noise pour Self-Play
**Durée:** 2-3 jours
**Objectif:** Briser la boucle circulaire d'apprentissage
**Gain attendu:** +5-10 pts (via meilleur training)

```rust
// src/bin/alphago_zero_trainer.rs - ADD Dirichlet Noise

use rand_distr::{Dirichlet, Distribution};

fn generate_self_play_game_with_exploration(
    neural_manager: &NeuralManager,
    mcts_sims: usize,
) -> GameRecord {
    let mut rng = rand::thread_rng();

    for turn in 0..19 {
        // Get MCTS policy
        let mcts_result = mcts_find_best_position(...);
        let mut policy = mcts_result.policy_distribution;

        // === ADD DIRICHLET NOISE AT ROOT ===
        if turn < 10 {  // Early game exploration
            let alpha = 0.3;  // Concentration (lower = more uniform)
            let epsilon = 0.25;  // Mix ratio

            let legal_moves = get_legal_moves(&plateau);
            let dirichlet = Dirichlet::new_with_size(alpha, legal_moves.len())
                .expect("Dirichlet creation failed");
            let noise: Vec<f64> = dirichlet.sample(&mut rng);

            // Mix policy with noise
            for (i, &pos) in legal_moves.iter().enumerate() {
                policy[pos] = (1.0 - epsilon) * policy[pos]
                            + epsilon * noise[i] as f32;
            }
        }

        // Sample action from noisy policy
        let action = sample_from_policy(&policy);
        record.add_step(state, policy, action);
    }

    record
}
```

**Paramètres AlphaGo Zero optimisés:**

```bash
./target/release/alphago_zero_trainer \
    --iterations 100 \
    --games-per-iter 100 \
    --mcts-simulations 200 \
    --epochs-per-iter 15 \
    --learning-rate 0.005 \
    --batch-size 64 \
    --dirichlet-alpha 0.3 \
    --dirichlet-epsilon 0.25 \
    --convergence-threshold 5.0 \
    --temperature 1.0 \
    --temperature-drop-turn 10
```

**Critères de succès Phase 3:**
- [ ] Policy loss < 2.5 (vs 2.944 = ln(19) uniforme)
- [ ] Value loss < 0.1
- [ ] Score amélioration: +10% après 50 iterations
- [ ] Benchmark final: 145+ pts

---

### PHASE 4: Fine-Tuning Final
**Durée:** 1-2 jours
**Objectif:** Optimiser les derniers pourcentages
**Gain attendu:** +2-5 pts

#### 4.1 Grid Search Hyperparamètres

```rust
// Configurations à tester
let configs = vec![
    HybridConfig { top_k: 5, prune_threshold: 8, w_cnn: 0.30, .. },
    HybridConfig { top_k: 6, prune_threshold: 10, w_cnn: 0.35, .. },
    HybridConfig { top_k: 7, prune_threshold: 12, w_cnn: 0.40, .. },
];

for config in configs {
    let scores = benchmark(100, config);
    log::info!("Config {:?}: mean={:.2}", config, mean(&scores));
}
```

#### 4.2 Optimisation Pattern Rollouts

```rust
// Renforcer heuristiques ligne
pub fn pattern_score_v3(plateau: &Plateau, pos: usize, tile: &Tile) -> f64 {
    let mut score = 0.0;

    for line in get_lines_through(pos) {
        let (filled, total, conflicts, potential) = analyze_line(plateau, line, tile);

        if conflicts > 0 {
            continue;  // Skip conflicted lines
        }

        // Scaling exponentiel pour lignes presque complètes
        let completion = filled as f64 / total as f64;
        let weight = completion.powf(2.5);  // Plus agressif que V2

        // Bonus massif pour complétion immédiate
        if filled + 1 == total {
            score += potential * 4.0;  // Vs 3.0 en V2
        } else {
            score += potential * weight;
        }
    }

    score
}
```

**Critères de succès Phase 4:**
- [ ] Score moyen: 147+ pts
- [ ] Score min > 100 pts
- [ ] Écart-type < 25 pts
- [ ] Win rate vs Pure: > 80%

---

## 📋 CHECKLIST ARCHITECTURE (SOLID/DRY)

### Principes Appliqués

| Principe | Application | Fichiers |
|----------|-------------|----------|
| **SRP** | Séparer Q-Net, CNN, Pattern Rollouts | `algorithm.rs`, `qvalue_net.rs` |
| **OCP** | MctsEvaluator extensible sans modification | `algorithm.rs:35` |
| **LSP** | PolicyNet/ValueNet interchangeables (trait) | `policy_value_net.rs` |
| **ISP** | Traits fins: `PolicyEvaluator`, `ValueEvaluator` | À implémenter |
| **DIP** | Dépendre des traits, pas des structs concrètes | À améliorer |

### Refactoring Recommandé

```rust
// AVANT: Couplage fort
pub fn mcts_with_nn(
    policy_net: &PolicyNetCNN,  // ❌ Type concret
    value_net: &ValueNetCNN,    // ❌ Type concret
) { ... }

// APRÈS: Abstraction via traits
pub trait PolicyEvaluator: Send + Sync {
    fn forward(&self, input: &Tensor, train: bool) -> Tensor;
    fn arch(&self) -> NNArchitecture;
}

pub fn mcts_with_nn<P: PolicyEvaluator, V: ValueEvaluator>(
    policy: &P,  // ✅ Trait bound
    value: &V,   // ✅ Trait bound
) { ... }
```

---

## 🧪 TESTS À IMPLÉMENTER

### Tests Unitaires

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_qnet_pruning_reduces_candidates() {
        let plateau = create_test_plateau();
        let tile = Tile(5, 5, 5);
        let positions = qnet.get_top_positions(&plateau, &tile, 6);
        assert_eq!(positions.len(), 6);
    }

    #[test]
    fn test_pattern_rollout_prefers_completion() {
        let mut plateau = create_almost_complete_line();
        let completing_pos = 8;
        let other_pos = 0;

        let score_complete = pattern_score(&plateau, completing_pos, &tile);
        let score_other = pattern_score(&plateau, other_pos, &tile);

        assert!(score_complete > score_other * 2.0);
    }

    #[test]
    fn test_dirichlet_adds_exploration() {
        let policy = vec![0.1; 19];
        let noisy = apply_dirichlet_noise(&policy, 0.3, 0.25);

        // Entropy should increase
        let entropy_before = compute_entropy(&policy);
        let entropy_after = compute_entropy(&noisy);
        assert!(entropy_after >= entropy_before * 0.9);
    }
}
```

### Tests d'Intégration

```bash
# Benchmark reproductible
./target/release/benchmark_integration \
    --seed 2025 \
    --games 100 \
    --strategies "pure,cnn,hybrid,qnet" \
    --output integration_results.csv

# Vérifier non-régression
diff integration_results.csv baseline_results.csv
```

---

## 📈 MÉTRIQUES DE SUCCÈS

### Objectifs Par Phase

| Phase | Score Cible | Win Rate | Policy Loss | Status |
|-------|-------------|----------|-------------|--------|
| **P0** | Baseline 125 | 74% | 2.944 | Current |
| **P1** | 140+ pts | 78% | - | - |
| **P2** | 143+ pts | 80% | - | - |
| **P3** | 147+ pts | 82% | < 2.5 | - |
| **P4** | **150+ pts** | **85%** | **< 2.0** | Target |

### KPIs Finaux

- **Score moyen:** > 145 pts ✅
- **Score minimum:** > 90 pts
- **Écart-type:** < 25 pts
- **Win rate vs Pure MCTS:** > 80%
- **Win rate vs CNN seul:** > 75%

---

## ⚠️ RISQUES & MITIGATIONS

| Risque | Probabilité | Impact | Mitigation |
|--------|-------------|--------|------------|
| Poids CNN irrécupérables | Moyen | Élevé | Ré-entraîner avec données fixes |
| Bug expert data non corrigeable | Faible | Élevé | Générer données via self-play |
| Overfitting Q-net | Moyen | Moyen | Validation croisée, dropout |
| Régression Pattern V3 | Élevé | Moyen | Garder V2 comme fallback |
| Self-play circulaire | Moyen | Élevé | Dirichlet noise + température |

---

## 📁 FICHIERS À MODIFIER

### Priorité Haute

| Fichier | Modification | Phase |
|---------|--------------|-------|
| `model_weights/cnn/*/` | Restaurer poids fonctionnels | P1 |
| `src/bin/expert_data_generator.rs` | Fix bug uniform data | P1 |
| `src/mcts/algorithm.rs` | Hybrid optimisé | P2 |
| `src/bin/alphago_zero_trainer.rs` | Dirichlet noise | P3 |

### Priorité Moyenne

| Fichier | Modification | Phase |
|---------|--------------|-------|
| `src/mcts/hyperparameters.rs` | Tune w_cnn si restauré | P2 |
| `src/game/simulate_game_smart.rs` | Pattern V3 (optionnel) | P4 |
| `src/neural/manager.rs` | Trait abstraction | P4 |

---

## 🏁 CONCLUSION

**Stratégie recommandée:**

1. **Phase 1 (CRITIQUE):** Restaurer poids CNN → 140 pts
2. **Phase 2:** Combiner Q-Net + CNN → 143 pts
3. **Phase 3:** Dirichlet self-play → 147 pts
4. **Phase 4:** Fine-tuning → **150+ pts**

**Temps total estimé:** 5-7 jours

**Probabilité de succès:** 75-85%

**Facteur de succès principal:** Récupération des poids CNN historiques. Si impossible, le ré-entraînement ajoute 2-3 jours.

---

**Prochaine action immédiate:**

```bash
# Chercher les poids historiques
git log --all --oneline --source -- "model_weights/" | head -20
git branch -a | xargs -I{} git log {} --oneline -1 -- "model_weights/"
```

---

**Document:** `docs/ARCHITECTURE_PLAN_145_PTS_2026-01-24.md`
**Auteur:** Claude Opus 4.5
**Date:** 2026-01-24
