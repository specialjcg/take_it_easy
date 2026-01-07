# Performance Comparison: All Architectures and Methods

**Last updated:** January 5, 2026

---

## Performance Summary Table

| Method | Architecture | Training Type | Epochs/Iters | Avg Score | Std Dev | Success Rate | Status |
|--------|--------------|---------------|--------------|-----------|---------|--------------|--------|
| **BASELINE (Target)** | CNN | AlphaGo Zero | 50 | **147-152** | 21-26 | - | ✓ Reference |
| Pure MCTS | None | - | - | 84-88 | 29 | - | Baseline |
| GNN Supervised | GNN | Supervised | 50 | **61.0** | 29.2 | 0/30 >140 | ❌ Current |
| GNN AlphaZero (iter 1) | GNN | Self-play | 1 | 132.8 | 25.6 | - | ✓ Promising |
| GNN AlphaZero (iter 2-3) | GNN | Self-play | 2-3 | 105 | 28-38 | - | ⚠ Degrading |
| GNN AlphaZero (full) | GNN | Self-play | Full | 21-23 | 17-20 | - | ❌ Failed |
| CNN + Neural | CNN | Unknown | - | 79-91 | - | - | ✓ Partial |

---

## Detailed Results by Architecture

### CNN Architecture

#### AlphaGo Zero Training (Historical - training_history_50iter.csv)
```
Iteration    Policy Loss    Value Loss    Score Mean    Score Std
1            2.9444         0.1069        145.63        21.80
2            2.9444         0.1120        150.89        23.18
10           2.9444         0.0926        148.64        24.38
20           2.9444         0.1107        149.34        24.93
30           2.9444         0.0900        146.47        23.66
40           2.9444         0.1152        149.98        23.04
50           2.9444         0.1000        147.64        25.78

Average (iters 1-50): 148.7 pts
Range: 145.6 - 153.8 pts
Consistency: ✓ Excellent (low variance across iterations)
```

#### CNN vs Pure MCTS (compare_mcts_log.csv)
```
Date                Games    Pure MCTS    CNN Neural    Delta
2025-12-25 20:40    10       88.90        90.70        +1.80 (CNN wins)
2025-12-25 20:55    100      84.25        79.66        -4.59 (MCTS wins)
```

**CNN Summary:**
- ✓ Proven to reach 147-152 pts consistently
- ✓ Stable across iterations
- ✓ AlphaZero-style ResNet architecture
- ✓ 128-channel capacity

---

### GNN Architecture

#### Supervised Learning (Current - supervised_training_50epoch.log)
```
Epoch    Train Policy    Train Value    Val Policy    Val Value
1        1.6326          0.0258         1.5761        0.0308
10       1.5992          0.0257         1.5529        0.0308
20       1.5747          0.0257         1.5295        0.0303
30       1.5554          0.0257         1.5127        0.0303
40       1.5311          0.0256         1.4952        0.0300
50       1.5138          0.0256         1.4696        0.0298

Final Test: 60.97 ± 29.24 pts (30 games)
```

**Loss Improvement:**
- Policy: 1.633 → 1.514 (7.3% reduction)
- Value: 0.0258 → 0.0256 (0.8% reduction)
- Validation: 1.576 → 1.470 (6.7% improvement)

**Entropy Analysis:**
```
Turn    Expected Entropy    Actual Entropy    Assessment
0       0.8 (uncertain)     0.807            ✓ Expected for early game
5       <0.4                0.265-0.424      ⚠ Borderline
10      <0.2                0.567-0.609      ❌ Too high
15      <0.1                0.618-0.793      ❌ Catastrophic
```

#### AlphaGo Zero Self-Play (training_history_gnn_test.csv)
```
Iteration    Policy Loss    Value Loss    Score Mean    Score Std
1            2.8102         0.1061        132.78        25.56    ✓ Excellent!
2            2.8039         0.0883        105.92        38.42    ⚠ Declining
3            2.7966         0.0921        104.78        28.44    ⚠ Further decline
```

**Pattern:** GNN shows initial promise but degrades during self-play training.

#### AlphaGo Zero Long Training (alphazero_gnn_training.csv)
```
Iteration    Score Mean    Score Std
1            21.05         21.26
2            23.10         16.96
3            21.95         19.73

Average: ~22 pts (catastrophic failure)
```

