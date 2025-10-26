#!/bin/bash
# Script de monitoring de l'entraînement Gold GNN

echo "📊 Monitoring Gold GNN Training"
echo "================================"
echo ""

# Vérifier si l'entraînement tourne
if pgrep -f "take_it_easy.*training" > /dev/null; then
    echo "✅ Entraînement en cours"
else
    echo "❌ Entraînement arrêté"
fi

echo ""
echo "📈 Dernières lignes du log:"
echo "----------------------------"
tail -30 training_gold_gnn_500games.log 2>/dev/null || echo "Fichier de log non trouvé"

echo ""
echo "💾 Poids du modèle:"
echo "-------------------"
ls -lh model_weights/*.pt 2>/dev/null || echo "Pas de poids trouvés"

echo ""
echo "⏱️  Temps écoulé depuis le démarrage:"
echo "------------------------------------"
ps -p $(pgrep -f "take_it_easy.*training" | head -1) -o etime= 2>/dev/null || echo "Process non trouvé"
