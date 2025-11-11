# Hyperparameter Tuning Strategy - Take It Easy MCTS

## Executive Summary

**Objective**: Optimize baseline MCTS hyperparameters for +1-2 pts gain (144 → 145-146 pts)
**Approach**: Grid search followed by fine-tuning on most promising parameters
**Effort**: 1 week implementation + 24h compute
**Risk**: 🟢 Low (no architectural changes)
**Status**: Ready to implement

---

## Current Baseline Performance

- **Score**: 143.98 ± 26.52 pts (documented)
- **Quick Test**: 148.80 ± 22.21 pts (10 games, seed 2025)
- **Method**: CNN Curriculum + Pattern Rollouts V2

---

## Key Hyperparameters Identified

### Category 1: MCTS Exploration (c_puct)
**Location**: `src/mcts/algorithm.rs:326-345`

```rust
// Current values
let base_c_puct = if current_turn < 5 {
    4.2  // Early game
} else if current_turn > 15 {
    3.0  // Late game
} else {
    3.8  // Mid game
};

// Variance multiplier: 0.85 - 1.3
```

**Search Space**:
- `c_puct_early`: [3.0, 3.5, 4.0, **4.2**, 4.5, 5.0]
- `c_puct_mid`: [3.0, 3.5, **3.8**, 4.0, 4.5]
- `c_puct_late`: [2.5, **3.0**, 3.5, 4.0]
- `variance_mult_high`: [1.2, **1.3**, 1.4, 1.5]
- `variance_mult_low`: [0.75, **0.85**, 0.9, 1.0]

**Expected Impact**: Medium (+0.5-1.0 pts)
**Reason**: c_puct controls exploration vs exploitation balance

---

### Category 2: Dynamic Pruning
**Location**: `src/mcts/algorithm.rs:360-368`

```rust
// Current values
let pruning_ratio = if current_turn < 5 {
    0.05  // Keep 95% of moves
} else if current_turn < 10 {
    0.10  // Keep 90%
} else if current_turn < 15 {
    0.15  // Keep 85%
} else {
    0.20  // Keep 80%
};
```

**Search Space**:
- `prune_early`: [0.00, **0.05**, 0.10] (keep 100%, 95%, 90%)
- `prune_mid1`: [0.05, **0.10**, 0.15] (keep 95%, 90%, 85%)
- `prune_mid2`: [0.10, **0.15**, 0.20] (keep 90%, 85%, 80%)
- `prune_late`: [0.15, **0.20**, 0.25] (keep 85%, 80%, 75%)

**Expected Impact**: Low-Medium (+0.2-0.5 pts)
**Reason**: Affects search space reduction

---

### Category 3: Adaptive Rollout Count
**Location**: `src/mcts/algorithm.rs:420-425`

```rust
// Current values
let rollout_count = match value_estimate {
    x if x > 0.7 => 3,   // Very strong
    x if x > 0.2 => 5,   // Strong
    x if x < -0.4 => 9,  // Weak
    _ => 7,              // Default
};
```

**Search Space**:
- `rollout_strong`: [2, **3**, 4]
- `rollout_medium`: [4, **5**, 6]
- `rollout_default`: [6, **7**, 8]
- `rollout_weak`: [8, **9**, 10]

**Expected Impact**: Medium (+0.3-0.8 pts)
**Reason**: Balances compute budget vs accuracy

---

### Category 4: Evaluation Weights (Pattern Rollouts V2)
**Location**: `src/mcts/algorithm.rs:489-492`

```rust
// Current values
let combined_eval = 0.6 * normalized_value      // CNN
                  + 0.2 * normalized_rollout    // Simulation
                  + 0.1 * normalized_heuristic  // Domain
                  + 0.1 * contextual;           // Entropy
```

**Search Space**:
- `weight_cnn`: [0.5, 0.55, **0.6**, 0.65, 0.7]
- `weight_rollout`: [0.1, 0.15, **0.2**, 0.25, 0.3]
- `weight_heuristic`: [0.05, **0.1**, 0.15, 0.2]
- `weight_contextual`: [0.05, **0.1**, 0.15]
- **Constraint**: sum = 1.0

**Expected Impact**: High (+0.5-1.5 pts)
**Reason**: Directly affects move quality evaluation

---

## Optimization Strategy

### Phase 1: Grid Search on High-Impact Parameters (3-4 days)
**Focus**: Evaluation weights (Category 4)

**Grid**:
```
weight_cnn:       [0.55, 0.6, 0.65]
weight_rollout:   [0.15, 0.2, 0.25]
weight_heuristic: [0.05, 0.1, 0.15]
weight_contextual: [0.05, 0.1, 0.15]
```

**Combinations**: ~20-30 valid combinations (where sum ≈ 1.0)
**Games per config**: 20 games
**Total games**: 400-600
**Estimated time**: 6-8 hours

**Expected Outcome**: Best weight configuration

---

### Phase 2: c_puct Tuning (2-3 days)
**Focus**: MCTS exploration (Category 1)

Using best weights from Phase 1, test:
```
c_puct_early: [3.5, 4.0, 4.2, 4.5]
c_puct_mid:   [3.5, 3.8, 4.0]
c_puct_late:  [2.5, 3.0, 3.5]
```

**Combinations**: ~36
**Games per config**: 20 games
**Total games**: 720
**Estimated time**: 10-12 hours

---

### Phase 3: Rollout Count Optimization (1-2 days)
**Focus**: Adaptive rollouts (Category 3)

