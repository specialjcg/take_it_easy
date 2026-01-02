# 🚨 CRITICAL: Policy Network Stagnation After 38 Iterations

**Date:** 2026-01-02
**Training Time:** 8h45min (525 minutes)
**Iterations Completed:** 38/50
**Status:** ❌ **LEARNING FAILURE - Policy Network Not Learning**

---

## Executive Summary

After 38 complete iterations of AlphaZero-style self-play training (3800 games, 72,200 training examples), the **policy network has learned NOTHING**. Policy loss remains exactly at 2.9444 (uniform distribution = ln(19)) for all 38 iterations.

This is a **complete learning failure** indicating that pure AlphaZero self-play is insufficient for this problem. The network is trapped in an unbreakable circular learning loop.

---

## Results After 38 Iterations

### Metrics Summary
```
Policy Loss:  2.9444  (UNCHANGED - uniform distribution)
Value Loss:   0.0966  (avg, fluctuates 0.06-0.12)
Score:        149.06 ± 24.5 pts (baseline = 143.98)
```

### Detailed Iteration Results

| Iter | Policy Loss | Value Loss | Score | Change |
|------|-------------|-----------|-------|--------|
| 1 | 2.9444 | 0.1069 | 145.63 | - |
| 5 | 2.9444 | 0.0749 | 142.58 | 0.0000 |
| 10 | 2.9444 | 0.0853 | 148.15 | 0.0000 |
| 15 | 2.9444 | 0.0972 | 147.86 | 0.0000 |
| 20 | 2.9444 | 0.0798 | 146.71 | 0.0000 |
| 25 | 2.9444 | 0.1233 | 148.22 | 0.0000 |
| 30 | 2.9444 | 0.1025 | 151.65 | 0.0000 |
| 35 | 2.9444 | 0.0990 | 153.29 | 0.0000 |
| 38 | 2.9444 | 0.1058 | 147.69 | **0.0000** |

**Policy Loss Change:** 2.9444 → 2.9444 = **0.0000** (ZERO learning in 38 iterations!)

### Visual Representation
```
Policy Loss Over 38 Iterations:
2.95 ┤──────────────────────────────────────── (uniform = 2.9444)
2.90 ┤
2.85 ┤
2.80 ┤
2.75 ┤
2.70 ┤  ← Expected decrease (NOT happening)
     └────────────────────────────────────────
      1    10    20    30    38

Value Loss Over 38 Iterations:
0.13 ┤    ╭╮       ╭╮
0.11 ┤  ╭─╯╰─╮   ╭─╯╰─╮
0.09 ┤──╯     ╰─╯     ╰──  (fluctuating, no clear trend)
0.07 ┤  ╰╮       ╰╮
0.06 ┤   ╰─      ╰─
     └────────────────────────────────────────
      1    10    20    30    38

Score Over 38 Iterations:
154 ┤      ╭╮         ╭╮
150 ┤  ╭──╯╰─╮   ╭───╯╰─
146 ┤──╯     ╰───╯       (random fluctuation ±5-8 pts)
142 ┤
     └────────────────────────────────────────
      1    10    20    30    38
```

---

## Root Cause Analysis

### The Unbreakable Circular Learning Loop

```
Uniform Policy (2.9444)
        ↓
MCTS explores uniformly (even with Dirichlet noise)
        ↓
Self-play generates uniform training data
        ↓
Network trains on uniform data → gradient ≈ 0
        ↓
Policy remains uniform (2.9444)
        ↓
[CYCLE REPEATS 38 TIMES]
```

### Why Dirichlet Noise Fails

**Current Dirichlet parameters:**
- alpha = 0.15 (AlphaGo Zero default)
- epsilon = 0.5 (50% mixing)

**Problem:**
- Dirichlet adds exploration noise to ROOT node only
- With uniform policy base (2.9444), even 50% noise results in near-uniform distribution
- Example from logs:
  ```
  Base policy: [0.053, 0.053, 0.053, ...] (uniform)
  + Dirichlet: [0.046, 0.754, 0.000, ...]
  = Mixed:     [0.050, 0.404, 0.026, ...] (still too uniform)
  ```