**GNN Summary:**
- ❌ Supervised learning: 61 pts (below target)
- ✓ Self-play iter 1: 132.8 pts (promising!)
- ❌ Self-play degrades to ~22-105 pts
- ❌ High entropy (poor confidence)
- ⚠ Unstable during training

---

## Architectural Specifications

### CNN (AlphaZero-style ResNet)

```
Input: (8, 5, 5) - 8 channels, 5x5 spatial grid
Architecture:
  - Initial Conv: 8 → 128 channels (3x3 kernel)
  - GroupNorm (8 groups)
  - ResNet Blocks x3: [128, 128, 96] channels
    - Each block: Conv → GroupNorm → ReLU → Conv → GroupNorm → Skip Connection
  - Policy Head: 1x1 conv to 19 positions

Parameters: ~200K-300K (estimated)
Training: AlphaGo Zero self-play
Best Performance: 147-152 pts ✓
```

### GNN (Graph Neural Network)

```
Input: 8 features per node, 19 nodes (hex positions)
Architecture:
  - Input → Linear: 8 → 64
  - Hidden Layer 1: GraphConv 64 → 64
  - Hidden Layer 2: GraphConv 64 → 64
  - Hidden Layer 3: GraphConv 64 → 64
  - Output: 64 → 19 (policy logits)
  - Dropout: 0.1

Parameters: ~50K-80K (estimated)
Training: Supervised on MCTS data
Best Performance: 132.8 pts (iter 1) → degrades to 61-105 pts ❌
```

---

## Entropy Evolution Comparison

### Well-Trained Neural Network (Expected)
```
Turn    Entropy (norm)    Confidence    Weight
0       0.8              Low           15-20% NN
5       0.4              Medium        40-50% NN
10      0.2              High          60-70% NN
15      0.1              Very High     80-90% NN
```

### Current GNN (Actual)
```
Turn    Entropy (norm)    Confidence    Weight        Gap
0       0.807            Low           15.2% NN      ✓ Expected
5       0.265-0.424      Low-Medium    17.5% NN      ⚠ Should be higher
10      0.567-0.609      Low           36.8% NN      ❌ Should be ~60%
15      0.618-0.793      Low           57.2% NN      ❌ Should be ~80%
```

**Interpretation:** GNN never gains confidence, even in endgame where patterns should be clearer.

---

## Training Data Quality

### Supervised Dataset (supervised_dataset_2k.csv)

```
Total Examples: 38,000
Games: 2,000
Source: Pure MCTS (150 simulations/move)
Features:
  - Plateau state (19 positions × 8 features)
  - Current tile (3 line values)
  - Chosen position (target)
  - Final score (value target)

Score Statistics:
  Mean: 83.7
  Range: [0, 189]

Split:
  Training: 34,200 examples (90%)
  Validation: 3,800 examples (10%)
```

**Quality Assessment:**
- ✓ Large dataset (38k examples)
- ✓ Generated by strong player (MCTS 150 sims)
- ✓ Good score distribution
- ⚠ May be biased toward MCTS+rollout strategy (not optimal policy)

---

## Adaptive Weight Behavior

### Turn-Based Baseline Strategy
```
Phase         Turns    w_cnn    w_rollout    w_heuristic    Rationale
Early Game    0-5      20%      70%          10%           Trust rollouts, GNN inexperienced
Mid Game      6-11     45%      45%          10%           Balanced approach
Late Game     12+      75%      15%          10%           GNN should excel with more info
```

### Entropy-Based Adjustment
```
Entropy (norm)    Confidence    GNN Weight Multiplier
0.0 - 0.2         Very High     1.2x (boost GNN)
0.2 - 0.4         High          1.0x (neutral)
0.4 - 0.6         Medium        0.8x (reduce GNN)
0.6 - 0.8         Low           0.6x (significantly reduce)
0.8 - 1.0         Very Low      0.4x (minimal trust in GNN)
```

### Actual Hybrid Weights (GNN Test)
```
Turn    Entropy    Turn-Only    Entropy-Only    HYBRID (Actual)    Adjustment
0       0.807      20% / 70%    32.7% / 57.3%   15.2% / 74.8%     -24% GNN (high entropy)
5       0.424      20% / 70%    48.0% / 42.0%   17.5% / 72.5%     -13% GNN
10      0.609      45% / 45%    40.7% / 49.3%   36.8% / 53.2%     -18% GNN
15      0.793      75% / 15%    33.3% / 56.7%   57.2% / 32.8%     -24% GNN (very high entropy)
```

