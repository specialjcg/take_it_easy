#!/bin/bash
# Script d'entraînement Gold GNN
# Architecture: [256, 256, 128, 64] (4 couches)
# Objectif: +3-5 pts → 142-144 pts

set -e

echo "=========================================="
echo "🚀 Gold GNN Training - Approche Hybride"
echo "=========================================="
echo ""
echo "Configuration:"
echo "  Architecture: Gold GNN [256, 256, 128, 64]"
echo "  Parties: 500 (vs 200 baseline)"
echo "  Simulations: 150/move"
echo "  Mode: Offline training (self-play)"
echo "  Durée estimée: ~12h"
echo "  Gain attendu: +3-5 pts → 142-144 pts"
echo ""
echo "=========================================="
echo ""

# Créer un dossier pour les poids Gold GNN
mkdir -p model_weights_gold_gnn

# Lancer l'entraînement
echo "⏳ Début de l'entraînement..."
echo "📊 Logs seront sauvegardés dans: training_gold_gnn_500games.log"
echo ""

# Compiler d'abord
echo "🔨 Compilation en mode release..."
cargo build --release

# Lancer l'entraînement avec logging
RUST_LOG=info cargo run --release --bin take_it_easy -- \
  --mode training \
  --offline-training \
  --nn-architecture gnn \
  --num-games 500 \
  --num-simulations 150 \
  --evaluation-interval 50 \
  --min-score-high 140.0 \
  --min-score-medium 120.0 \
  --medium-mix-ratio 0.2 \
  2>&1 | tee training_gold_gnn_500games.log

echo ""
echo "✅ Entraînement terminé !"
echo "📁 Poids sauvegardés dans: model_weights/"
echo "📊 Logs disponibles dans: training_gold_gnn_500games.log"
echo ""
echo "🎯 Prochaine étape: Benchmark Gold GNN vs CNN baseline"
echo "Commande: cargo run --release --bin compare_mcts -- -g 50 -s 150 --nn-architecture gnn"
