#!/bin/bash

# Script de nettoyage avant restart de l'assistant
echo "🧹 Nettoyage environnement Take It Easy..."

# Arrêter tous les processus
echo "🛑 Arrêt des processus..."
pkill -f "take_it_easy" 2>/dev/null || true
pkill -f "vite" 2>/dev/null || true
pkill -f "launch_modes" 2>/dev/null || true
pkill -f "launch.sh" 2>/dev/null || true
pkill -f "rust_web_start" 2>/dev/null || true
pkill -f "web_start" 2>/dev/null || true

# Supprimer les fichiers PID
echo "🗑️ Suppression fichiers PID..."
rm -f .rust_pid .frontend_pid

# Vérifier les ports
echo "🔍 Vérification ports..."
lsof -ti:50051 | xargs kill -9 2>/dev/null || true
lsof -ti:3000 | xargs kill -9 2>/dev/null || true
lsof -ti:3001 | xargs kill -9 2>/dev/null || true
lsof -ti:51051 | xargs kill -9 2>/dev/null || true

# Attendre un peu
sleep 2

# Status final
echo "✅ Nettoyage terminé"
echo "📊 Processus restants:"
ps aux | grep -E "(take_it_easy|vite|launch)" | grep -v grep || echo "Aucun processus trouvé"

echo "🌐 Ports libérés:"
netstat -tlnp | grep -E ":3000|:3001|:50051|:51051" || echo "Aucun port occupé"

echo ""
echo "🚀 Prêt pour relancer l'assistant!"
echo "📋 Context sauvé dans: SESSION_CONTEXT.md"