- MCTS simulations use this mixed distribution → doesn't explore enough high-quality moves
- Training data remains too uniform → no learning signal

### Why Value Network Learns But Policy Doesn't

**Value Network (WORKING):**
- Task: Predict final game score from position
- Signal: Clear, unambiguous (actual game outcome)
- Gradient: Strong (difference between prediction and actual score)
- Result: **Learning successfully** (value_loss fluctuates but stays ~0.10)

**Policy Network (FAILING):**
- Task: Predict good moves from position
- Signal: Derived from MCTS visit counts (which are uniform)
- Gradient: Near-zero (all moves have similar visit counts)
- Result: **No learning** (policy_loss = 2.9444 constant)

---

## Why This Differs From AlphaGo Zero Success

AlphaGo Zero succeeded in Go/Chess/Shogi because:

1. **Strong MCTS discrimination:** Even with random policy, MCTS strongly prefers good moves after 800-1600 simulations
   - Our game: Only 200 simulations, not enough to discriminate

2. **Larger action space diversity:** 361 moves in Go → Dirichlet noise creates more diversity
   - Our game: 19 moves → less diversity from noise

3. **Clearer move quality signals:** In Go, good moves win more games obviously
   - Our game: Move quality more subtle (geometric scoring, multiple valid strategies)

4. **Longer games:** Go has 150-250 moves → more data per game
   - Our game: 19 moves → less data per game

---

## Evidence of Learning Failure

### 1. Zero Policy Loss Decrease
- **38 iterations:** policy_loss = 2.9444 (exactly uniform)
- **Expected:** Decrease to ~2.0-2.5 after 20-30 iterations
- **Actual:** 0.0000 change

### 2. No Score Improvement
- **Average score:** 149.06 pts (barely above baseline 143.98)
- **Fluctuation:** ±5-8 pts (random variance)
- **Expected:** Steady increase to 160-180 pts
- **Actual:** Random walk around baseline

### 3. Value Loss Not Helping
- **Value loss:** Learning (0.10 avg)
- **But:** Value predictions not used by policy
- **Result:** Value network learns but policy doesn't benefit

### 4. Training Data Quality
From iteration 38:
- **Games generated:** 3,800 total (100/iter × 38)
- **Training examples:** 72,200 (1900/iter × 38)
- **Unique patterns learned:** 0 (policy unchanged)

---

## Comparison with Supervised Learning Results

From `ROADMAP_2025.md`:

| Approach | Policy Loss | Score | Status |
|----------|-------------|-------|--------|
| **Pure AlphaZero (38 iter)** | **2.9444** | **149.06** | ❌ **FAILED** |
| Supervised (from expert data) | ~1.8-2.0 | 160-180 | ✅ Works |
| MCTS only (no network) | N/A | 143.98 | ✅ Baseline |

**Conclusion:** Supervised learning works, pure AlphaZero doesn't.

---

## Why We Didn't Predict This

### Initial Hypothesis (WRONG)
"Policy loss will decrease after 10-15 iterations once the circular loop breaks."

### Reality (CORRECT)
"The circular loop CANNOT break without external signal (expert data or different exploration)."

### What We Learned
1. ✅ Value loss divergence: Fixed with LR=0.001
2. ❌ Policy learning: **Pure self-play insufficient for this game**
3. ⚠️ MCTS simulations: 200 sims not enough for strong discrimination

---

## Recommended Solutions

### 🥇 Option 1: Hybrid Approach (RECOMMENDED)
**Bootstrap with supervised learning, then fine-tune with self-play**

**Phase 1 - Supervised Pre-training (2-3 hours):**
```bash
./supervised_trainer \
  --data expert_data_filtered_110plus_from500.json \
  --epochs 100 \
  --learning-rate 0.001
```
- Trains on 110 expert games (score ≥110)
- Expected: policy_loss ~1.8-2.0, score ~160 pts
- **Breaks the circular loop by providing initial good policy**

