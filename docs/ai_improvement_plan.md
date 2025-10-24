# AI Improvement Plan – Target Average > 140

## Current Baseline
- `compare_mcts` (200 games, 150 sims) → Pure MCTS 103.8, NN-guided 128.34, Δ +24.54, NN wins 75.5 %.
- Training pipeline filters games ≥140, weights refreshed (8-channel board tensor, entropy-aware heuristics).

## Objectives
1. Raise NN-guided average score above 140.
2. Maintain or increase win rate versus pure MCTS.
3. Ensure training/benchmark flow is repeatable and automated.

## Workstreams

### 1. Data Generation & Training Strategy
- Increase self-play volume (≥1k games per iteration) with filtered buffer.
- Alternate opponents (pure MCTS, heuristic-only, previous checkpoint) to diversify states.
- Introduce replay buffer mix: 80 % ≥140, 20 % 120–140 for diversity.
- Implement arena evaluation: accept new weights only if Δ_avg ≥ +X.

### 2. Architecture Enhancements
- Extend PolicyNet/ValueNet with deeper residual stacks + wider heads.
- Evaluate adding squeeze‑excitation or attention over hex neighbours.
- Prototype graph-based encoder (nodes = 19 cells, edges = adjacencies).
  - Consider PyTorch Geometric or a lightweight custom message-passing layer.
  - Encode board as graph features (tile bands, completion ratios) and aggregate with GAT/GIN.

### 3. MCTS Tuning
- Dynamic simulations per phase (more in opening/endgame).
- Adaptive `c_puct` schedule per entropy & depth.
- Early prune moves with low value confidence.
- Log per-move search stats for post-run analysis.

### 4. Heuristics & Feature Engineering
- Refine positional heuristics via learned coefficients (optimize on dataset).
- Add more board features: alignment completion, potential score variance, entropy history.
- Experiment with temperature annealing when sampling policy for data generation.

### 5. Instrumentation & Automation
- Extend `compare_mcts_log.csv` with seeds/config metadata.
- Add CI step running `cargo test model_weights_sanity` + small `compare_mcts` smoke test.
- Generate plots (Python notebook) from `compare_mcts_log.csv` for trend tracking.
- Document procedures in README/AGENTS for reproducibility.

## Next Milestones
1. Implement arena-based self-play + logging automation.
2. Introduce deeper Policy/Value architecture (v2 checkpoint).
3. Collect ≥3 iterations of train→arena→compare to evaluate trajectory toward 140+.
