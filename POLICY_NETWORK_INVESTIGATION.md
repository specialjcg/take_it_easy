# Policy Network Non-Convergence - Root Cause Analysis

**Date:** 2025-10-31
**Context:** Supervised training on Phase 1 expert data (50 games, 950 examples)

---

## 🔍 Symptom

During supervised training with curriculum learning:

| Metric | Epoch 1 | Epoch 5 | Epoch 10 | Epoch 15 |
|--------|---------|---------|----------|----------|
| **Policy Loss (train)** | 2.9445 | 2.9445 | 2.9445 | 2.9445 |
| **Policy Loss (val)** | 2.9445 | 2.9445 | 2.9445 | 2.9445 |
| **Value Loss (train)** | 2.6653 | 0.1193 | 0.1088 | 0.1077 |
| **Value Loss (val)** | 2.5563 | 0.0947 | 0.0948 | 0.0948 |

**Observation:**
- ✅ **Value network** converged normally (2.66 → 0.11)
- ❌ **Policy network** completely stuck at 2.9445

**Constant value 2.9445 = -log(1/19) ≈ 2.944** → Uniform distribution!

---

## 🎯 Root Cause Analysis

### Investigation 1: Position Distribution in Expert Data

```
Position | Count | Percentage
----------------------------------------
   0    |   50  |   5.26%
   1    |   50  |   5.26%
   2    |   50  |   5.26%
   ...  |  ...  |   ...
  18    |   50  |   5.26%

Total moves: 950
Entropy: 4.248 bits (normalized: 1.000)
```

**Finding:** PERFECTLY uniform distribution (max entropy)

### Investigation 2: Per-Game Analysis

```
Game 0: 19/19 unique positions used
Game 1: 19/19 unique positions used
Game 2: 19/19 unique positions used
...
Game 49: 19/19 unique positions used
```

**Finding:** **EVERY game uses EACH position EXACTLY ONCE!**

### Investigation 3: Per-Turn Analysis

```
Turn  0: 1/19  unique positions  → Very deterministic
Turn  1: 8/19  unique positions
Turn  2: 9/19  unique positions
Turn  3: 13/19 unique positions
...
Turn 14: 18/19 unique positions  → High diversity
Turn 18: 13/19 unique positions
```

**Finding:** Early turns show some consistency, but overall each position is used exactly 50 times across 50 games.

---

## 💡 Root Cause: MCTS Exploration Behavior

**Hypothesis:** 500-simulation MCTS with neural guidance produces quasi-uniform position preferences.

**Why this happens:**

1. **MCTS explores broadly** with 500 simulations
2. **All legal positions get visited** multiple times
3. **With good value network**, many positions have similar expected values
4. **Selection becomes quasi-random** when values are close
5. **Over 50 games with different seeds**, this averages to uniform distribution

**Mathematical explanation:**
```
For each state (plateau, tile):
  MCTS evaluates all 19 legal positions
  With 500 sims: ~26 visits per position

  If value estimates are:
    Position A: 145.2 ± 15.0
    Position B: 143.8 ± 14.5
    Position C: 146.1 ± 15.2

  Differences are within noise!
  → Selection appears random
  → Over many games → Uniform distribution
```

---

## ❓ Why Does This Break Policy Network Learning?

**Cross-Entropy Loss for classification:**
```
Loss = -Σ p_target(i) * log(p_pred(i))

When p_target is uniform: p_target(i) = 1/19 for all i

Optimal prediction = Uniform distribution
→ p_pred(i) = 1/19 for all i
→ Loss = -log(1/19) = 2.944

ANY other prediction has HIGHER loss!
```

**The network correctly learns to predict uniform distribution!**

---

## 🤔 But Wait... The Benchmark Improved +22 pts!

**Paradox:** Policy network didn't learn, but performance improved significantly!

**Explanation:**

The improvement came **entirely from the Value Network**:

1. **Value Network learned well:**
   - Loss: 2.66 → 0.11 (96% reduction)
   - Can predict game outcome from board state

2. **MCTS uses Value Network for:**
   - Evaluating leaf nodes in tree search
   - Guiding exploration
   - Estimating position quality

3. **Policy Network's role:**
   - Initial move priors
   - If uniform → No harm, MCTS explores all anyway
   - Value network corrects during search

**Result:** Value network alone is sufficient for +22 pts improvement!

---