Using best weights + c_puct from Phase 1-2, test:
```
rollout_strong:  [2, 3, 4]
rollout_default: [6, 7, 8]
rollout_weak:    [8, 9, 10]
```

**Combinations**: ~27
**Games per config**: 20 games
**Total games**: 540
**Estimated time**: 8-10 hours

---

### Phase 4: Fine-Tuning (1 day)
**Focus**: Pruning ratios (Category 2) - optional

Only if phases 1-3 show significant gains (>1 pt).

---

## Implementation Plan

### Step 1: Create Hyperparameter Config Structure

```rust
// src/mcts/hyperparameters.rs
pub struct MCTSHyperparameters {
    // c_puct
    pub c_puct_early: f64,      // default: 4.2
    pub c_puct_mid: f64,        // default: 3.8
    pub c_puct_late: f64,       // default: 3.0
    pub variance_mult_high: f64, // default: 1.3
    pub variance_mult_low: f64,  // default: 0.85

    // Pruning
    pub prune_early: f64,       // default: 0.05
    pub prune_mid1: f64,        // default: 0.10
    pub prune_mid2: f64,        // default: 0.15
    pub prune_late: f64,        // default: 0.20

    // Rollouts
    pub rollout_strong: usize,  // default: 3
    pub rollout_medium: usize,  // default: 5
    pub rollout_default: usize, // default: 7
    pub rollout_weak: usize,    // default: 9

    // Evaluation weights
    pub weight_cnn: f64,        // default: 0.6
    pub weight_rollout: f64,    // default: 0.2
    pub weight_heuristic: f64,  // default: 0.1
    pub weight_contextual: f64, // default: 0.1
}

impl Default for MCTSHyperparameters {
    fn default() -> Self {
        Self {
            c_puct_early: 4.2,
            c_puct_mid: 3.8,
            c_puct_late: 3.0,
            variance_mult_high: 1.3,
            variance_mult_low: 0.85,
            prune_early: 0.05,
            prune_mid1: 0.10,
            prune_mid2: 0.15,
            prune_late: 0.20,
            rollout_strong: 3,
            rollout_medium: 5,
            rollout_default: 7,
            rollout_weak: 9,
            weight_cnn: 0.6,
            weight_rollout: 0.2,
            weight_heuristic: 0.1,
            weight_contextual: 0.1,
        }
    }
}
```

### Step 2: Modify mcts_core to Accept Hyperparameters

```rust
fn mcts_core(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    evaluator: MctsEvaluator<'_>,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    hyperparams: &MCTSHyperparameters,  // NEW
) -> MCTSResult {
    // Use hyperparams.c_puct_early instead of hardcoded 4.2
    // ...
}
```

### Step 3: Create Tuning Binary

```rust
// src/bin/tune_hyperparameters.rs
// - Generate parameter combinations
// - Run N games per config
// - Log results to CSV
// - Report best configuration
```

### Step 4: Analysis Script

```python
# scripts/analyze_hyperparameter_results.py
# - Read CSV results
# - Statistical analysis (mean, std, confidence intervals)
# - Visualize parameter impact
# - Recommend best config
```

---

## Success Criteria

### Minimum Viable (Accept)
- **+1.0 pt improvement** over baseline (143.98 → 144.98)
- Statistical significance (p < 0.05, t-test)
- No variance increase (std dev ≤ 27)

### Target Goal
- **+1.5-2.0 pts improvement** (143.98 → 145.5-146.0)
- Lower variance (std dev < 25)

### Stretch Goal
- **+2.5 pts improvement** (143.98 → 146.5)
- Identify parameter interactions
- Document optimal configuration

---

## Risk Analysis

### Low Risk Factors ✅
- No architectural changes
- Easy to revert (just use default params)
- Incremental testing
- Can stop early if no improvement

### Potential Issues ⚠️
1. **Overfitting to seed 2025**: Mitigate with multiple seeds
2. **Compute time**: Use parallel runs on multiple cores
3. **Local optima**: Use random search for exploration
4. **Parameter interactions**: May need 2D/3D search grids

---

## Timeline

| Phase | Duration | Compute | Output |
|-------|----------|---------|--------|
| Implementation | 2 days | - | Tuning binary ready |
| Phase 1 (Weights) | 1 day | 8h | Best weights |
| Phase 2 (c_puct) | 1 day | 12h | Best c_puct |
| Phase 3 (Rollouts) | 1 day | 10h | Best rollouts |
| Analysis & Docs | 1 day | - | Final report |
| **TOTAL** | **6 days** | **30h** | Optimized config |

---

## Next Actions

1. ✅ Document current hyperparameters (DONE)
2. 🔄 Create MCTSHyperparameters struct
3. 🔄 Modify mcts_core signature
4. 🔄 Create tune_hyperparameters binary
5. ⏳ Run Phase 1 (evaluation weights)
6. ⏳ Run Phase 2 (c_puct)
7. ⏳ Run Phase 3 (rollouts)
8. ⏳ Document results

---

## References

- Browne et al. (2012) - "A Survey of Monte Carlo Tree Search Methods"
- Silver et al. (2016) - "Mastering the game of Go with deep neural networks and tree search" (AlphaGo hyperparameters)
- Our baseline: CNN Curriculum + Pattern Rollouts V2

**Date**: 2025-11-07
**Author**: Automation Assistant
**Status**: PLAN READY