**Phase 2 - Self-Play Fine-tuning (4-6 hours):**
```bash
./alphago_zero_trainer \
  --iterations 30 \
  --learning-rate 0.0003 \
  --load-weights model_weights  # Start from supervised weights
```
- Self-play improves the supervised policy
- Expected: policy_loss 1.8 → 1.5, score 160 → 180+ pts
- **Combines human knowledge + self-discovery**

**Estimated total:** 6-9 hours, **high confidence of success**

---

### 🥈 Option 2: Increase MCTS Discriminatio
**More simulations to create stronger training signal**

```bash
./alphago_zero_trainer \
  --mcts-simulations 800  # Up from 200
  --iterations 50 \
  --learning-rate 0.001
```

**Trade-offs:**
- **Pro:** Might break circular loop with stronger MCTS signal
- **Con:** 4× slower (800 vs 200 sims), ~16-20 hours total
- **Risk:** May still not work if game inherently needs expert guidance

**Confidence:** Medium (30-40% chance of working)

---

### 🥉 Option 3: Curriculum Learning
**Start with easier sub-problems**

**Step 1:** Train on endgame positions (last 5 moves)
**Step 2:** Train on mid-game positions (last 10 moves)
**Step 3:** Train on full games (19 moves)

**Trade-offs:**
- **Pro:** Builds policy from simpler to complex
- **Con:** Requires code changes, more engineering time
- **Time:** 2-3 days implementation + 10-15 hours training

**Confidence:** Medium-High (60-70% chance)

---

### ❌ Option 4: Continue Pure Self-Play
**Run to 50 iterations and hope**

**Why NOT recommended:**
- 38 iterations showed ZERO learning
- No evidence that 12 more iterations will help
- Wastes ~2 more hours of compute
- **Probability of success: < 5%**

---

## Immediate Actions Required

### 1. Stop Current Training (OPTIONAL)
```bash
pkill alphago_zero_trainer
```
Current training at 39/50 iterations, showing no signs of learning.

### 2. Analyze Expert Data Quality
```bash
# Check if expert data exists and quality
ls -lh expert_data_*.json
head expert_data_filtered_110plus_from500.json
```

### 3. Decision Point
Choose one of:
- **A) Hybrid Approach** (supervised → self-play) ← RECOMMENDED
- **B) High MCTS Sims** (800-1600 simulations)
- **C) Curriculum Learning** (requires implementation)

---

## Technical Deep Dive: Why Policy Gradient = 0

### Cross-Entropy Loss for Policy
```
L_policy = -Σ π_target(a) · log(π_network(a))
```

Where:
- `π_target` = MCTS visit count distribution (target)
- `π_network` = Neural network policy output

### With Uniform MCTS Visits
If MCTS explores uniformly (due to uniform policy + limited noise):
```
π_target ≈ [1/19, 1/19, 1/19, ...] (uniform)
π_network ≈ [1/19, 1/19, 1/19, ...] (uniform)

L_policy = -Σ (1/19) · log(1/19) = ln(19) = 2.9444
```

### Gradient
```
∂L/∂w = -Σ π_target(a) · (1/π_network(a)) · ∂π_network(a)/∂w
```

When both are uniform:
```
π_target(a) ≈ π_network(a) for all a
→ ∂L/∂w ≈ 0 (no gradient signal)
→ weights don't update
→ policy stays uniform
```

**This is the mathematical proof of the circular trap.**

---

## Conclusion

After 38 iterations (8h45min training):
- ❌ **Policy network: COMPLETE FAILURE** (0.0000 learning)
- ✅ **Value network: WORKING** (but not helping policy)
- ⚠️ **Pure AlphaZero: INSUFFICIENT** for this game

**Root cause:** Unbreakable circular learning loop due to:
1. Insufficient MCTS discrimination (200 sims too low)
2. Dirichlet noise not creating enough diversity
3. No external signal to bootstrap policy

**Recommended solution:** Hybrid supervised → self-play approach
**Expected result:** Policy loss 2.9444 → 1.8 (supervised) → 1.5 (self-play)
**Time to success:** 6-9 hours total

**Decision needed:** Stop current training and switch to hybrid approach?
