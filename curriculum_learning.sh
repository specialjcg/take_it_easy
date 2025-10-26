#!/bin/bash
# Curriculum Learning - Entraînement progressif avec données expertes
# Phase 1: Beam 100 (50 parties) ~150 pts
# Phase 2: Beam 500 (100 parties) ~165 pts
# Phase 3: Beam 1000 (200 parties) ~175 pts
# Objectif: +10-15 pts → 149-154 pts

set -e

echo "=========================================="
echo "🎓 Curriculum Learning - Expert Data"
echo "=========================================="
echo ""
echo "Configuration:"
echo "  Phase 1: 50 parties × Beam 100"
echo "  Phase 2: 100 parties × Beam 500"
echo "  Phase 3: 200 parties × Beam 1000"
echo "  Durée estimée: ~3-4 jours"
echo "  Gain attendu: +10-15 pts → 149-154 pts"
echo ""
echo "=========================================="
echo ""

# Créer les dossiers
mkdir -p expert_data
mkdir -p model_weights_curriculum

echo "📊 Phase 1 - Génération données faciles (Beam 100)"
echo "================================================="
echo ""

if [ ! -f expert_data/phase1_beam100.json ]; then
    echo "⏳ Génération de 50 parties avec Beam 100..."
    cargo run --release --bin expert_data_generator -- \
        -g 50 \
        -b 100 \
        -o expert_data/phase1_beam100.json \
        -s 2025
    echo ""
else
    echo "✅ Phase 1 data already exists, skipping generation"
    echo ""
fi

echo "📊 Phase 2 - Génération données moyennes (Beam 500)"
echo "===================================================="
echo ""

if [ ! -f expert_data/phase2_beam500.json ]; then
    echo "⏳ Génération de 100 parties avec Beam 500..."
    cargo run --release --bin expert_data_generator -- \
        -g 100 \
        -b 500 \
        -o expert_data/phase2_beam500.json \
        -s 2026
    echo ""
else
    echo "✅ Phase 2 data already exists, skipping generation"
    echo ""
fi

echo "📊 Phase 3 - Génération données difficiles (Beam 1000)"
echo "======================================================="
echo ""

if [ ! -f expert_data/phase3_beam1000.json ]; then
    echo "⏳ Génération de 200 parties avec Beam 1000..."
    cargo run --release --bin expert_data_generator -- \
        -g 200 \
        -b 1000 \
        -o expert_data/phase3_beam1000.json \
        -s 2027
    echo ""
else
    echo "✅ Phase 3 data already exists, skipping generation"
    echo ""
fi

echo "=========================================="
echo "✅ Génération de données terminée !"
echo "=========================================="
echo ""
echo "Prochaine étape : Implémenter l'entraînement supervisé"
echo "Le code d'entraînement doit être modifié pour :"
echo "  1. Charger les fichiers JSON de données expertes"
echo "  2. Entraîner PolicyNet à prédire les coups experts"
echo "  3. Entraîner ValueNet à prédire les scores finaux"
echo ""
echo "Fichiers générés :"
ls -lh expert_data/*.json 2>/dev/null || echo "Aucun fichier trouvé"
echo ""
