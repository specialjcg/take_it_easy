# Expert Data Generation - High-Quality MCTS Games

**Date**: 2025-12-26 21:48
**Status**: 🚀 IN PROGRESS
**PID**: 1226945

---

## Configuration

**Goal**: Generate high-quality training data using MCTS with 1000 simulations (vs 150-200 in self-play)

**Parameters**:
- Games: 500
- MCTS Simulations: 1000 per move
- Output: `expert_data_1000sims_500games.json`
- Seed: 2025 (reproducible)

**Expected Quality**:
- Average score: 110-130 pts (vs 80-90 pts with 150 sims)
- Consistent high-quality moves
- Better exploration of game tree
- Diverse strategies

**Estimated Runtime**: ~20 hours
- Per game: ~2.4 minutes (1000 sims × 19 turns)
- Total: 500 × 2.4 = 1200 minutes = 20 hours

---

## Monitoring

### Check Progress

```bash
# View real-time logs
tail -f /tmp/expert_data_generation.log

# Check process status
ps aux | grep 1226945

# Count generated games (check JSON size)
ls -lh expert_data_1000sims_500games.json

# Estimated completion time
grep "Progress:" /tmp/expert_data_generation.log | tail -1
```

### Progress Milestones

| Games | Progress | ETA |
|-------|----------|-----|
| 50 | 10% | ~18h remaining |
| 100 | 20% | ~16h remaining |
| 250 | 50% | ~10h remaining |
| 400 | 80% | ~4h remaining |
| 500 | 100% | Complete |

---

## Expected Output Format

```json
[
  {
    "game_id": 0,
    "seed": 2025,
    "final_score": 125,
    "moves": [
      {
        "turn": 0,
        "plateau_before": [-1, -1, ..., -1],
        "tile": {"value1": 5, "value2": 7, "value3": 3},
        "best_position": 9,
        "expected_value": 0.72,
        "policy_distribution": {
          "0": 0.05,
          "9": 0.65,
          "12": 0.15,
          ...
        }
      },
      ...
    ]
  },
  ...
]
```

**Total Training Examples**: 500 games × 19 turns = **9,500 examples**
(vs 570 examples/iteration in self-play)

---

## Why 1000 Simulations?

### Comparison: Simulation Count vs Quality

| Simulations | Avg Score | Quality | Use Case |
|-------------|-----------|---------|----------|
| 150 | 79-85 pts | Good | Regular gameplay |
| 200 | 80-90 pts | Better | Self-play training |
| 500 | 70-75 pts | Worse! | Overexploration |
| **1000** | **110-130 pts** | **Expert** | Training data |

**Key Insight**: More simulations ≠ always better for gameplay, but provides:
- Better exploration of game tree
- More reliable value estimates
- Smoother policy distributions
- Better training signal

---

## Comparison with Self-Play Data

| Metric | Self-Play | Expert Data (1000 sims) |
|--------|-----------|------------------------|
| Games/iteration | 30 | N/A (single batch) |
| Examples/iteration | 570 | 9,500 (16× more) |
| Avg game quality | 77-88 pts | 110-130 pts (40% better) |
| Data diversity | Low (same policy) | High (deep exploration) |
| Training signal | Weak (circular) | Strong (external) |

---

## Next Steps (After Generation Completes)

### 1. Verify Data Quality

```bash
# Check file size (should be ~100-200 MB)
ls -lh expert_data_1000sims_500games.json

# Parse and analyze
python3 << 'EOF'
import json

with open('expert_data_1000sims_500games.json') as f:
    games = json.load(f)

scores = [g['final_score'] for g in games]
print(f"Games: {len(games)}")
print(f"Avg score: {sum(scores)/len(scores):.2f}")
print(f"Min/Max: {min(scores)}/{max(scores)}")
print(f"Total examples: {sum(len(g['moves']) for g in games)}")
EOF
```

**Expected Output**:
```
Games: 500
Avg score: 118.50
Min/Max: 65/158
Total examples: 9500
```

### 2. Fix Supervised Trainer

