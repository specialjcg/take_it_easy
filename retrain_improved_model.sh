#!/bin/bash

# Script d'entraînement du Transformer amélioré
# Génère de nouvelles données MCTS puis entraîne le Transformer

set -e  # Arrêter en cas d'erreur

echo "=========================================="
echo "🚀 ENTRAÎNEMENT DU TRANSFORMER AMÉLIORÉ"
echo "=========================================="
echo ""

# Configuration
NUM_GAMES=5000
NUM_SIMULATIONS=150
EVALUATION_INTERVAL=50

# Couleurs pour les logs
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Fonction pour afficher les étapes
step() {
    echo -e "${BLUE}➤ $1${NC}"
}

# Fonction pour afficher les succès
success() {
    echo -e "${GREEN}✓ $1${NC}"
}

# Fonction pour afficher les warnings
warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

# Fonction pour afficher les erreurs
error() {
    echo -e "${RED}✗ $1${NC}"
}

# Vérifier que nous sommes dans le bon répertoire
if [ ! -f "Cargo.toml" ]; then
    error "Cargo.toml introuvable. Exécutez ce script depuis la racine du projet."
    exit 1
fi

success "Répertoire du projet validé"

# Étape 0: Backup des anciennes données
step "Étape 0: Backup des données existantes..."
BACKUP_DIR="backup_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"

if ls game_data*.pt 1> /dev/null 2>&1; then
    mv game_data*.pt "$BACKUP_DIR/" 2>/dev/null || true
    success "Anciennes données sauvegardées dans $BACKUP_DIR/"
else
    warning "Aucune donnée existante à sauvegarder"
fi

if [ -d "transformer_weights" ]; then
    cp -r transformer_weights "$BACKUP_DIR/transformer_weights_old" 2>/dev/null || true
    success "Anciens poids du Transformer sauvegardés"
fi

echo ""

# Étape 1: Build du projet
step "Étape 1: Compilation en mode release..."
cargo build --release

if [ $? -eq 0 ]; then
    success "Compilation réussie"
else
    error "Échec de la compilation"
    exit 1
fi

echo ""

# Étape 2: Générer des données MCTS
step "Étape 2: Génération de ${NUM_GAMES} parties MCTS..."
echo "   Simulations par état: ${NUM_SIMULATIONS}"
echo "   Intervalle d'évaluation: ${EVALUATION_INTERVAL}"
echo ""

cargo run --release -- \
    --mode training \
    --offline-training \
    --num-games "$NUM_GAMES" \
    --num-simulations "$NUM_SIMULATIONS" \
    --evaluation-interval "$EVALUATION_INTERVAL" \
    2>&1 | tee mcts_generation.log

if [ $? -eq 0 ]; then
    success "Génération de données MCTS terminée"
else
    error "Échec de la génération de données MCTS"
    exit 1
fi

echo ""

# Étape 3: Vérifier les données générées
step "Étape 3: Vérification des données générées..."

if ls game_data*.pt 1> /dev/null 2>&1; then
    echo "   Fichiers de données détectés:"
    ls -lh game_data*.pt | awk '{print "     - " $9 " (" $5 ")"}'
    success "Données MCTS disponibles"
else
    error "Aucun fichier de données généré"
    exit 1
fi

echo ""

# Étape 4: Entraîner le Transformer
step "Étape 4: Entraînement du Transformer avec les nouvelles features..."
echo "   Architecture: 4 couches, 128 dim, 4 têtes, 512 FF"
echo "   Features: 256 (vs 64 avant)"
echo ""

cargo run --release -- \
    --mode transformer-training \
    --offline-training \
    --evaluation-interval "$EVALUATION_INTERVAL" \
    --num-games "$NUM_GAMES" \
    2>&1 | tee transformer_training.log

if [ $? -eq 0 ]; then
    success "Entraînement du Transformer terminé"
else
    error "Échec de l'entraînement du Transformer"
    exit 1
fi

echo ""

# Étape 5: Résumé
step "Étape 5: Résumé de l'entraînement"
echo ""
echo "=========================================="
echo "📊 RÉSULTATS"
echo "=========================================="

if [ -f "transformer_training.log" ]; then
    echo ""
    echo "🎯 Dernières métriques d'entraînement:"
    echo ""
    tail -20 transformer_training.log | grep -E "(Loss|Score|Epoch)" || echo "Pas de métriques détectées"
    echo ""
fi

if [ -d "transformer_weights" ]; then
    echo "💾 Poids sauvegardés:"
    ls -lh transformer_weights/ | tail -n +2 | awk '{print "   - " $9 " (" $5 ")"}'
    echo ""
fi

echo "📁 Logs disponibles:"
echo "   - mcts_generation.log: Génération des données MCTS"
echo "   - transformer_training.log: Entraînement du Transformer"
echo ""

if [ -d "$BACKUP_DIR" ]; then
    echo "💼 Backup des anciennes données: $BACKUP_DIR/"
    echo ""
fi

success "ENTRAÎNEMENT TERMINÉ !"
echo ""
echo "🎯 Prochaines étapes recommandées:"
echo "   1. Vérifier les scores dans transformer_training.log"
echo "   2. Comparer avec la baseline (14.55 → objectif >100 points)"
echo "   3. Si nécessaire, ajuster les hyperparamètres"
echo ""
echo "📖 Documentation:"
echo "   - TRANSFORMER_DIAGNOSIS.md: Diagnostic complet"
echo "   - IMPROVEMENTS_SUMMARY.md: Résumé des améliorations"
echo ""
