#!/bin/bash

# Script de lancement complet Take It Easy (Backend + Frontend)
# Usage: ./start_all.sh

set -e

# Couleurs pour les logs
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Gestion du flag --rebuild
if [[ "$1" == "--rebuild" ]]; then
    echo -e "${YELLOW}🧹 Rebuild complet demandé...${NC}"

    # Clean + rebuild backend (à la racine)
    echo -e "${BLUE}🔧 Nettoyage du backend...${NC}"
    cargo clean
    echo -e "${GREEN}✅ Backend nettoyé${NC}"

    echo -e "${BLUE}🔨 Rebuild du backend...${NC}"
    cargo build --release
    echo -e "${GREEN}✅ Backend rebuilded${NC}"

    # Clean + rebuild frontend
    echo -e "${BLUE}🔧 Nettoyage du frontend...${NC}"
    cd frontend
    rm -rf node_modules dist .vite
    echo -e "${GREEN}✅ Frontend nettoyé${NC}"

    echo -e "${BLUE}🔨 Rebuild du frontend...${NC}"
    npm install
    npm run build
    echo -e "${GREEN}✅ Frontend rebuilded${NC}"
    cd ..

    echo -e "${GREEN}🚀 Rebuild complet terminé ! Lancement des services...${NC}"
fi

# Fonction de nettoyage
cleanup() {
    echo -e "\n${YELLOW}🛑 Arrêt en cours...${NC}"
    
    # Arrêter les processus enfants
    if [[ -f .rust_pid ]]; then
        kill "$(cat .rust_pid)" 2>/dev/null || true
        rm -f .rust_pid
    fi
    
    if [[ -f .frontend_pid ]]; then
        kill "$(cat .frontend_pid)" 2>/dev/null || true
        rm -f .frontend_pid
    fi
    
    # Nettoyage des processus par nom
    pkill -f "take_it_easy.*multiplayer" 2>/dev/null || true
    pkill -f "vite.*dev" 2>/dev/null || true
    
    echo -e "${GREEN}✅ Nettoyage terminé${NC}"
    exit 0
}

# Capturer les signaux d'arrêt
trap cleanup SIGINT SIGTERM

echo -e "${BLUE}🎮 Démarrage de Take It Easy - Full Stack${NC}"
echo -e "${BLUE}=======================================${NC}"

# Vérifications préalables
echo -e "${BLUE}🔍 Vérifications...${NC}"

if ! command -v /home/jcgouleau/.cargo/bin/cargo &> /dev/null; then
    echo -e "${RED}❌ Cargo non trouvé dans /home/jcgouleau/.cargo/bin/cargo${NC}"
    exit 1
fi

if ! command -v npm &> /dev/null; then
    echo -e "${RED}❌ npm non trouvé${NC}"
    exit 1
fi

# Nettoyage des anciens processus
echo -e "${YELLOW}🧹 Nettoyage des processus existants...${NC}"
pkill -f "take_it_easy.*multiplayer" 2>/dev/null || true
pkill -f "vite.*dev" 2>/dev/null || true
rm -f .rust_pid .frontend_pid backend.log frontend.log

# Démarrage du backend Rust
echo -e "${GREEN}🦀 Démarrage du backend Rust...${NC}"
echo -e "${BLUE}Port gRPC: 50051 | Port Web: 51051${NC}"

cd "$(dirname "$0")"

# Lancer le backend en arrière-plan
(/home/jcgouleau/.cargo/bin/cargo run --color=always --profile dev -- --mode multiplayer -p 50051 > backend.log 2>&1 & echo $! > .rust_pid) &

# Attendre que le backend soit prêt
echo -e "${YELLOW}⏳ Attente du démarrage du backend...${NC}"
sleep 3

# Vérifier que le backend est lancé
if [[ -f .rust_pid ]] && kill -0 "$(cat .rust_pid)" 2>/dev/null; then
    echo -e "${GREEN}✅ Backend démarré (PID: $(cat .rust_pid))${NC}"
else
    echo -e "${RED}❌ Échec du démarrage du backend${NC}"
    exit 1
fi

# Démarrage du frontend
echo -e "${GREEN}⚛️ Démarrage du frontend SolidJS...${NC}"
cd frontend

# Installation des dépendances si nécessaire
if [[ ! -d "node_modules" ]]; then
    echo -e "${YELLOW}📦 Installation des dépendances npm...${NC}"
    npm install
fi

# Build du frontend
echo -e "${BLUE}🔨 Build du frontend...${NC}"
npm run build

# Lancement du serveur de développement
echo -e "${GREEN}🚀 Lancement du serveur de dev...${NC}"
(npm run dev > ../frontend.log 2>&1 & echo $! > ../.frontend_pid) &

cd ..

# Attendre que le frontend soit prêt
echo -e "${YELLOW}⏳ Attente du démarrage du frontend...${NC}"
sleep 5

# Vérifier que le frontend est lancé
if [[ -f .frontend_pid ]] && kill -0 "$(cat .frontend_pid)" 2>/dev/null; then
    echo -e "${GREEN}✅ Frontend démarré (PID: $(cat .frontend_pid))${NC}"
else
    echo -e "${RED}❌ Échec du démarrage du frontend${NC}"
    cleanup
fi

# Affichage des informations finales
echo -e "\n${GREEN}🎉 Take It Easy est prêt !${NC}"
echo -e "${BLUE}==============================${NC}"
echo -e "${GREEN}🦀 Backend Rust:${NC} http://localhost:51051 (gRPC: 50051)"
echo -e "${GREEN}⚛️ Frontend:${NC}     http://localhost:3000"
echo -e "${BLUE}📋 Logs Backend:${NC}  tail -f backend.log"
echo -e "${BLUE}📋 Logs Frontend:${NC} tail -f frontend.log"
echo -e "\n${YELLOW}Appuyez sur Ctrl+C pour arrêter les deux serveurs${NC}"

# Boucle infinie pour maintenir le script actif
while true; do
    # Vérifier que les processus sont toujours actifs
    if [[ -f .rust_pid ]] && ! kill -0 "$(cat .rust_pid)" 2>/dev/null; then
        echo -e "${RED}❌ Backend arrêté de manière inattendue${NC}"
        cleanup
    fi
    
    if [[ -f .frontend_pid ]] && ! kill -0 "$(cat .frontend_pid)" 2>/dev/null; then
        echo -e "${RED}❌ Frontend arrêté de manière inattendue${NC}"
        cleanup
    fi
    
    sleep 2
done