# Hyperparameter Optimization - Implementation Status

## Date: 2025-11-07

---

## ✅ Completed

### 1. Analysis & Planning
- ✅ Identified 4 categories of hyperparameters (16 total parameters)
- ✅ Designed 4-phase optimization strategy
- ✅ Created `HYPERPARAMETER_TUNING_PLAN.md` (comprehensive plan)
- ✅ Created `GUMBEL_FAILURE_ANALYSIS.md` (why Gumbel failed)

### 2. Infrastructure
- ✅ Created `src/mcts/hyperparameters.rs` (295 lines)
  - MCTSHyperparameters struct with all 16 parameters
  - Default values matching current baseline
  - Helper methods: `get_c_puct()`, `get_rollout_count()`, etc.
  - Validation: `validate_weights()` ensures sum = 1.0
  - Logging: `to_config_string()` for CSV output
  - ✅ Full test coverage (5 unit tests passing)
- ✅ Added to `src/mcts/mod.rs`
- ✅ Compilation verified (no errors)

---

## 🔄 In Progress

### 3. Modify mcts_core Function

**Current signature**:
```rust
fn mcts_core(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    evaluator: MctsEvaluator<'_>,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
) -> MCTSResult
```

**Target signature**:
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
) -> MCTSResult
```

---

## ⏳ TODO

### Step 1: Modify mcts_core Implementation

**Lines to change in `src/mcts/algorithm.rs`**:

#### A. Import hyperparameters (top of file)
```rust
use crate::mcts::hyperparameters::MCTSHyperparameters;
```

#### B. Line 326-332: Replace c_puct calculation
```rust
// BEFORE
let base_c_puct = if current_turn < 5 {
    4.2
} else if current_turn > 15 {
    3.0
} else {
    3.8
};

// AFTER
let base_c_puct = hyperparams.get_c_puct(current_turn);
```

#### C. Line 335-343: Replace variance multiplier
```rust
// BEFORE
let variance_multiplier = if variance > 0.5 {
    1.3
} else if variance > 0.2 {
    1.1
} else if variance > 0.05 {
    1.0
} else {
    0.85
};

// AFTER
let variance_multiplier = hyperparams.get_variance_multiplier(variance);
```

#### D. Line 360-368: Replace pruning ratio
```rust
// BEFORE
let pruning_ratio = if current_turn < 5 {
    0.05
} else if current_turn < 10 {
    0.10
} else if current_turn < 15 {
    0.15
} else {
    0.20
};

// AFTER
let pruning_ratio = hyperparams.get_pruning_ratio(current_turn);
```

#### E. Line 275: Replace Pure evaluator rollout count
```rust
// BEFORE
let rollout_count = 6;

// AFTER
let rollout_count = hyperparams.rollout_default;
```

#### F. Line 420-425: Replace adaptive rollout count
```rust
// BEFORE
let rollout_count = match value_estimate {
    x if x > 0.7 => 3,
    x if x > 0.2 => 5,
    x if x < -0.4 => 9,
    _ => 7,
};

// AFTER
let rollout_count = hyperparams.get_rollout_count(value_estimate);
```

#### G. Line 489-492: Replace evaluation weights
```rust
// BEFORE
let combined_eval = 0.6 * normalized_value
                  + 0.2 * normalized_rollout
                  + 0.1 * normalized_heuristic
                  + 0.1 * contextual;

// AFTER
let combined_eval = hyperparams.weight_cnn * normalized_value
                  + hyperparams.weight_rollout * normalized_rollout
                  + hyperparams.weight_heuristic * normalized_heuristic
                  + hyperparams.weight_contextual * contextual;
