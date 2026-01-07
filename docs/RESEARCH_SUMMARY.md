# Quick Reference: GNN vs CNN Performance Research

**Date:** January 5, 2026
**Full report:** [research_gnn_vs_cnn_performance.md](./research_gnn_vs_cnn_performance.md)

---

## TL;DR

**Objective:** Achieve >140 pts average score
**Result:** GNN with adaptive weights = **60.97 pts** ❌
**Root cause:** Wrong architecture - baseline used **CNN** (147-152 pts), not GNN

---

## Key Findings

| Architecture | Method | Score | Status |
|--------------|--------|-------|--------|
| CNN | AlphaGo Zero | **147-152 pts** | ✓ Proven baseline |
| GNN | AlphaGo Zero (iter 1) | 132.78 pts | ✓ Promising but unstable |
| GNN | Supervised (50 epochs) | **60.97 pts** | ❌ Failed |
| Pure MCTS | Rollout-based | ~84-88 pts | Baseline reference |

---

## Problem: GNN Has High Entropy

**Expected:** Entropy <0.2 (confident predictions)
**Actual:** Entropy 0.6-0.8 even after 50 epochs (very uncertain)

```
Turn  0: entropy=0.807 → GNN weight reduced to 15% (from 20%)
Turn 15: entropy=0.793 → GNN weight reduced to 57% (from 75%)
```

**Interpretation:** GNN didn't learn well despite 50 epochs of supervised training.

---

## What Worked

- ✓ Adaptive weight system correctly detected GNN uncertainty
- ✓ Turn-based strategy implemented properly
- ✓ Training pipeline completed successfully
- ✓ No architecture mismatches (fixed 8-channel issue)

---

## What Failed

- ❌ GNN performance: 61 pts vs 140 pts target (57% below)
- ❌ GNN consistently degrades during AlphaGo Zero training
- ❌ High entropy indicates fundamental learning problem

---

## Recommendations

### Option 1: Switch to CNN (90% success probability) ⭐ RECOMMENDED

**Action:**
```bash
# Train CNN with supervised learning
./supervised_trainer_csv --data supervised_dataset_2k.csv --arch cnn --epochs 50

# Benchmark
./test_gnn_benchmark --games 30 --simulations 150
```

**Expected result:** 140-150 pts (based on historical evidence)
**Effort:** Low (architecture exists)
**Time:** 1-2 hours

### Option 2: Improve GNN (50% success probability)

**Action:**
- Increase capacity: [128, 128, 96] (match CNN)
- Train 100-200 epochs
- Try AlphaGo Zero self-play

**Expected result:** Uncertain
**Effort:** High
**Time:** 1-2 days

### Option 3: Optimize Pure MCTS (30% success probability)

**Action:**
- Tune hyperparameters
- Improve rollout policy

**Expected result:** Maybe 90-110 pts (unlikely to reach 140)
**Effort:** Medium
**Time:** Several hours

---

## Architecture Comparison

### CNN (AlphaZero-style)
```rust
Channels: [128, 128, 96]  // 2x capacity of GNN
ResNet blocks: 3
Spatial: 2D convolutions (preserves board structure)
Proven: 147-152 pts ✓
```

### GNN (Graph-based)
```rust
Channels: [64, 64, 64]    // Half of CNN
Layers: Simple feedforward
Spatial: Graph representation
Proven: Unstable, degrades during training ❌
```

---

## Quick Decision Matrix

| Goal | Recommended Approach |
|------|---------------------|
| Reach >140 pts quickly | Option 1: CNN |
| Research why GNN fails | Option 2: Improve GNN |
| No neural network complexity | Option 3: Pure MCTS |
| Best ROI | **Option 1: CNN** ⭐ |

---

## Files & Commands

### Run CNN Training
```bash
cd /home/jcgouleau/IdeaProjects/RustProject/take_it_easy

# Build
cargo build --release

# Train
RUST_LOG=warn LIBTORCH=/home/jcgouleau/libtorch-clean/libtorch \
LD_LIBRARY_PATH=/home/jcgouleau/libtorch-clean/libtorch/lib:$LD_LIBRARY_PATH \
./target/release/supervised_trainer_csv \
  --data supervised_dataset_2k.csv \
  --arch cnn \
  --epochs 50 \
  --batch-size 512 \
  --policy-lr 0.001 \
  --value-lr 0.0001 \
  2>&1 | tee /tmp/cnn_training_50epoch.log

# Test
RUST_LOG=warn LIBTORCH=/home/jcgouleau/libtorch-clean/libtorch \
LD_LIBRARY_PATH=/home/jcgouleau/libtorch-clean/libtorch/lib:$LD_LIBRARY_PATH \
./target/release/test_gnn_benchmark \
  --games 30 \
  --simulations 150 \
  2>&1 | tee /tmp/cnn_benchmark_test.log
```

### Key Files
- Full report: `docs/research_gnn_vs_cnn_performance.md`
- Training data: `supervised_dataset_2k.csv` (38k examples)
- CNN implementation: `src/neural/policy_value_net.rs` (PolicyNetCNN)
- GNN implementation: `src/neural/gnn.rs`
- MCTS algorithm: `src/mcts/algorithm.rs`

### Recent Logs
- GNN training: `/tmp/supervised_training_50epoch.log`
- GNN test: `/tmp/gnn_50epoch_hybrid_test.log`
- Historical baseline: `training_history_50iter.csv`

---

## Next Action

**Recommended:** Train CNN architecture with existing dataset

Expected timeline:
1. Training: ~30-60 minutes
2. Testing: ~10 minutes
3. Analysis: ~5 minutes

**Total:** ~1-2 hours to validate if CNN reaches >140 pts target.
