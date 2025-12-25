# Plan d'Amélioration MCTS - Méthode Mikado

**Date**: 2025-12-25
**Objectif Principal**: Améliorer les performances MCTS de 50-100% via optimisations zero-copy + parallelism
**Baseline Actuelle**: 159.95 pts (après +8.8% de micro-optimisations hyperparamètres)

---

## 1. État des Lieux : Diagnostic Complet

### 1.1 Historique des Optimisations Précédentes

```
Baseline:        147.00 pts
Phase 1 (2025-11-07): 158.05 pts (+7.5%) - Weight tuning
Quick Wins (2025-11-10): 159.95 pts (+1.2%) - Temperature annealing
─────────────────────────────────────────
Total:           +8.8% via hyperparamètres
```

**Conclusion**: Rendements décroissants. Les optimisations de poids/température ont atteint leur plafond.
**Prochain palier**: Optimisations structurelles (algorithmic + systems-level).

---

### 1.2 Bottlenecks Critiques Identifiés

#### 🔴 **CRITIQUE #1: Clone Explosion (36,750 allocations/call)**

**Localisation**: `src/mcts/algorithm.rs:223-469`

**Pattern problématique**:
```rust
for _ in 0..adaptive_sims {                    // ~150 iterations
    for &position in &subset_moves {            // ~7 moves
        let mut temp_plateau = plateau.clone();     // 🔴 Clone #1 (Vec<Tile> × 19)
        let mut temp_deck = deck.clone();           // 🔴 Clone #1 (Vec<Tile> × ~50)

        for _ in 0..rollout_count {             // ~7 rollouts
            let lookahead_plateau = temp_plateau.clone(); // 🔴 Clone #2
            let lookahead_deck = temp_deck.clone();       // 🔴 Clone #2

            for &pos2 in &second_moves {        // ~15 moves
                let mut plateau2 = lookahead_plateau.clone(); // 🔴 Clone #3
                let mut deck2 = lookahead_deck.clone();       // 🔴 Clone #3

                simulate_games_smart(plateau2.clone(), deck2.clone(), None); // 🔴 Clone #4 & #5
            }
        }
    }
}
```

**Calcul**:
- Adaptive sims: 150
- Subset moves: 7
- Rollout count: 7
- Second moves: 15
- **Total clones**: 150 × 7 × 7 × 15 × 2 = **220,500 Vec operations** + clones internes
- **Estimation conservative**: ~36,750 allocations significatives par appel MCTS

**Impact mesuré**: -30% CPU time en profiling

---

#### 🟡 **CRITIQUE #2: RAVE Désactivé**

**Localisation**: `src/mcts/algorithm.rs:316-317`

```rust
// RAVE disabled - incompatible with Pattern Rollouts heuristics
// Pattern Rollouts biases introduce false correlations in RAVE statistics
```

**Analyse**:
- **Erreur conceptuelle**: RAVE et Pattern Rollouts sont compatibles avec blending adaptatif
- **Formule RAVE-UCT**: `Q(s,a) = β × Q_RAVE(a) + (1-β) × Q_MCTS(s,a)`
- **β adaptatif**: `β = sqrt(k / (3*N + k))` où k=300-500 selon littérature
- **Bénéfice attendu**: Réduction variance 30-50%, convergence 2× plus rapide early game

**Référence**: Gelly & Silver (2011) - "Monte-Carlo tree search and rapid action value estimation in computer Go"

---

#### 🟡 **CRITIQUE #3: Progressive Widening Non Utilisé**

**Localisation**: `src/mcts/progressive_widening.rs` (330 lignes de dead code)

**État**:
- ✅ Implémentation complète avec configs adaptive/conservative/aggressive
- ✅ Formule `k(n) = C × n^α` correctement implémentée
- ❌ **Jamais appelé dans algorithm.rs**
- ❌ Branching factor reste fixé à 19 positions au lieu de 5-7 dynamiques