```

---

### Step 2: Update Public Functions

#### A. `mcts_find_best_position_for_tile_with_nn` (line 36-58)
```rust
// Add hyperparams parameter with default
pub fn mcts_find_best_position_for_tile_with_nn(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    hyperparams: Option<&MCTSHyperparameters>,  // NEW - optional
) -> MCTSResult {
    let hyperparams = hyperparams.unwrap_or(&MCTSHyperparameters::default());
    mcts_core(
        plateau,
        deck,
        chosen_tile,
        MctsEvaluator::Neural { policy_net, value_net },
        num_simulations,
        current_turn,
        total_turns,
        hyperparams,  // Pass through
    )
}
```

#### B. `mcts_find_best_position_for_tile_pure` (line 62-80)
```rust
// Same pattern: add optional hyperparams parameter
pub fn mcts_find_best_position_for_tile_pure(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    hyperparams: Option<&MCTSHyperparameters>,  // NEW
) -> MCTSResult {
    let hyperparams = hyperparams.unwrap_or(&MCTSHyperparameters::default());
    mcts_core(
        plateau,
        deck,
        chosen_tile,
        MctsEvaluator::Pure,
        num_simulations,
        current_turn,
        total_turns,
        hyperparams,
    )
}
```

#### C. `mcts_find_best_position_for_tile_gumbel` (line 85-107)
```rust
// Also add optional hyperparams (even though Gumbel uses different logic)
pub fn mcts_find_best_position_for_tile_gumbel(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    hyperparams: Option<&MCTSHyperparameters>,  // NEW
) -> MCTSResult {
    // Note: mcts_core_gumbel doesn't use hyperparams yet
    mcts_core_gumbel(
        plateau,
        deck,
        chosen_tile,
        MctsEvaluator::Neural { policy_net, value_net },
        num_simulations,
        current_turn,
        total_turns,
    )
}
```

---

### Step 3: Fix Calling Code

All files that call MCTS functions need to be updated:

**Files to check**:
1. `src/bin/compare_mcts.rs` - Add `None` for hyperparams
2. `src/bin/test_gumbel.rs` - Add `None` for hyperparams
3. `src/bin/test_expectimax.rs.disabled` - Skip (disabled)
4. `src/training/evaluator.rs` - Check if calls MCTS
5. `src/services/game_manager.rs` - Check if calls MCTS

**Pattern**:
```rust
// BEFORE
mcts_find_best_position_for_tile_with_nn(
    &mut plateau,
    &mut deck,
    chosen_tile,
    &policy_net,
    &value_net,
    150,
    turn_idx,
    19,
)

// AFTER
mcts_find_best_position_for_tile_with_nn(
    &mut plateau,
    &mut deck,
    chosen_tile,
    &policy_net,
    &value_net,
    150,
    turn_idx,
    19,
    None,  // Use default hyperparameters
)
```

---

### Step 4: Create Tuning Binary

**File**: `src/bin/tune_hyperparameters.rs`

**Functionality**:
- Accept hyperparameter values via CLI
- Run N games with given config
- Log results to CSV
- Support grid search mode

**CLI args**:
```bash
cargo run --release --bin tune_hyperparameters -- \
  --games 20 \
  --seed 2025 \
  --weight-cnn 0.6 \
  --weight-rollout 0.2 \
  --weight-heuristic 0.1 \
  --weight-contextual 0.1 \
  --log-path hyperparameter_results.csv
```

---

### Step 5: Create Grid Search Script

**File**: `scripts/grid_search.sh`

**Purpose**: Run Phase 1 - Evaluation weights grid search

```bash
#!/bin/bash

# Phase 1: Evaluation Weights Grid Search
for w_cnn in 0.55 0.6 0.65; do
  for w_roll in 0.15 0.2 0.25; do
    for w_heur in 0.05 0.1 0.15; do
      w_ctx=$(echo "1.0 - $w_cnn - $w_roll - $w_heur" | bc -l)

      if (( $(echo "$w_ctx >= 0.05" | bc -l) && $(echo "$w_ctx <= 0.15" | bc -l) )); then
        echo "Testing: CNN=$w_cnn, Roll=$w_roll, Heur=$w_heur, Ctx=$w_ctx"

        cargo run --release --bin tune_hyperparameters -- \
          --games 20 \
          --seed 2025 \
          --weight-cnn $w_cnn \
          --weight-rollout $w_roll \
          --weight-heuristic $w_heur \
          --weight-contextual $w_ctx
      fi
    done
  done
done
```

---

## Estimated Remaining Work

| Task | Complexity | Est. Time |
|------|------------|-----------|
| Modify mcts_core | Medium | 1h |
| Update public functions | Low | 30min |
| Fix calling code | Low | 30min |
| Test compilation | Low | 15min |
| Create tuning binary | Medium | 2-3h |
| Create grid search script | Low | 30min |
| Run Phase 1 (weights) | - | 8h compute |
| Analysis | Low | 1h |
| **TOTAL** | - | **~5-6h** + compute |

---

## Next Session Actions

1. Complete mcts_core modifications (7 locations)
2. Update 3 public function signatures
3. Fix calling code in binaries (add `None` parameter)
4. Test that baseline still works with default hyperparams
5. Create tune_hyperparameters binary
6. Run small test (3-5 games) to validate system
7. Launch Phase 1 grid search overnight

---

## Current State Summary

- ✅ **Infrastructure**: 100% complete
- 🔄 **Integration**: 0% complete (not started)
- ⏳ **Tuning Binary**: 0% complete (not started)
- ⏳ **Testing**: 0% complete (not started)

**Estimated completion**: 1-2 days of work + overnight compute

**Expected Outcome**: Optimized hyperparameters → +1-2 pts improvement

---

**Author**: Automation Assistant
**Date**: 2025-11-07
**Status**: PAUSED - Infrastructure complete, integration pending
