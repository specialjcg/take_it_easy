#!/bin/bash

# Script d'arrêt Take It Easy
# Usage: ./stop_all.sh

# Couleurs pour les logs
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🛑 Arrêt de Take It Easy...${NC}"

cd "$(dirname "$0")"

# Arrêter via les PIDs sauvegardés
if [[ -f .rust_pid ]]; then
    echo -e "${BLUE}🦀 Arrêt du backend Rust...${NC}"
    kill $(cat .rust_pid) 2>/dev/null || true
    rm -f .rust_pid
    echo -e "${GREEN}✅ Backend arrêté${NC}"
fi

if [[ -f .frontend_pid ]]; then
    echo -e "${BLUE}⚛️ Arrêt du frontend...${NC}"
    kill $(cat .frontend_pid) 2>/dev/null || true
    rm -f .frontend_pid
    echo -e "${GREEN}✅ Frontend arrêté${NC}"
fi

# Nettoyage complet par nom de processus
echo -e "${YELLOW}🧹 Nettoyage des processus restants...${NC}"
pkill -f "take_it_easy.*multiplayer" 2>/dev/null || true
pkill -f "vite.*dev" 2>/dev/null || true

# Nettoyage des fichiers de logs
rm -f backend.log frontend.log

echo -e "${GREEN}🎉 Take It Easy complètement arrêté !${NC}"