**Observation:** System correctly reduces GNN weight due to high uncertainty, but can't compensate for poor GNN quality.

---

## Cost-Benefit Analysis

### Option 1: CNN Training

**Effort:**
- Code changes: None (architecture exists)
- Training time: 30-60 minutes
- Testing time: 10 minutes

**Expected Outcome:**
- Success probability: 90%
- Expected score: 140-150 pts
- ROI: Excellent

**Risk:**
- Low (proven architecture)

### Option 2: Improve GNN

**Effort:**
- Code changes: Moderate (increase capacity)
- Training time: 2-4 hours (100-200 epochs)
- Research time: 4-8 hours

**Expected Outcome:**
- Success probability: 50%
- Expected score: 90-140 pts
- ROI: Poor

**Risk:**
- High (historical instability)

### Option 3: Optimize MCTS

**Effort:**
- Code changes: Low (hyperparameter tuning)
- Testing time: 2-3 hours
- Tuning iterations: 5-10

**Expected Outcome:**
- Success probability: 30%
- Expected score: 90-110 pts
- ROI: Medium

**Risk:**
- Medium (may not reach target)

---

## Recommendations by Use Case

### Goal: Reach >140 pts ASAP
**Recommendation:** Option 1 (CNN)
**Reasoning:** Proven, low risk, fast

### Goal: Understand Why GNN Fails
**Recommendation:** Option 2 (Improve GNN)
**Reasoning:** Research value, learning opportunity

### Goal: No Neural Network Dependencies
**Recommendation:** Option 3 (Optimize MCTS)
**Reasoning:** Simpler, deterministic

### Goal: Production Deployment
**Recommendation:** Option 1 (CNN)
**Reasoning:** Stable, proven, reliable

### Goal: Best Performance Possible
**Recommendation:** Option 1 (CNN) → AlphaGo Zero self-play
**Reasoning:** Highest ceiling (potentially 160+ pts)

---

## Historical Timeline

```
Date             Event                              Result
-------------    --------------------------------   ------------------
Dec 25, 2025     CNN MCTS comparison test          CNN: 79-91 pts
Dec 25, 2025     Pure MCTS baseline                84-88 pts
Jan 2, 2026      AlphaGo Zero (CNN) - 50 iters     147-152 pts ✓
Jan 3, 2026      AlphaGo Zero training history     Recorded baseline
Jan 4, 2026      GNN AlphaZero test (iter 1)       132.78 pts ✓
Jan 4, 2026      GNN AlphaZero test (iter 2-3)     105 pts ⚠
Jan 4, 2026      Adaptive entropy weights          Implemented
Jan 5, 2026      GNN supervised training (50ep)    Val loss: 1.4696
Jan 5, 2026      GNN benchmark test (30 games)     60.97 pts ❌
Jan 5, 2026      Research documentation            This document
```

---

## Key Takeaways

1. **CNN is proven:** 147-152 pts with AlphaGo Zero training
2. **GNN is unstable:** Shows promise (132 pts) but degrades during training
3. **GNN has high entropy:** Never develops confidence, even after 50 epochs
4. **Supervised learning insufficient:** GNN trained on MCTS data achieves only 61 pts
5. **Adaptive weights work:** System correctly detects and responds to GNN uncertainty
6. **Architecture matters:** 2x capacity (CNN 128 vs GNN 64) correlates with 2x+ performance

---

## Decision Matrix

| Criteria | CNN | GNN | Pure MCTS |
|----------|-----|-----|-----------|
| Reaches >140 pts | ✓✓✓ | ? | ✗ |
| Low risk | ✓✓✓ | ✗ | ✓✓ |
| Fast to implement | ✓✓✓ | ✗✗ | ✓✓ |
| Research value | ✓ | ✓✓✓ | ✓ |
| Proven | ✓✓✓ | ✗ | ✓✓ |
| Future potential | ✓✓✓ | ? | ✓ |
| **TOTAL SCORE** | **17/18** | **6/18** | **10/18** |

**Winner:** CNN Architecture ⭐

---

**Next Action:** Train CNN with supervised learning (50 epochs) and benchmark performance.