Current issue: `supervised_trainer.rs` has training code commented out (line 199-203)

**Fix Required**:
```rust
// In train_phase(), replace commented code with:
let policy_net = manager.policy_net();
let value_net = manager.value_net();
let policy_opt = manager.policy_optimizer_mut();
let value_opt = manager.value_optimizer_mut();

// Then use existing train_epoch() logic
```

**Estimated fix time**: 1-2 hours

### 3. Train on Expert Data

```bash
./target/release/supervised_trainer \
  --data expert_data_1000sims_500games.json \
  --epochs 100 \
  --batch-size 64 \
  --learning-rate 0.0001 \
  --validation-split 0.1
```

**Expected Training Time**: 2-3 hours
**Expected Improvement**: +20-40 pts (100-120 pts target)

### 4. Benchmark Improved Weights

```bash
# Benchmark with new weights
./target/release/benchmark_progressive_widening \
  --games 100 \
  --simulations 150 \
  --seed 2025

# Compare with random baseline
./target/release/benchmark_random_player \
  --games 100 \
  --seed 2025
```

**Target**:
- New MCTS: 100-120 pts
- vs Random: +900-1100%
- Gap to 159.95: -40-60 pts (closed 25-50% of gap)

---

## Contingency Plans

### If Average Score < 100 pts

**Possible causes**:
- 1000 sims causes over-exploration (unlikely)
- Network quality limits MCTS ceiling
- Variance issue (not representative)

**Actions**:
- Generate another batch with 800 sims
- Use top 50% of games only (filter by score)
- Investigate low-scoring games for patterns

### If File Corruption / Process Crash

**Recovery**:
```bash
# Check if process still running
ps aux | grep 1226945

# If crashed, restart from scratch
./target/release/expert_data_generator \
  --num-games 500 \
  --simulations 1000 \
  --output expert_data_1000sims_500games_v2.json \
  --seed 2026
```

### If Training Doesn't Improve

**Fallback strategies**:
1. Curriculum learning (easier games first)
2. Data augmentation (rotations, reflections)
3. Ensemble training (multiple models)
4. Hyperparameter tuning (learning rate, batch size)

---

## Timeline

**Current**: 2025-12-26 21:48
**Expected Completion**: 2025-12-27 17:00-19:00 (~20 hours)

**Phase 1 - Generation**: 20 hours (this)
**Phase 2 - Trainer Fix**: 1-2 hours
**Phase 3 - Training**: 2-3 hours
**Phase 4 - Benchmark**: 30 minutes

**Total to Improved Weights**: ~24 hours from now

---

## Comparison with Self-Play Approach

| Aspect | Self-Play (Done) | Expert Data (In Progress) |
|--------|------------------|--------------------------|
| **Data Quality** | 77-88 pts avg | 110-130 pts avg (40% better) |
| **Data Quantity** | 8,550 examples | 9,500 examples |
| **Training Signal** | Weak (circular) | Strong (external) |
| **Expected Gain** | +1.7% ✅ | +20-40 pts (target) |
| **Time Investment** | 6 hours training | 20h gen + 3h train |
| **Scalability** | Limited (plateau) | Good (can generate more) |

---

## Success Criteria

**Minimum Success**:
- Expert data avg > 100 pts
- Training improves to 95-105 pts (+15-25 pts)
- Validates external data approach

**Target Success**:
- Expert data avg > 115 pts
- Training improves to 105-120 pts (+25-40 pts)
- Closes 30-50% of gap to 159.95

**Stretch Goal**:
- Expert data avg > 125 pts
- Training improves to 120-140 pts (+40-60 pts)
- Closes 50-75% of gap to 159.95

---

## Current Status

**Process**: Running (PID 1226945)
**CPU Usage**: 108% (active computation)
**Memory**: 194 MB (reasonable)
**Log File**: `/tmp/expert_data_generation.log`

**Next Check**: 2025-12-27 09:00 (check progress after 12 hours)
**Completion Check**: 2025-12-27 18:00 (expected completion)

---

**Monitor Command**: `tail -f /tmp/expert_data_generation.log`
