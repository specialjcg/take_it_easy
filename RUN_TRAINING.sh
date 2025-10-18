#!/bin/bash

echo "╔════════════════════════════════════════════════════════════╗"
echo "║     Entraînement du Transformer - Take It Easy            ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# 1. Vérifier les données existantes
echo "📊 1. Inspection des données d'entraînement..."
cargo run --release --bin inspect_pt | grep "Nombre d'exemples"
echo ""

# 2. Lancer l'entraînement
echo "🚀 2. Lancement de l'entraînement du Transformer..."
echo "   (Ceci peut prendre 30-60 minutes selon la configuration)"
echo ""
cargo run --release --bin take_it_easy -- \
  --mode transformer-training \
  --offline-training \
  --evaluation-interval 50 \
  2>&1 | tee transformer_training_$(date +%Y%m%d_%H%M%S).log

echo ""
echo "✅ Entraînement terminé!"
echo ""

# 3. Validation rapide
echo "🧪 3. Validation rapide (5 parties)..."
cargo test test_quick_validation \
  --test transformer_validation_quick_test \
  --release -- --ignored --nocapture

echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                   Entraînement terminé!                    ║"
echo "╚════════════════════════════════════════════════════════════╝"