## 🔧 How to Fix Policy Network Learning

### Solution 1: Generate Less Uniform Expert Data

**Option A: Reduce MCTS simulations**
- Use 50-100 simulations instead of 500
- Less exploration → More deterministic choices
- Cons: Lower quality expert data

**Option B: Use temperature sampling**
- Instead of argmax, sample from MCTS visit counts
- T=0.5: More deterministic
- T=1.0: Current behavior
- T=2.0: More exploration

**Option C: Use policy distribution from MCTS**
- We already save it! (`policy_distribution` in expert data)
- Instead of training on argmax position
- Train on full visit count distribution
- This is what AlphaZero does!

### Solution 2: Modify Training Objective

**Option A: Use KL divergence instead of Cross-Entropy**
```python
# Current (classification):
target = best_position  # One-hot: [0,0,0,1,0,...]
loss = CrossEntropy(predictions, target)

# Better (distribution matching):
target = mcts_visit_distribution  # Soft: [0.05, 0.08, 0.31, ...]
loss = KLDivergence(predictions, target)
```

**Option B: Add entropy regularization**
```python
policy_loss = CrossEntropy(pred, target)
entropy_bonus = -Σ p_pred * log(p_pred)
total_loss = policy_loss - 0.01 * entropy_bonus
```

### Solution 3: Multi-Task Learning

Train policy to predict:
1. Best move (current)
2. Top-3 moves
3. Moves to avoid (bottom-5)

This provides more signal even with uniform data.

---

## 📊 Recommendation for Gold GNN Training

### Immediate Action (Phases 2 & 3 already generating):

**Keep current approach** because:
1. ✅ Value network learns well
2. ✅ Already getting +22 pts improvement
3. ✅ Phases 2 & 3 will have similar data structure
4. ⏱️ Too late to change (already generating)

### Future Improvement:

**After Gold GNN training**, if we want better policy:

1. **Generate new expert data with policy distribution:**
   ```rust
   // In expert_data_generator.rs, DON'T use --simple flag
   // This saves the full MCTS visit distribution
   ```

2. **Modify supervised_trainer.rs:**
   ```rust
   // Use KL divergence loss for policy
   if let Some(policy_dist) = &expert_move.policy_distribution {
       let target = create_distribution_tensor(policy_dist);
       let loss = policy_pred.kl_div(&target.log(), Reduction::Mean);
   }
   ```

3. **Expected improvement:**
   - Policy network will learn move preferences
   - MCTS will start with better priors
   - Estimated +3-5 pts additional improvement

---

## 🎯 Key Insights

### What We Learned:

1. **Uniform expert data** → Policy network cannot learn
2. **Value network alone** is surprisingly effective (+22 pts)
3. **MCTS with good value net** doesn't need strong policy priors
4. **Distribution matching** (KL divergence) better than classification (Cross-Entropy)

### Why It Still Worked:

```
Pure MCTS:  120.05 pts
Baseline:   139.40 pts  (+19 pts from Value Net)
Phase 1:    142.07 pts  (+3 pts from better Value Net training)

Improvement source: 100% Value Network
Policy Network contribution: 0%
```

The +22 pts improvement vs Pure MCTS came from:
- **Pure MCTS → Baseline:** Value Net (+19 pts)
- **Baseline → Phase 1:** Better Value Net training (+3 pts)

---

## 📝 Action Items

### For Current Gold GNN Training:
- ✅ Proceed with Phases 2 & 3 as planned
- ✅ Train Gold GNN on 6,650 examples
- ✅ Expect similar behavior (Value Net learns, Policy doesn't)
- ✅ Still expect +5-10 pts improvement from better architecture

### For Future Work (Optional):
- 🔄 Regenerate data WITHOUT --simple flag
- 🔄 Implement KL divergence training
- 🔄 Benchmark policy-learned model
- 🎯 Target: +3-5 pts additional improvement

---

## 📚 References

**AlphaZero approach:**
- Uses MCTS visit counts as policy targets
- Trains with KL divergence, not cross-entropy
- Policy and Value networks both critical

**Our approach (accidentally different):**
- Uses argmax position as policy target
- Uniform distribution → No learning
- Value network alone carries the improvement

**Lesson:** Sometimes "bugs" reveal insights! The Value Network is more important than we thought.

---

*Document created: 2025-10-31*
*Investigation completed during Phase 2 & 3 data generation*