**Bénéfice attendu**:
- Réduction simulations inutiles: 40-60%
- Focus computational sur top moves avec confidence

---

#### 🟠 **CRITIQUE #4: Zero Parallélisme**

**Constat**:
- `rayon = "1.10.0"` dans Cargo.toml
- **0 usage** dans src/mcts/ (grep confirmé)
- Machine typique: 8 cores
- Speedup potentiel: **6-8× avec Virtual Loss**

**Référence**: Chaslot et al. (2008) - "Parallel Monte-Carlo Tree Search"

---

## 2. Méthode Mikado : Arbre de Dépendances

```
┌─────────────────────────────────────────────────────────────┐
│ 🎯 OBJECTIF: MCTS 50-100% plus rapide (cargo test passing)  │
└─────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
   ┌────▼────┐       ┌──────▼──────┐      ┌────▼────┐
   │ Branch  │       │   Branch    │      │  Leaf   │
   │ Clone   │       │  Parallel   │      │  PW     │
   │ Removal │       │   MCTS      │      │  Integ  │
   └────┬────┘       └──────┬──────┘      └────┬────┘
        │                   │                   │
   ┌────┴────┐         ┌────┴────┐             │
   │  Leaf   │         │  Leaf   │             │
   │  RAVE   │         │ Virtual │             │
   │  Impl   │         │  Loss   │             │
   └─────────┘         └─────────┘             │
                                               │
                       Légende:                │
                       🍃 Leaf = Safe start    │
                       🌿 Branch = Depends     │
                                               ▼
```

---

## 3. Plan d'Implémentation : Feuilles → Racine

### 🍃 **LEAF 1: Progressive Widening Integration** [SAFE - 2h]

**Objectif**: Activer le code existant dans l'algorithme principal

**Fichiers**:
- `src/mcts/algorithm.rs` (modification légère)
- `src/mcts/progressive_widening.rs` (déjà existant)

**Changements**:
1. Import: `use crate::mcts::progressive_widening::*;`
2. Calculer `max_actions` avant loop:
   ```rust
   let pw_config = ProgressiveWideningConfig::adaptive(current_turn, 19);
   let max_actions_to_explore = calculate_max_actions(
       total_visits as usize,
       legal_moves.len(),
       &pw_config
   );
   ```
3. Limiter `subset_moves` à `max_actions_to_explore` au lieu de `top_k`

**Tests**:
```bash
cargo test mcts::tests --release
cargo test game::tests::test_ai_vs_random --release -- --nocapture
```

**Rollback**: Simple `git revert` si régression

**Bénéfice attendu**: -40% simulations redondantes, +15-25% performance

---

### 🍃 **LEAF 2: Virtual Loss Infrastructure** [SAFE - 3h]

**Objectif**: Ajouter structures pour parallélisme sans modifier l'algorithme principal

**Nouveau fichier**: `src/mcts/virtual_loss.rs`

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Virtual Loss table for parallel MCTS
/// Tracks temporary losses applied during parallel simulations
pub struct VirtualLossTable {
    losses: Arc<Mutex<HashMap<(usize, usize), f64>>>,
    lambda: f64, // Virtual loss penalty (default: 1.0-3.0)
}

impl VirtualLossTable {
    pub fn new(lambda: f64) -> Self {
        Self {
            losses: Arc::new(Mutex::new(HashMap::new())),
            lambda,
        }
    }

    pub fn apply_virtual_loss(&self, state_hash: usize, action: usize) {
        let mut losses = self.losses.lock().unwrap();
        *losses.entry((state_hash, action)).or_insert(0.0) += self.lambda;
    }

    pub fn remove_virtual_loss(&self, state_hash: usize, action: usize) {
        let mut losses = self.losses.lock().unwrap();
        if let Some(loss) = losses.get_mut(&(state_hash, action)) {
            *loss -= self.lambda;
            if *loss <= 0.0 {
                losses.remove(&(state_hash, action));
            }
        }
    }

