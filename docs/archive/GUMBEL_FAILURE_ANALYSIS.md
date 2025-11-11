# Gumbel MCTS - Post-Mortem Analysis

## Executive Summary

**Verdict**: ❌ **FAILED** - Gumbel MCTS does not improve upon baseline
**Final Score**: 65.70 ± 46.34 pts (vs Baseline: 148.80 ± 22.21 pts)
**Regression**: -83.10 pts (-55.8%)
**Recommendation**: Abandon Gumbel MCTS approach

---

## Test Results

### Baseline MCTS (Standard UCB + Pattern Rollouts)
- **Score**: 148.80 ± 22.21 pts
- **Min/Max**: 112 / 195 pts
- **Status**: ✅ Current best approach

### Gumbel MCTS (Gumbel-Top-k Selection)
| Metric | Value |
|--------|-------|
| Average Score | 65.70 pts |
| Std Deviation | 46.34 pts |
| Min/Max | 0 / 150 pts |
| Regression | -83.10 pts (-55.8%) |

**Test Details**: 10 games, 150 simulations, seed 2025
**Scores**: [150, 76, 96, 32, 123, 39, 36, 89, 16, 0]

---

## Implementation Details

### Location
- **Module**: `src/mcts/gumbel_selection.rs` (295 lines)
- **Integration**: `src/mcts/algorithm.rs:597-931` (`mcts_core_gumbel`)
- **Test Binary**: `src/bin/test_gumbel.rs`

### Key Components
```rust
// Gumbel noise sampling
fn sample(&self, rng: &mut R) -> f64 {
    let u: f64 = rng.random(); // Uniform(0,1)
    self.mu - self.beta * (-u.ln()).ln()
}

// Selection formula
gumbel_score = q_value + (gumbel_noise / temperature)

// Adaptive temperature
temperature = match current_turn {
    0..6 => 1.5,    // Early: high exploration
    7..12 => 1.0,   // Mid: balanced
    _ => 0.5,       // Late: exploitation
}
```

---

## Root Cause Analysis

### Problem #1: Poor Q-value Bootstrapping
```rust
// ISSUE: All Q-values initialized to 0.0
let mut q_values: HashMap<usize, f64> = HashMap::new();
for &position in &legal_moves {
    q_values.insert(position, 0.0);  // ❌ No initial guidance
}
```

**Impact**: First simulations are purely random since all Q-values are identical

**Baseline Comparison**: Baseline uses neural network value estimates for initial guidance:
```rust
// Baseline has differentiated values from start
value_estimates.insert(position, pred_value); // CNN prediction
```

---

### Problem #2: Gumbel Noise Overwhelms Signal

**Analysis**:
- Q-values after normalization: typically in [-1.0, 1.0] range
- Gumbel noise: has mean ≈ 0.577, std dev ≈ 1.28
- With temperature = 1.5 early game: noise impact is **reduced** but still significant
- Selection becomes: `argmax[Q + Gumbel/1.5]` where Gumbel dominates

**Result**: Exploration is too random, not guided by actual game quality

---

### Problem #3: No UCB Exploration Bonus

**Baseline MCTS uses**:
```rust
let exploration_param = c_puct * (total_visits.ln()) / (1.0 + visits);
let ucb_score = combined_eval + exploration_param * prior_prob.sqrt();
```

**Gumbel MCTS does NOT**:
- No visit count balancing
- No prior probability from neural network
- Pure Q-value + Gumbel noise

**Impact**: Doesn't prioritize under-explored branches effectively

---

### Problem #4: Incremental Q-value Updates vs Combined Evaluation

**Gumbel uses simple incremental average**:
```rust
*q_value += (normalized_score - *q_value) / (*visits as f64);
```

**Baseline uses weighted combination**:
```rust
let combined_eval = 0.6 * normalized_value    // CNN prediction
                  + 0.2 * normalized_rollout   // Simulation result
                  + 0.1 * normalized_heuristic // Domain knowledge
                  + 0.1 * contextual;          // Entropy boost
```

**Result**: Gumbel ignores neural network guidance and domain heuristics

---

### Problem #5: Final Selection Strategy Issues

**Gumbel selection**:
```rust
if current_turn > 15 {
    // Late game: greedy Q-value (can pick low-visit moves)
    max_by Q-value
} else {
    // Early/mid: visit count (robust but may miss best moves)
    max_by visit_count
}
```

**Baseline selection**:
```rust
// Always uses UCB scores combining quality + exploration
legal_moves.max_by(|&a, &b| {
    ucb_scores.get(&a).partial_cmp(ucb_scores.get(&b))
})
```

---

## Performance Comparison

