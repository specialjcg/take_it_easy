# Documentation Index

This directory contains research documentation and performance analysis for the Take It Easy AI project.

---

## Quick Start

**Want to know what happened?** → Read [RESEARCH_SUMMARY.md](./RESEARCH_SUMMARY.md)

**Need detailed analysis?** → Read [research_gnn_vs_cnn_performance.md](./research_gnn_vs_cnn_performance.md)

**Want to compare architectures?** → Read [performance_comparison.md](./performance_comparison.md)

---

## Document Overview

### [RESEARCH_SUMMARY.md](./RESEARCH_SUMMARY.md)
**Type:** Quick reference (5-minute read)
**Purpose:** TL;DR of the research findings

**Contents:**
- Key findings summary
- Problem diagnosis
- Recommendations with commands
- Quick decision matrix

**Best for:**
- Getting up to speed quickly
- Deciding next steps
- Finding specific commands

---

### [research_gnn_vs_cnn_performance.md](./research_gnn_vs_cnn_performance.md)
**Type:** Full research report (20-minute read)
**Purpose:** Comprehensive investigation documentation

**Contents:**
- Executive summary
- Detailed methodology
- Complete results analysis
- Root cause investigation
- Technical implementation details
- Conclusions and recommendations
- Appendices with raw data

**Best for:**
- Understanding the full investigation
- Technical deep-dive
- Academic/research purposes
- Future reference

---

### [performance_comparison.md](./performance_comparison.md)
**Type:** Performance comparison tables (10-minute read)
**Purpose:** Side-by-side comparison of all methods

**Contents:**
- Performance summary table
- Detailed results by architecture
- Architectural specifications
- Entropy evolution analysis
- Cost-benefit analysis
- Decision matrix
- Historical timeline

**Best for:**
- Comparing different approaches
- Understanding trade-offs
- Making architecture decisions
- Visualizing performance data

---

## Research Summary

### Objective
Improve neural network performance to reach >140 points average score.

### Result
GNN with adaptive hybrid weights achieved **60.97 ± 29.24 pts** (30 games).

**Status:** ❌ Target not achieved

### Key Finding
The baseline performance of 147-152 pts was achieved using **CNN architecture**, not GNN.

---

## Quick Facts

| Metric | Value |
|--------|-------|
| **Target Score** | >140 pts |
| **CNN Performance** | 147-152 pts ✓ |
| **GNN Performance** | 60.97 pts ❌ |
| **Pure MCTS** | 84-88 pts |
| **Gap to Target** | -86 pts |
| **Success Rate** | 0/30 games >140 pts |

---

## Architecture Comparison

| Feature | CNN | GNN |
|---------|-----|-----|
| **Channels** | [128, 128, 96] | [64, 64, 64] |
| **Style** | ResNet (AlphaZero) | Graph Network |
| **Spatial** | 2D Convolutions | Graph Representation |
| **Best Score** | 147-152 pts | 132.8 pts (iter 1) |
| **Stability** | ✓ Excellent | ✗ Degrades during training |
| **Status** | ✓ Proven | ❌ Unstable |

---

## Recommendations

### Primary Recommendation: Switch to CNN ⭐

**Approach:**
1. Train CNN with supervised learning (50 epochs)
2. Benchmark performance (30 games)
3. Validate >140 pts target achieved

**Success Probability:** 90%
**Time Required:** 1-2 hours
**Risk:** Low (proven architecture)

**Command:**
```bash
cd /home/jcgouleau/IdeaProjects/RustProject/take_it_easy

RUST_LOG=warn LIBTORCH=/home/jcgouleau/libtorch-clean/libtorch \
LD_LIBRARY_PATH=/home/jcgouleau/libtorch-clean/libtorch/lib:$LD_LIBRARY_PATH \
./target/release/supervised_trainer_csv \
  --data supervised_dataset_2k.csv \
  --arch cnn \
  --epochs 50
```

---

## Key Files Referenced