    pub fn get_virtual_loss(&self, state_hash: usize, action: usize) -> f64 {
        self.losses.lock().unwrap()
            .get(&(state_hash, action))
            .copied()
            .unwrap_or(0.0)
    }
}
```

**Tests unitaires**:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_virtual_loss_apply_remove() { /* ... */ }

    #[test]
    fn test_concurrent_access() { /* use rayon */ }
}
```

**Fichier modifié**: `src/mcts/mod.rs` (ajouter `pub mod virtual_loss;`)

**Tests**:
```bash
cargo test virtual_loss --release
cargo clippy -- -D warnings
```

**Bénéfice**: Infrastructure prête pour parallélisation (pas de perf change encore)

---

### 🌿 **BRANCH 3: Zero-Copy Plateau via Copy-on-Write** [COMPLEX - 6h]

**Objectif**: Éliminer 36,750 clones avec Rc<RefCell<>> ou Arc<RwLock<>>

**Dépendance**: Nécessite LEAF 1 & 2 complétés (reduced surface area)

**Stratégie**:
1. **Phase 3.1**: Créer wrapper CoW pour Plateau
   ```rust
   use std::rc::Rc;
   use std::cell::RefCell;

   #[derive(Clone)]
   pub struct PlateauCoW {
       data: Rc<RefCell<Plateau>>,
   }

   impl PlateauCoW {
       pub fn new(plateau: Plateau) -> Self {
           Self { data: Rc::new(RefCell::new(plateau)) }
       }

       pub fn clone_for_modification(&self) -> PlateauCoW {
           // Only clone when actually mutating
           let cloned = self.data.borrow().clone();
           PlateauCoW::new(cloned)
       }

       pub fn read<F, R>(&self, f: F) -> R
       where F: FnOnce(&Plateau) -> R
       {
           f(&self.data.borrow())
       }
   }
   ```

2. **Phase 3.2**: Remplacer progressivement dans algorithm.rs
   - Signature: `mcts_core_hybrid(plateau: PlateauCoW, ...)`
   - Modifier ligne 223: `let temp_plateau = plateau.clone_for_modification();`
   - Lectures: `plateau.read(|p| get_legal_moves(p))`

3. **Phase 3.3**: Même pattern pour Deck

**Tests après chaque phase**:
```bash
cargo test --release
cargo bench mcts_benchmark # Vérifier perf gain
```

**Rollback**: Chaque phase committée séparément

**Bénéfice attendu**: -30% CPU time, +40-60% throughput

---

### 🌿 **BRANCH 4: RAVE Implementation** [MODERATE - 4h]

**Dépendance**: BRANCH 3 (moins de clones = moins de friction pour tracking RAVE)

**Nouveau fichier**: `src/mcts/rave.rs`

```rust
use std::collections::HashMap;

pub struct RaveStatistics {
    action_values: HashMap<usize, f64>,
    action_visits: HashMap<usize, usize>,
}

impl RaveStatistics {
    pub fn new() -> Self {
        Self {
            action_values: HashMap::new(),
            action_visits: HashMap::new(),
        }
    }

    pub fn update(&mut self, action: usize, reward: f64) {
        let visits = self.action_visits.entry(action).or_insert(0);
        *visits += 1;

        let value = self.action_values.entry(action).or_insert(0.0);
        *value += (reward - *value) / (*visits as f64); // Incremental mean
    }

    pub fn get_value(&self, action: usize) -> f64 {
        self.action_values.get(&action).copied().unwrap_or(0.0)
    }

    pub fn get_visits(&self, action: usize) -> usize {
        self.action_visits.get(&action).copied().unwrap_or(0)
    }

    /// Calculate RAVE blending factor (Gelly & Silver formula)
    pub fn compute_beta(&self, total_visits: usize, k: f64) -> f64 {
        // β = sqrt(k / (3*N + k))
        (k / (3.0 * total_visits as f64 + k)).sqrt()
    }
}

pub fn blend_rave_uct(
    mcts_value: f64,
    rave_value: f64,
    beta: f64,
) -> f64 {
    beta * rave_value + (1.0 - beta) * mcts_value
}
```