| Approach | Score | Delta | Time/Move | Status |
|----------|-------|-------|-----------|--------|
| **Baseline (CNN + UCB)** | **148.80** | - | ~50ms | ✅ KEPT |
| Gumbel MCTS | 65.70 | -83.10 | ~50ms | ❌ Failed |
| Expectimax MCTS | 7.80 | -141.00 | ~28ms | ❌ Failed |
| CVaR MCTS | 142.45 | -1.53 | ~50ms | ❌ Rejected |
| Progressive Widening | 143.49 | -0.49 | ~50ms | ⚠️ No gain |

---

## Key Learnings

### 1. Gumbel Selection Requires Proper Value Estimates
- Gumbel MCTS was designed for MuZero with **trained** value networks
- Our implementation starts with Q=0, causing catastrophic early decisions
- Lesson: Don't use Gumbel without proper value initialization

### 2. Domain Heuristics Matter More Than Selection Strategy
- Pattern Rollouts: 148 pts
- Gumbel without heuristics: 65 pts
- **Evaluation quality > Selection algorithm**

### 3. Take It Easy ≠ Adversarial Games
- Gumbel proven effective for: Go, Chess, Shogi (via MuZero)
- Take It Easy is: Single-player optimization with tile draws
- Baseline UCB is already optimal for this domain

### 4. Adaptive Temperature Doesn't Fix Fundamental Issues
- Temperature schedule (1.5 → 1.0 → 0.5) was theoretically sound
- But can't compensate for lack of value guidance
- Lesson: Tuning parameters won't fix architectural problems

---

## Theoretical Context

### Why Gumbel MCTS Works in MuZero

**MuZero Context**:
1. ✅ Trained world model provides accurate state values
2. ✅ Millions of self-play games for value network training
3. ✅ Adversarial games benefit from exploration diversity
4. ✅ Gumbel noise breaks symmetry in identical-looking positions

**Take It Easy Context**:
1. ❌ Untrained/limited CNN provides weak value estimates
2. ❌ Curriculum learning still in progress (~100 games)
3. ❌ Single-player optimization needs exploitation > exploration
4. ❌ Each position is unique (no symmetry to break)

---

## What Would Be Needed to Make Gumbel Work

### Option A: Hybrid Gumbel (Not Recommended)
```rust
// Initialize Q-values with CNN predictions
q_values.insert(position, value_net.forward(...));

// Use weighted Gumbel selection
gumbel_score = 0.7 * q_value + 0.3 * (gumbel_noise / temperature);
```

**Estimated Effort**: 1-2 days
**Expected Gain**: +20-30 pts (still worse than baseline 148)
**Risk**: Medium
**Verdict**: Not worth it

### Option B: Full MuZero-style Training (Not Practical)
1. Train value network on 100k+ games
2. Implement world model for future state prediction
3. Use Gumbel for policy improvement
4. Iterate training for weeks

**Estimated Effort**: 2-3 months
**Expected Gain**: Unknown
**Risk**: Very High
**Verdict**: Pursue World Model research instead (see ROADMAP_2025.md)

---

## Conclusion

**Gumbel MCTS is ABANDONED** for the following reasons:

1. ❌ **Performance**: 55.8% worse than baseline
2. ❌ **Fundamental mismatch**: Designed for adversarial games, not single-player optimization
3. ❌ **Requires trained value network**: Our curriculum learning is incomplete
4. ❌ **No clear path to improvement**: Fixing issues would require baseline-level value estimates (defeating the purpose)

**Recommendation**: Focus on alternatives that enhance baseline MCTS:
- ✅ **Hyperparameter Tuning** (Option 1.2 from roadmap) - NEXT
- ✅ **MCTS-Guided Neural Network** (reduces search space, keeps MCTS)
- ✅ **Parallel MCTS** (speedup, no algorithm change)

---

## Code Status

**Modules created**:
- `src/mcts/gumbel_selection.rs` - Keep for reference
- `src/mcts/algorithm.rs:597-931` - Keep function disabled
- `src/bin/test_gumbel.rs` - Keep for testing

**Action**: Comment out Gumbel function in exports to prevent accidental use

```rust
// In src/mcts/algorithm.rs - add #[allow(dead_code)]
#[allow(dead_code)]
pub fn mcts_find_best_position_for_tile_gumbel(...) -> MCTSResult {
    // Implementation kept for research reference
}
```

---

## References

- Danihelka et al. (2022) - "Policy improvement by planning with Gumbel"
- Schrittwieser et al. (2020) - "Mastering Atari, Go, chess and shogi by planning with a learned model" (MuZero)
- Our baseline: CNN Curriculum + Pattern Rollouts V2 (143.98 pts documented)

**Date**: 2025-11-07
**Author**: Claude Code
**Status**: POST-MORTEM COMPLETE