### Source Code
- `src/mcts/algorithm.rs` - MCTS with adaptive weights
- `src/mcts/hyperparameters.rs` - Weight strategies
- `src/neural/policy_value_net.rs` - CNN and GNN implementations
- `src/neural/gnn.rs` - Graph neural network
- `src/bin/supervised_trainer_csv.rs` - Training script
- `src/bin/test_gnn_benchmark.rs` - Benchmarking tool

### Data Files
- `supervised_dataset_2k.csv` - Training data (38k examples)
- `training_history_50iter.csv` - CNN baseline results
- `training_history_gnn_test.csv` - GNN AlphaZero results
- `compare_mcts_log.csv` - MCTS comparison logs

### Test Logs
- `/tmp/supervised_training_50epoch.log` - GNN training log
- `/tmp/gnn_50epoch_hybrid_test.log` - GNN benchmark log
- `/tmp/training_monitor.log` - Training monitor output

---

## Research Timeline

```
Jan 5, 2026  12:00-12:06  GNN supervised training (50 epochs)
Jan 5, 2026  12:06-12:52  GNN benchmark test (30 games)
Jan 5, 2026  12:52-13:45  Analysis and documentation
```

**Total Investigation Time:** ~2 hours

---

## Problem Diagnosis

### Issue: GNN High Entropy

**Symptom:**
```
Turn  0: entropy=0.807 (norm) - Expected
Turn  5: entropy=0.424 (norm) - Should be <0.4
Turn 10: entropy=0.609 (norm) - Should be <0.2 ❌
Turn 15: entropy=0.793 (norm) - Should be <0.1 ❌
```

**Interpretation:** GNN never gains confidence, indicating poor learning despite 50 epochs of supervised training.

### Root Cause: Wrong Architecture

**Evidence:**
1. Historical baseline used CNN (147-152 pts), not GNN
2. GNN showed promise (132 pts) but degrades during training
3. CNN has 2x capacity (128 vs 64 channels)
4. CNN's spatial convolutions better suited for hexagonal board

---

## Next Steps

### Immediate Action (Recommended)
1. Train CNN with supervised learning
2. Benchmark CNN performance
3. Compare CNN vs GNN vs Pure MCTS

### If CNN Succeeds (>140 pts)
1. Deploy as production model
2. Optionally: Continue with AlphaGo Zero self-play for further improvement
3. Target: 160+ pts with self-play

### If CNN Falls Short (<140 pts)
1. Investigate training data quality
2. Try AlphaGo Zero self-play training
3. Consider hybrid architectures

### Long-term Research (Optional)
1. Understand why GNN fails for this spatial problem
2. Explore CNN-GNN hybrid architectures
3. Investigate Transformer-based approaches

---

## Related Documentation

- Project README: `../README.md`
- Source code: `../src/`
- Training scripts: `../src/bin/`
- Neural networks: `../src/neural/`
- MCTS implementation: `../src/mcts/`

---

## Contact & Updates

**Last Updated:** January 5, 2026
**Status:** Investigation complete, awaiting decision on next steps
**Next Review:** After CNN training and benchmarking

---

## Appendix: Quick Commands

### Train CNN
```bash
./target/release/supervised_trainer_csv \
  --data supervised_dataset_2k.csv \
  --arch cnn \
  --epochs 50 \
  --batch-size 512 \
  --policy-lr 0.001 \
  --value-lr 0.0001
```

### Benchmark Performance
```bash
./target/release/test_gnn_benchmark \
  --games 30 \
  --simulations 150
```

### Monitor Training
```bash
tail -f /tmp/cnn_training_50epoch.log | grep -E "Epoch|policy|value"
```

### Compare Results
```bash
# GNN result
echo "GNN: 60.97 ± 29.24 pts"

# CNN result (after training)
tail -3 /tmp/cnn_benchmark_test.log
```

---

## Version History

| Date | Version | Changes |
|------|---------|---------|
| 2026-01-05 | 1.0 | Initial research documentation |

---

**End of Documentation Index**