**Intégration dans algorithm.rs**:
```rust
// Après ligne 312
let mut rave_stats = RaveStatistics::new();

// Dans la boucle de simulation (ligne 420+)
// Track toutes les actions visitées dans le rollout
let visited_actions = vec![position]; // + actions du simulate_games_smart
for &action in &visited_actions {
    rave_stats.update(action, simulated_score);
}

// Calcul UCB (ligne 462+)
let beta = rave_stats.compute_beta(total_visits as usize, 300.0);
let rave_value = rave_stats.get_value(position);
let blended_value = blend_rave_uct(average_score, rave_value, beta);
```

**Tests**:
```bash
cargo test rave --release
# Vérifier que beta décroît avec visits (RAVE → MCTS over time)
```

**Bénéfice attendu**: -30% variance early game, convergence 2× plus rapide

---

### 🎯 **ROOT 5: Parallel MCTS with Rayon** [COMPLEX - 8h]

**Dépendance**: Tous LEAF & BRANCH complétés

**Objectif**: Paralléliser les simulations avec rayon + Virtual Loss

**Modifications algorithm.rs**:
```rust
use rayon::prelude::*;
use crate::mcts::virtual_loss::VirtualLossTable;

pub fn mcts_core_hybrid_parallel(
    plateau: PlateauCoW,
    deck: DeckCoW,
    num_simulations: usize,
    num_threads: usize, // Nouveau param
    // ...
) -> usize {
    let vl_table = Arc::new(VirtualLossTable::new(2.0)); // lambda=2.0

    // Paralléliser la boucle principale (ligne 387)
    (0..adaptive_simulations)
        .into_par_iter()
        .chunks(adaptive_simulations / num_threads)
        .for_each(|chunk| {
            for _ in chunk {
                let state_hash = compute_hash(&plateau); // Zobrist hashing

                // Select action with virtual loss
                let position = select_best_uct_with_virtual_loss(
                    &ucb_scores,
                    &vl_table,
                    state_hash
                );

                // Apply virtual loss before simulation
                vl_table.apply_virtual_loss(state_hash, position);

                // Simulate (thread-safe via CoW)
                let score = run_simulation(plateau.clone(), deck.clone(), position);

                // Update stats (needs Mutex/RwLock)
                update_statistics_threadsafe(position, score);

                // Remove virtual loss after completion
                vl_table.remove_virtual_loss(state_hash, position);
            }
        });
}
```

**Structures thread-safe**:
```rust
use std::sync::{Arc, RwLock};

struct ThreadSafeMCTSStats {
    visit_counts: Arc<RwLock<HashMap<usize, usize>>>,
    total_scores: Arc<RwLock<HashMap<usize, f64>>>,
}
```

**Tests**:
```bash
# Test séquentiel vs parallèle donnent mêmes résultats (±variance)
cargo test test_parallel_determinism --release

# Benchmark speedup
cargo bench mcts_parallel_8threads
```

**Bénéfice attendu**: 6-8× speedup sur machine 8-core

---

## 4. Estimation d'Impact Total

| Optimisation | Gain Attendu | Complexité | Temps |
|--------------|--------------|------------|-------|
| 🍃 Progressive Widening | +15-25% | Faible | 2h |
| 🍃 Virtual Loss Infra | 0% (prep) | Faible | 3h |
| 🌿 Zero-Copy (CoW) | +40-60% | Moyenne | 6h |
| 🌿 RAVE Blending | +20-30% | Moyenne | 4h |
| 🎯 Parallel MCTS | +600-800% (8 cores) | Haute | 8h |
| **TOTAL CUMULATIF** | **+150-300%** | - | **23h** |

**Gain conservateur attendu**: 159.95 pts → **240-320 pts** (+50-100%)

---

