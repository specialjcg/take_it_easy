#!/bin/bash
# Phase 1: Evaluation Weights Grid Search
# Goal: Find optimal weight distribution for CNN, Rollout, Heuristic, and Contextual components
# Constraint: All weights must sum to 1.0

set -e

GAMES=20
SEED=2025
SIMULATIONS=150
LOG_FILE="hyperparameter_tuning_log.csv"

echo "🔍 Phase 1: Evaluation Weights Grid Search"
echo "=========================================="
echo "Games per config: $GAMES"
echo "Simulations: $SIMULATIONS"
echo "Seed: $SEED"
echo "Log file: $LOG_FILE"
echo ""

# Counter for progress tracking
total_configs=0
completed=0

# First pass: count total configurations
for w_cnn in 0.55 0.60 0.65; do
  for w_roll in 0.15 0.20 0.25; do
    for w_heur in 0.05 0.10 0.15; do
      w_ctx=$(echo "1.0 - $w_cnn - $w_roll - $w_heur" | bc -l)

      # Check if contextual weight is in valid range [0.05, 0.15]
      if (( $(echo "$w_ctx >= 0.04 && $w_ctx <= 0.16" | bc -l) )); then
        total_configs=$((total_configs + 1))
      fi
    done
  done
done

echo "Total configurations to test: $total_configs"
echo ""

# Second pass: run experiments
for w_cnn in 0.55 0.60 0.65; do
  for w_roll in 0.15 0.20 0.25; do
    for w_heur in 0.05 0.10 0.15; do
      w_ctx=$(echo "1.0 - $w_cnn - $w_roll - $w_heur" | bc -l)

      # Check if contextual weight is in valid range [0.05, 0.15]
      if (( $(echo "$w_ctx >= 0.04 && $w_ctx <= 0.16" | bc -l) )); then
        completed=$((completed + 1))

        echo "[$completed/$total_configs] Testing: CNN=$w_cnn, Roll=$w_roll, Heur=$w_heur, Ctx=$w_ctx"

        cargo run --release --bin tune_hyperparameters -- \
          --games $GAMES \
          --seed $SEED \
          --simulations $SIMULATIONS \
          --weight-cnn $w_cnn \
          --weight-rollout $w_roll \
          --weight-heuristic $w_heur \
          --weight-contextual $w_ctx \
          --log-path $LOG_FILE

        echo ""
      fi
    done
  done
done

echo "✅ Phase 1 Complete!"
echo "Results saved to: $LOG_FILE"
echo ""
echo "📊 To analyze results, run:"
echo "   python3 scripts/analyze_hyperparameters.py $LOG_FILE"
