# Optimization Roadmap - Post Phase 1
## Date: 2025-11-07

---

## ✅ État Actuel
- **Score**: 158.05 pts (Phase 1 optimisée)
- **Baseline**: 147 pts
- **Amélioration**: +11 pts (+7.5%)

---

## 🎯 Voies d'Amélioration Validées

### 1. Phase 2: c_puct Grid Search ⭐ PRIORITÉ #1

**Paramètres à optimiser**:
```rust
c_puct_early: [3.8, 4.2, 4.6]         // actuellement 4.2
c_puct_mid: [3.4, 3.8, 4.2]           // actuellement 3.8
c_puct_late: [2.6, 3.0, 3.4]          // actuellement 3.0
variance_mult_high: [1.2, 1.3, 1.4]   // actuellement 1.3
variance_mult_low: [0.80, 0.85, 0.90] // actuellement 0.85
```

**Grid size**: 3×3×3×3×3 = 243 configs (filtrer à ~100 configs valides)
**Games per config**: 10 (rapide) ou 20 (précis)
**Total compute**: ~20h (10 games) ou ~40h (20 games)
**Gain attendu**: +1-2 pts → **159-160 pts**
**Risque**: 🟢 Faible (juste tuning)

**Implémentation**: Créer `src/bin/grid_search_phase2.rs`

---

### 2. Phase 3: Rollout + Pruning ⭐ PRIORITÉ #2

**Paramètres à optimiser**:
```rust
// Rollout counts
rollout_strong: [2, 3, 4]     // actuellement 3
rollout_medium: [4, 5, 6]     // actuellement 5
rollout_default: [6, 7, 8]    // actuellement 7
rollout_weak: [8, 9, 10]      // actuellement 9

// Pruning ratios
prune_early: [0.03, 0.05, 0.07]   // actuellement 0.05
prune_mid1: [0.08, 0.10, 0.12]    // actuellement 0.10
prune_mid2: [0.13, 0.15, 0.17]    // actuellement 0.15
prune_late: [0.18, 0.20, 0.22]    // actuellement 0.20
```

**Grid size**: 3^4 × 3^4 = 6561 configs (trop!)
**Solution**: Grid search séquentiel (rollouts puis pruning)
**Total compute**: ~10h + ~10h = 20h
**Gain attendu**: +0.5-1.5 pts → **160-161.5 pts**
**Risque**: 🟢 Faible

---

### 3. Simulation Budget Adaptatif 🚀 QUICK WIN

**Implémentation simple**:
```rust
pub fn adaptive_simulations(turn: usize, total_turns: usize) -> usize {
    let progress = turn as f64 / total_turns as f64;

    if progress < 0.25 {
        100  // Début: exploration rapide
    } else if progress < 0.75 {
        150  // Milieu: équilibré
    } else {
        250  // Fin: décisions critiques
    }
}
```

**Coût**: Quasi nul (moins de sims en début compensé par plus en fin)
**Gain attendu**: +0.5-1 pt → **161-162 pts**
**Risque**: 🟢 Très faible
**Effort**: 30 minutes

---

### 4. Temperature Annealing 🌡️ QUICK WIN

**Implémentation**:
```rust
// Réduire l'exploration progressivement
let temperature = if turn < 5 {
    1.5  // Haute exploration début
} else if turn < 15 {
    1.0 - (turn as f64 - 5.0) / 10.0  // Décroissance linéaire
} else {
    0.5  // Exploitation pure fin de jeu
};

// Appliquer dans UCB
let exploration = temperature * sqrt(ln(parent_visits) / child_visits);
```

**Gain attendu**: +0.5 pt → **162-163 pts**
**Risque**: 🟢 Très faible
**Effort**: 15 minutes

---

### 5. JETA (Just Enough Time Algorithm) ⏱️ NOUVEAU

**Concept**: Allouer le temps intelligemment entre les coups

#### 5a. Time Budget par Phase
```rust
pub struct TimeManager {
    total_time_ms: u64,
    turns_remaining: usize,
    time_used: u64,
}

impl TimeManager {
    pub fn allocate_time(&self, turn: usize, total_turns: usize) -> u64 {
        let progress = turn as f64 / total_turns as f64;
        let base_time = self.total_time_ms / total_turns as u64;

        // Plus de temps pour les coups critiques
        if progress < 0.2 {
            base_time * 80 / 100  // -20% début
        } else if progress < 0.8 {
            base_time               // Normal milieu
        } else {
            base_time * 150 / 100   // +50% fin
        }
    }
}
```

#### 5b. Adaptive Termination
```rust
// Arrêter tôt si décision claire
pub fn should_stop_search(
    best_score: f64,
    second_best: f64,
    visits: usize,
    min_visits: usize,
) -> bool {
    if visits < min_visits {
        return false;
    }

    // Écart > 30% et > 50 visites
    let gap = (best_score - second_best).abs() / best_score;
    gap > 0.3 && visits > 50
}
```