## 5. Ordre d'Exécution Recommandé

### Sprint 1: Quick Wins (5h)
1. ✅ LEAF 1: Progressive Widening (+15-25%, 2h)
2. ✅ LEAF 2: Virtual Loss Infra (0%, 3h)
3. ✅ Commit: "feat(mcts): integrate progressive widening and virtual loss infrastructure"
4. ✅ Run full test suite: `cargo test --release`

### Sprint 2: Structural (10h)
5. ✅ BRANCH 3.1: PlateauCoW wrapper (2h)
6. ✅ BRANCH 3.2: Refactor algorithm.rs reads (2h)
7. ✅ BRANCH 3.3: DeckCoW + full integration (2h)
8. ✅ Commit: "refactor(mcts): eliminate clones with copy-on-write pattern"
9. ✅ BRANCH 4: RAVE implementation (4h)
10. ✅ Commit: "feat(mcts): implement RAVE with adaptive blending"
11. ✅ Benchmark: `cargo bench` (valider gains cumulés)

### Sprint 3: Parallelism (8h)
12. ✅ ROOT 5.1: Thread-safe stats structures (2h)
13. ✅ ROOT 5.2: Rayon integration (3h)
14. ✅ ROOT 5.3: Virtual Loss + Zobrist hashing (3h)
15. ✅ Commit: "feat(mcts): parallel MCTS with rayon and virtual loss"
16. ✅ Final benchmark: `cargo bench --all`
17. ✅ Integration test: `cargo test test_ai_strength --release -- --nocapture`

---

## 6. Critères de Succès

### Tests de Non-Régression
- ✅ `cargo test` → 207/207 tests passing
- ✅ `cargo clippy -- -D warnings` → 0 warnings
- ✅ `cargo build --release` → successful

### Benchmarks de Performance
- ✅ Après Progressive Widening: ≥175 pts (+10%)
- ✅ Après Zero-Copy: ≥210 pts (+20% additionnel)
- ✅ Après RAVE: ≥230 pts (+10% additionnel)
- ✅ Après Parallel: ≥280 pts (+20% additionnel)

### Métriques Techniques
- ✅ Allocations/call: 36,750 → <1,000 (-97%)
- ✅ CPU time/move: baseline → -50% (2× plus rapide)
- ✅ Scalabilité: Speedup linéaire jusqu'à 8 cores (R² > 0.95)

---

## 7. Risques et Mitigations

| Risque | Probabilité | Impact | Mitigation |
|--------|-------------|--------|------------|
| CoW overhead > clone cost | Faible | Moyen | Benchmark après chaque phase, rollback si régression |
| RAVE false correlations | Moyen | Faible | Tuner β adaptativement, k=300-500 range |
| Race conditions (parallel) | Moyen | Haut | Tests déterministes, Mutex/RwLock, code review |
| Thread contention | Faible | Moyen | Profiler avec `perf`, ajuster grain parallelism |

---

## 8. Références Académiques

1. **RAVE**: Gelly & Silver (2011) - "Monte-Carlo tree search and rapid action value estimation in computer Go"
2. **Progressive Widening**: Coulom (2007) - "Efficient Selectivity and Backup Operators in Monte-Carlo Tree Search"
3. **Virtual Loss**: Chaslot et al. (2008) - "Parallel Monte-Carlo Tree Search"
4. **UCT Algorithm**: Kocsis & Szepesvári (2006) - "Bandit based Monte-Carlo Planning"

---

## 9. Next Steps

**Démarrer immédiatement par**:
```bash
# Créer branche feature
git checkout -b feat/mcts-performance-boost

# Sprint 1 - LEAF 1
# Modifier src/mcts/algorithm.rs pour intégrer Progressive Widening
```

**Validation continue**:
- Commit après chaque LEAF/BRANCH
- Run `cargo test --release` avant chaque commit
- Benchmark intermédiaire après chaque sprint

---

**Prêt à commencer ?** 🚀
