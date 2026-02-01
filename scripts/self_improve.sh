#!/bin/bash
# Self-improvement loop for Take It Easy AI
# Usage: ./scripts/self_improve.sh [games] [simulations] [min_score]

GAMES=${1:-200}
SIMULATIONS=${2:-200}
MIN_SCORE=${3:-80}
EPOCHS=30

echo "=========================================="
echo "  AI Self-Improvement Loop"
echo "=========================================="
echo "Games: $GAMES"
echo "Simulations: $SIMULATIONS"
echo "Min Score: $MIN_SCORE"
echo "Epochs: $EPOCHS"
echo ""

# Step 1: Generate training data from high-scoring games (with Q-Net Hybrid)
echo "📊 Step 1: Generating training data from AI vs AI (Q-Net Hybrid)..."
cargo run --release --bin ai_arena -- \
  --model-a model_weights/ \
  --model-b model_weights/ \
  --games $GAMES \
  --simulations $SIMULATIONS \
  --hybrid-mcts \
  --generate-training-data \
  --min-score $MIN_SCORE \
  --training-output data/selfplay_training.csv

MOVE_COUNT=$(wc -l < data/selfplay_training.csv)
echo "Generated $((MOVE_COUNT - 1)) training moves"

if [ $MOVE_COUNT -lt 100 ]; then
    echo "❌ Not enough training data. Try lower --min-score or more --games"
    exit 1
fi

# Step 2: Backup current production model
echo ""
echo "💾 Step 2: Backing up production model..."
BACKUP_DIR="model_weights_backup_$(date +%Y%m%d_%H%M%S)"
cp -r model_weights "$BACKUP_DIR"
echo "Backed up to: $BACKUP_DIR"

# Step 3: Train candidate model
echo ""
echo "🏋️ Step 3: Training candidate model..."
cargo run --release --bin train_from_human_games -- \
  --data "data/selfplay_training.csv" \
  --epochs $EPOCHS \
  --output model_weights_candidate

# Step 4: Validate candidate vs production
echo ""
echo "🔍 Step 4: Validating candidate vs production..."
cargo run --release --bin validate_new_model -- \
  --candidate model_weights_candidate \
  --production model_weights \
  --games 50 \
  --simulations 150 \
  --threshold 2.0

# Check validation result
if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Candidate APPROVED!"
    read -p "Deploy new model? (y/n) " confirm
    if [ "$confirm" = "y" ]; then
        cp -r model_weights_candidate/cnn/* model_weights/cnn/
        echo "🚀 Deployed! New model is now in production."
    fi
else
    echo ""
    echo "❌ Candidate REJECTED - keeping current production model"
    echo "Backup available at: $BACKUP_DIR"
fi