#### 5c. Progressive Deepening
```rust
// Augmenter progressivement les simulations
pub fn progressive_search(
    position: &Position,
    max_time: Duration,
) -> Move {
    let mut depth = 50;
    let start = Instant::now();
    let mut best_move = None;

    while start.elapsed() < max_time {
        let result = mcts_search(position, depth);
        best_move = Some(result.best_move);

        if result.is_decisive {
            break;  // Décision claire
        }

        depth += 50;  // Augmenter progressivement
    }

    best_move.unwrap()
}
```

**Gain attendu**: +1-2 pts (via meilleure allocation)
**Risque**: 🟡 Moyen (nouveau système)
**Effort**: 1 semaine

---

## 📊 Timeline Optimale

### Semaine 1: Quick Wins (0 risque)
- [x] Phase 1 complete (158 pts)
- [ ] Adaptive simulations (+0.5-1 pt) → 159 pts
- [ ] Temperature annealing (+0.5 pt) → 159.5 pts
- [ ] Phase 2 grid search (+1-2 pts) → 161 pts

**Total**: 158 → **161 pts** (+3 pts, +1.9%)

### Semaine 2: Phase 3 + JETA Foundation
- [ ] Phase 3 rollouts grid search (+0.5 pt) → 161.5 pts
- [ ] Phase 3 pruning grid search (+0.5 pt) → 162 pts
- [ ] JETA time manager (+0.5 pt) → 162.5 pts

**Total**: 161 → **162.5 pts** (+1.5 pts)

### Semaine 3: JETA Advanced
- [ ] Adaptive termination (+0.5 pt) → 163 pts
- [ ] Progressive deepening (+0.5 pt) → 163.5 pts
- [ ] Integration complète (+0.5 pt) → 164 pts

**Total**: 162.5 → **164 pts** (+1.5 pts)

---

## 🎯 Objectifs Réalistes

| Timeline | Score | Amélioration | Méthode |
|----------|-------|--------------|---------|
| **Actuel** | 158 pts | - | Phase 1 |
| **+1 semaine** | 161 pts | +3 pts | Quick wins + Phase 2 |
| **+2 semaines** | 162.5 pts | +4.5 pts | Phase 3 + JETA base |
| **+3 semaines** | 164 pts | +6 pts | JETA complet |

**ROI décroissant attendu**: Chaque point devient plus difficile

---

## ❌ À NE PAS Refaire (Échecs Prouvés)

1. **Expectimax**: -141 pts (4 niveaux d'échec documentés)
2. **Gumbel**: -83 pts (manque value initialization)
3. **CVaR**: -6 pts (risk sensitivity contre-productive)
4. **Progressive Widening**: -5 pts (pas adapté à notre domaine)
5. **Gold GNN direct**: -21 pts (supprime exploration MCTS)

---

## 🔬 Expérimentations Optionnelles (Plus risquées)

### Neural Network Re-training (Risque moyen)
```bash
# Régénérer données expertes avec nouveaux hyperparams
cargo run --release --bin optimal_data_generator -- \
  --num-games 1000 \
  --beam-width 50

# Réentraîner
python train_supervised.py
```

**Gain potentiel**: +3-5 pts
**Effort**: 2-3 semaines
**Risque**: 🟡 Moyen (peut régresser)

### Virtual Loss (Parallélisation)
```rust
use rayon::prelude::*;

// Paralléliser simulations
(0..num_simulations)
    .into_par_iter()
    .for_each(|_| {
        tree.run_simulation_with_virtual_loss();
    });
```

**Gain potentiel**: +2-3 pts (via 300-500 sims)
**Effort**: 1 semaine
**Risque**: 🟡 Moyen (bugs de concurrence)

---

## 🎮 JETA Détaillé

### Composants JETA

1. **TimeManager**: Budget global
2. **AdaptiveTermination**: Stop quand décision claire
3. **ProgressiveDeepening**: Iterative deepening
4. **CriticalMoveDetection**: Identifier coups importants

### Métriques JETA
```rust
struct JETAMetrics {
    time_saved: Duration,
    early_terminations: usize,
    critical_moves_detected: usize,
    avg_depth_reached: f64,
}
```

### Test JETA
```bash
cargo run --release --bin test_jeta -- \
  --games 20 \
  --time-budget-ms 30000 \
  --compare-with-fixed
```

---

## 📈 Prévisions Finales

**Conservative** (tout fonctionne moyennement):
- 158 → 162 pts (+4 pts, +2.5%)

**Realistic** (quick wins + grid search):
- 158 → 164 pts (+6 pts, +3.8%)

**Optimistic** (tout fonctionne parfaitement):
- 158 → 166 pts (+8 pts, +5%)

**Limite estimée** (sans changer l'algo):
- ~170 pts (après 3-6 mois d'optimisation)

---

## ✅ Prochaines Actions

1. **Implémenter adaptive_simulations** (30 min)
2. **Implémenter temperature_annealing** (15 min)
3. **Tester quick wins** (1h)
4. **Créer grid_search_phase2.rs** (2-3h)
5. **Lancer Phase 2 overnight** (20-40h compute)

---

**Author**: Optimization Analysis
**Date**: 2025-11-07
**Status**: ROADMAP DÉFINI
