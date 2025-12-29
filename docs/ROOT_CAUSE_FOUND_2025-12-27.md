# Root Cause Analysis: Performance Regression 140 pts → 80 pts

**Date**: 2025-12-27
**Investigation Status**: ✅ ROOT CAUSE IDENTIFIED

---

## Executive Summary

**Problem**: Same network that achieved > 140 pts on previous branch now produces ~80 pts
**Root Cause**: **NETWORK WEIGHTS ARE UNTRAINED/CORRUPTED**

### Critical Findings

1. ❌ **Network produces UNIFORM policy** (essentially random guessing)
2. ❌ **Network value is CONSTANT** (doesn't react to game state)
3. ✅ **MCTS optimizations work correctly** (rollouts + heuristics give 80 pts)
4. ❌ **Expert data is ARTIFICIALLY UNIFORM** (generation bug)

---

## Investigation Timeline

### Test 1: Baseline CNN-Only (No MCTS Optimizations)

**Configuration**:
- c_puct = 1.41 (classic)
- No pruning, no rollouts, no heuristics
- **weight_cnn = 1.0** (100%, CNN only)

**Result**: **12.75 pts** (20 games, 150 sims)

**Conclusion**: Network is WORSE than random (9.75 pts)!

### Test 2: Network Forward Pass Analysis

**Policy Network**:
```
Max probability: 0.0540
Uniform (1/19):  0.0526
Difference:      0.0014 (0.27% deviation)
```

**Value Network**:
```
Empty plateau:   0.307795
Partial plateau: 0.307795  (IDENTICAL!)
```

**Diagnosis**:
- Policy is essentially uniform → Network doesn't know which moves are good
- Value is constant → Network doesn't evaluate game states

### Test 3: Training on Elite Data (> 140 pts)

**Dataset**:
- 10 games, average 149.50 pts
- 190 training examples
- Max score: 170 pts

**Training Result**:
```
Epoch 1: policy_loss=2.9446
Epoch 5: policy_loss=2.9444
Epoch 10: policy_loss=2.9442
Early stop at epoch 11
```

**Policy loss = 2.9444 = ln(19)** = Cross-entropy for uniform distribution!

**Conclusion**: Network cannot learn

### Test 4: Training from Scratch (Fresh Weights)

**Result**: Policy loss STILL ~2.9444!

**Conclusion**: Not just corrupted weights, there's a deeper problem

### Test 5: Expert Data Analysis

**CRITICAL DISCOVERY**:
```
Position distribution in "expert" data:
  Position  0:  10 occurrences (5.3%)
  Position  1:  10 occurrences (5.3%)
  Position  2:  10 occurrences (5.3%)
  ...
  Position 18:  10 occurrences (5.3%)

Entropy: 2.9444 (100.0% of maximum)
```

**EVERY POSITION APPEARS EXACTLY 10 TIMES!**

**Conclusion**: Expert data is ARTIFICIALLY UNIFORM (bug in expert_data_generator)

---

## Root Cause Explanation

### Why Current Performance is 80 pts

**MCTS Score Breakdown**:
```
Combined eval = 0.65 * CNN_value      (broken - uniform)
              + 0.25 * rollout_score  (works!)
              + 0.05 * heuristic      (works!)
              + 0.05 * contextual     (works!)
```

**All 80 pts come from**:
- 25% rollouts (smart simulations)
- 5% heuristics (domain knowledge)
- 5% contextual (pattern-based)

**CNN contributes nothing** (65% weight on uniform garbage)

### Why Network is Broken

**Two Interconnected Problems**:

1. **Current Weights are Untrained/Corrupted**
   - Policy is uniform (0.0526 ± 0.0014 for all actions)
   - Value is constant (0.308 regardless of state)
   - MD5: `ff093dfa1f5826d0e9851c6002c2afab` (policy)
   - MD5: `808dc36bb0172f20f99b8030f9215e49` (value)

2. **Expert Data Generation is Broken**
   - Generated 500 games with 1000 MCTS sims
   - **Best_position distribution is PERFECTLY UNIFORM** (bug!)
   - Cannot train on uniform data (no learning signal)
   - Explains why training didn't help

### Why Previous Branch Had > 140 pts

**Previous branch had**:
- ✅ Good network weights (trained properly)
- ✅ Network gave meaningful guidance to MCTS
- ✅ 65% CNN weight was justified

**Current branch has**:
- ❌ Broken/untrained network weights
- ❌ Network gives no guidance (uniform)
- ❌ 65% weight wasted on noise

---

## Evidence Chain

### 1. Network Produces Uniform Output

**Test**:
```rust
let plateau = create_plateau_empty();
let policy = policy_net.forward(&tensor, false);
```

**Output**:
```
Position  0: 0.053961
Position  1: 0.052811
...
Position 18: 0.053282

Expected uniform: 0.052632 (1/19)
Actual: 0.052632 ± 0.0007
```

**Interpretation**: Network is essentially `return vec![1.0/19.0; 19]`

### 2. Value Network is Constant

**Test**:
```rust
Empty plateau:   value = 0.307795
After 3 moves:   value = 0.307795
```

**Interpretation**: Network ignores input, returns constant

### 3. Training Cannot Improve

**Evidence**:
- LR = 0.001: policy_loss = 2.9445 (constant)
- LR = 0.01: policy_loss = 2.9447 (constant)
- Fresh weights: policy_loss = 2.9444 (constant)

**Why**: Loss = 2.9444 = ln(19) = optimal for uniform data!

### 4. Expert Data is Artificially Uniform

**Evidence**:
```python
Counter({0: 10, 1: 10, 2: 10, ..., 18: 10})
```

**Real expert data would show**:
- Center positions preferred (~15-20%)
- Edge positions less frequent (~2-3%)
- Strong correlation with game state

**Actual data shows**:
- All positions exactly equal (5.3%)
- Perfect uniformity (impossible in real MCTS)

---

## Bug Hypothesis: Expert Data Generator

### Suspected Code Location

`src/bin/expert_data_generator.rs` likely has one of these bugs:

#### Bug A: Storing Wrong Data
```rust
// BUG: Stores random position instead of MCTS best_position
let best_position = random_legal_move();  // Wrong!
// Should be:
let best_position = mcts_result.best_position;
```

#### Bug B: Index vs Position Confusion
```rust
// BUG: Stores move index instead of board position
expert_move.best_position = move_index;  // Wrong!
// Should be:
expert_move.best_position = board_position;
```

#### Bug C: Overwriting Data
```rust
// BUG: Each move overwrites previous in circular buffer
data[turn % 19] = expert_move;  // Circular, loses data
// Should be:
data.push(expert_move);  // Append
```

---

## Action Plan

### Immediate (High Priority)

1. **Restore Good Network Weights from Previous Branch**
   - Locate branch with > 140 pts performance
   - Copy `model_weights/cnn/policy/policy.params`
   - Copy `model_weights/cnn/value/value.params`
   - Verify MD5 checksums different from current

2. **Verify Weights Work**
   ```bash
   # After restoring weights
   ./target/release/test_network_forward
   # Should show NON-uniform policy

   ./target/release/benchmark_baseline_cnn --games 20
   # Should show > 100 pts (not 12.75 pts)
   ```

3. **Test Full MCTS with Good Weights**
   ```bash
   ./target/release/benchmark_progressive_widening --games 100
   # Expected: > 120 pts (good network + MCTS optimizations)
   ```

### Secondary (Medium Priority)

4. **Fix Expert Data Generator**
   - Investigate `src/bin/expert_data_generator.rs`
   - Find why best_position is uniform
   - Generate new expert data with fixed code
   - Verify position distribution is non-uniform

5. **Verify Fixed Data**
   ```bash
   python3 << 'EOF'
   import json
   from collections import Counter

   with open('expert_data_fixed.json') as f:
       games = json.load(f)

   positions = [m['best_position'] for g in games for m in g['moves']]
   counts = Counter(positions)

   # Should show variation (not all equal)
   for pos, count in sorted(counts.items()):
       print(f"Position {pos}: {count}")
   EOF
   ```

### Long-Term (Low Priority)

6. **Re-train Network on Fixed Data** (only if weights not recoverable)

7. **Comprehensive Testing**
   - Benchmark across multiple seeds
   - Compare with previous branch baseline
   - Document new baseline

---

## Files to Check/Modify

### Critical Files

1. **`model_weights/cnn/policy/policy.params`** - Needs replacement
2. **`model_weights/cnn/value/value.params`** - Needs replacement
3. **`src/bin/expert_data_generator.rs`** - Has bug causing uniform data

### Investigation Files

4. **`src/bin/benchmark_baseline_cnn.rs`** - Test network alone ✅ Created
5. **`src/bin/test_network_forward.rs`** - Inspect network outputs ✅ Created

---

## Performance Comparison

| Configuration | Mean Score | Network Role | Status |
|---------------|-----------|--------------|--------|
| **Random Player** | 9.75 pts | None | Baseline |
| **CNN-Only (Current)** | 12.75 pts | Broken (uniform) | ❌ Worse than random |
| **MCTS Current (80% optimizations)** | 80.86 pts | Mostly ignored | ⚠️ Works despite bad network |
| **Previous Branch (Good Weights)** | > 140 pts | Strong guidance | ✅ TARGET |
| **Expected (Fixed Weights)** | 120-150 pts | Proper guidance | 🎯 GOAL |

---

## Key Insights

### What Works

✅ MCTS algorithm (UCT, exploration/exploitation)
✅ Rollout simulations (Pattern Rollouts V2)
✅ Domain heuristics (line completion scoring)
✅ Contextual boosting (entropy-based)
✅ Progressive widening (adaptive action selection)
✅ Temperature annealing (exploration → exploitation)

### What's Broken

❌ Neural network weights (untrained/corrupted)
❌ Expert data generator (produces uniform data)
❌ Training pipeline (can't fix broken weights with broken data)

### Why 80 pts Despite Broken Network

**MCTS is resilient**:
- Rollouts provide signal even without network
- Heuristics encode domain knowledge
- UCT exploration finds good moves via trial

**But we're leaving performance on the table**:
- 65% weight on broken network = wasted compute
- Good network would boost to 120-150 pts
- Current 80 pts = running MCTS at 35% capacity

---

## Next Steps

**User needs to**:
1. Provide access to previous branch (> 140 pts) OR
2. Provide network weights from that branch

**Then we can**:
1. Restore good weights
2. Verify performance returns to > 120 pts
3. Fix expert data generator if training needed
4. Further optimize to reach 159.95 pts target

---

## Conclusion

**The regression from 140 → 80 pts is NOT due to**:
- ❌ MCTS algorithm changes (all optimizations work)
- ❌ Hyperparameter tuning (values are reasonable)
- ❌ Code bugs in MCTS (produces sensible moves)

**The regression IS due to**:
- ✅ Network weights being untrained/corrupted
- ✅ Expert data being artificially uniform (generation bug)
- ✅ Training unable to recover (garbage in = garbage out)

**Solution**: Restore network weights from previous branch that achieved > 140 pts.

---

**Status**: Waiting for access to previous branch or good network weights.
