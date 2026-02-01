#!/bin/bash
#
# Safe Model Deployment Script
#
# This script automates the process of:
# 1. Backing up the current production model
# 2. Training a new model on human game data
# 3. Validating the new model against production
# 4. Deploying if approved (with manual confirmation)
#
# Usage:
#   ./scripts/safe_deploy.sh [options]
#
# Options:
#   --data PATTERN      Glob pattern for training data (default: data/recorded_games/*.csv)
#   --filter-human-wins Only use games where human won
#   --min-score N       Minimum human score to include (default: 0)
#   --epochs N          Training epochs (default: 50)
#   --validation-games N Games for validation (default: 200)
#   --threshold N       Improvement threshold in points (default: 3.0)
#   --skip-training     Skip training, only validate existing candidate
#   --auto-deploy       Deploy without manual confirmation (use with caution)
#   --help              Show this help message

set -e

# Default values
DATA_PATTERN="data/recorded_games/*.csv"
FILTER_HUMAN_WINS=""
MIN_SCORE=0
EPOCHS=50
VALIDATION_GAMES=200
THRESHOLD=3.0
SKIP_TRAINING=false
AUTO_DEPLOY=false
NN_ARCH="GNN"
SIMULATIONS=150

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --data)
            DATA_PATTERN="$2"
            shift 2
            ;;
        --filter-human-wins)
            FILTER_HUMAN_WINS="--filter-human-wins"
            shift
            ;;
        --min-score)
            MIN_SCORE="$2"
            shift 2
            ;;
        --epochs)
            EPOCHS="$2"
            shift 2
            ;;
        --validation-games)
            VALIDATION_GAMES="$2"
            shift 2
            ;;
        --threshold)
            THRESHOLD="$2"
            shift 2
            ;;
        --skip-training)
            SKIP_TRAINING=true
            shift
            ;;
        --auto-deploy)
            AUTO_DEPLOY=true
            shift
            ;;
        --nn-architecture)
            NN_ARCH="$2"
            shift 2
            ;;
        --simulations)
            SIMULATIONS="$2"
            shift 2
            ;;
        --help)
            head -30 "$0" | tail -25
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                   SAFE MODEL DEPLOYMENT                        ${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
echo ""

# Step 1: Backup production model
echo -e "${YELLOW}Step 1: Backing up production model...${NC}"
BACKUP_DIR="model_weights_backup_$(date +%Y%m%d_%H%M%S)"

if [ -d "model_weights" ]; then
    cp -r model_weights "$BACKUP_DIR"
    echo -e "${GREEN}✓ Backup created: $BACKUP_DIR${NC}"
else
    echo -e "${YELLOW}⚠ No production model found, skipping backup${NC}"
fi
echo ""

# Step 2: Train on human data (unless skipped)
if [ "$SKIP_TRAINING" = false ]; then
    echo -e "${YELLOW}Step 2: Training on human game data...${NC}"
    echo "  Data pattern: $DATA_PATTERN"
    echo "  Filter human wins: ${FILTER_HUMAN_WINS:-no}"
    echo "  Min score: $MIN_SCORE"
    echo "  Epochs: $EPOCHS"
    echo ""

    # Check if data exists
    DATA_COUNT=$(ls -1 $DATA_PATTERN 2>/dev/null | wc -l || echo "0")
    if [ "$DATA_COUNT" = "0" ]; then
        echo -e "${RED}✗ No data files found matching: $DATA_PATTERN${NC}"
        echo "  Run some games first to generate training data."
        exit 1
    fi
    echo "  Found $DATA_COUNT data file(s)"
    echo ""

    cargo run --release --bin train_from_human_games -- \
        --data "$DATA_PATTERN" \
        $FILTER_HUMAN_WINS \
        --min-score $MIN_SCORE \
        --epochs $EPOCHS \
        --nn-architecture $NN_ARCH \
        --output model_weights_candidate

    echo -e "${GREEN}✓ Training complete${NC}"
else
    echo -e "${YELLOW}Step 2: Skipping training (using existing candidate)${NC}"
    if [ ! -d "model_weights_candidate" ]; then
        echo -e "${RED}✗ No candidate model found at model_weights_candidate/${NC}"
        exit 1
    fi
fi
echo ""

# Step 3: Validate new model
echo -e "${YELLOW}Step 3: Validating candidate model...${NC}"
echo "  Games: $VALIDATION_GAMES"
echo "  Simulations: $SIMULATIONS"
echo "  Threshold: $THRESHOLD points"
echo ""

VALIDATION_RESULT=0
cargo run --release --bin validate_new_model -- \
    --candidate model_weights_candidate \
    --production model_weights \
    --games $VALIDATION_GAMES \
    --simulations $SIMULATIONS \
    --threshold $THRESHOLD \
    --nn-architecture $NN_ARCH || VALIDATION_RESULT=$?

echo ""

# Step 4: Deploy if approved
if [ $VALIDATION_RESULT -eq 0 ]; then
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}                    VALIDATION PASSED                          ${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo ""

    if [ "$AUTO_DEPLOY" = true ]; then
        echo -e "${YELLOW}Auto-deploy enabled, deploying...${NC}"
        cp -r model_weights_candidate/* model_weights/
        echo -e "${GREEN}✓ New model deployed to model_weights/${NC}"
    else
        echo -e "${YELLOW}Deploy new model? This will replace the production weights.${NC}"
        read -p "Type 'yes' to confirm: " CONFIRM

        if [ "$CONFIRM" = "yes" ]; then
            cp -r model_weights_candidate/* model_weights/
            echo -e "${GREEN}✓ New model deployed to model_weights/${NC}"
            echo ""
            echo "Backup available at: $BACKUP_DIR"
            echo "To rollback: cp -r $BACKUP_DIR/* model_weights/"
        else
            echo -e "${YELLOW}Deployment cancelled.${NC}"
            echo "Candidate model available at: model_weights_candidate/"
        fi
    fi
else
    echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}                    VALIDATION FAILED                          ${NC}"
    echo -e "${RED}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo "The candidate model did not meet the improvement threshold."
    echo "Consider:"
    echo "  - Collecting more training data"
    echo "  - Adjusting training parameters"
    echo "  - Lowering the threshold (--threshold)"
    echo ""
    echo "Candidate model available at: model_weights_candidate/"
    exit 1
fi

echo ""
echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                        COMPLETE                                ${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}"
