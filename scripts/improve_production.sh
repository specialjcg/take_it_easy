#!/bin/bash
#
# PRODUCTION AI SELF-IMPROVEMENT SYSTEM
# =====================================
# Uses CNN + Q-Net Hybrid MCTS (same as production)
#
# Strategy:
# 1. Generate many games with production AI (different random decks)
# 2. Keep only high-scoring games (>= MIN_SCORE)
# 3. Train new model on best examples
# 4. Validate new model beats production
# 5. Deploy if statistically better
#

set -e  # Exit on error

# Configuration
GAMES=${1:-500}           # Number of games to generate
SIMULATIONS=${2:-200}     # MCTS simulations per move
MIN_SCORE=${3:-130}       # Minimum score for training data
EPOCHS=${4:-50}           # Training epochs
VALIDATION_GAMES=100      # Games for validation
THRESHOLD=3.0             # Minimum improvement (points)

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║         PRODUCTION AI SELF-IMPROVEMENT SYSTEM                ║"
echo "╠══════════════════════════════════════════════════════════════╣"
echo "║  Using: CNN Policy + Value + Q-Net Hybrid MCTS               ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Configuration:"
echo "  Games:       $GAMES"
echo "  Simulations: $SIMULATIONS"
echo "  Min Score:   $MIN_SCORE"
echo "  Epochs:      $EPOCHS"
echo ""

# Create data directory
mkdir -p data
mkdir -p model_weights_candidate

# ============================================================================
# STEP 1: Generate high-quality training data
# ============================================================================
echo "┌────────────────────────────────────────────────────────────────┐"
echo "│ STEP 1: Generating training data (Q-Net Hybrid MCTS)          │"
echo "└────────────────────────────────────────────────────────────────┘"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
TRAINING_FILE="data/selfplay_${TIMESTAMP}.csv"

cargo run --release --bin ai_arena -- \
  --model-a model_weights/ \
  --model-b model_weights/ \
  --name-a "Production" \
  --name-b "Production" \
  --games $GAMES \
  --simulations $SIMULATIONS \
  --hybrid-mcts \
  --generate-training-data \
  --min-score $MIN_SCORE \
  --training-output "$TRAINING_FILE"

# Check if we have enough data
MOVE_COUNT=$(wc -l < "$TRAINING_FILE")
MOVE_COUNT=$((MOVE_COUNT - 1))  # Subtract header

echo ""
echo "Generated $MOVE_COUNT training moves from games >= $MIN_SCORE pts"

if [ $MOVE_COUNT -lt 500 ]; then
    echo ""
    echo "⚠️  Warning: Only $MOVE_COUNT moves. Consider:"
    echo "    - Lower --min-score (currently $MIN_SCORE)"
    echo "    - More --games (currently $GAMES)"
    echo ""
    read -p "Continue anyway? (y/n) " confirm
    if [ "$confirm" != "y" ]; then
        echo "Aborted."
        exit 1
    fi
fi

# ============================================================================
# STEP 2: Backup current production
# ============================================================================
echo ""
echo "┌────────────────────────────────────────────────────────────────┐"
echo "│ STEP 2: Backing up production model                           │"
echo "└────────────────────────────────────────────────────────────────┘"

BACKUP_DIR="model_weights_backup_${TIMESTAMP}"
cp -r model_weights "$BACKUP_DIR"
echo "✅ Backup saved to: $BACKUP_DIR"

# ============================================================================
# STEP 3: Train candidate model
# ============================================================================
echo ""
echo "┌────────────────────────────────────────────────────────────────┐"
echo "│ STEP 3: Training candidate model                              │"
echo "└────────────────────────────────────────────────────────────────┘"

# First copy production weights as starting point
rm -rf model_weights_candidate/cnn
cp -r model_weights/cnn model_weights_candidate/

cargo run --release --bin train_from_human_games -- \
  --data "$TRAINING_FILE" \
  --epochs $EPOCHS \
  --output model_weights_candidate \
  --nn-architecture CNN

# ============================================================================
# STEP 4: Validate candidate vs production
# ============================================================================
echo ""
echo "┌────────────────────────────────────────────────────────────────┐"
echo "│ STEP 4: Validating candidate vs production                    │"
echo "└────────────────────────────────────────────────────────────────┘"

cargo run --release --bin validate_new_model -- \
  --candidate model_weights_candidate \
  --production model_weights \
  --games $VALIDATION_GAMES \
  --simulations $SIMULATIONS \
  --threshold $THRESHOLD \
  --nn-architecture CNN

VALIDATION_RESULT=$?

# ============================================================================
# STEP 5: Deploy if approved
# ============================================================================
echo ""
echo "┌────────────────────────────────────────────────────────────────┐"
echo "│ STEP 5: Deployment decision                                   │"
echo "└────────────────────────────────────────────────────────────────┘"

if [ $VALIDATION_RESULT -eq 0 ]; then
    echo ""
    echo "✅ Candidate model APPROVED!"
    echo ""
    echo "Production backup: $BACKUP_DIR"
    echo "Training data:     $TRAINING_FILE"
    echo ""
    read -p "Deploy new model to production? (y/n) " confirm
    if [ "$confirm" = "y" ]; then
        cp -r model_weights_candidate/cnn/* model_weights/cnn/
        echo ""
        echo "🚀 NEW MODEL DEPLOYED!"
        echo ""
        echo "To rollback: cp -r $BACKUP_DIR/* model_weights/"
    else
        echo "Deployment cancelled. Candidate saved at: model_weights_candidate/"
    fi
else
    echo ""
    echo "❌ Candidate model REJECTED"
    echo ""
    echo "The new model did not improve significantly."
    echo "Production model unchanged."
    echo ""
    echo "Suggestions:"
    echo "  - Generate more training data (increase --games)"
    echo "  - Use higher quality threshold (increase --min-score)"
    echo "  - Train longer (increase --epochs)"
    echo ""
    echo "Backup available at: $BACKUP_DIR"
fi

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "                    SELF-IMPROVEMENT COMPLETE"
echo "════════════════════════════════════════════════════════════════"
