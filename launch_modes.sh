#!/bin/bash

# launch_modes.sh - Script unifié pour lancer Take It Easy (Backend + Frontend)

# Couleurs pour les logs
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}🎮 Take It Easy - Lanceur Unifié${NC}"
echo -e "${BLUE}=================================${NC}"

# Fonction de nettoyage complet
cleanup() {
    echo -e "\n${YELLOW}🛑 Arrêt en cours...${NC}"
    
    # Arrêter backend
    if [[ -f .rust_pid ]]; then
        local pid=$(cat .rust_pid)
        if ps -p $pid > /dev/null 2>&1; then
            echo -e "${YELLOW}🛑 Arrêt du backend (PID: $pid)...${NC}"
            kill $pid 2>/dev/null || true
            sleep 2
        fi
        rm -f .rust_pid
    fi
    
    # Arrêter frontend
    if [[ -f .frontend_pid ]]; then
        local pid=$(cat .frontend_pid)
        if ps -p $pid > /dev/null 2>&1; then
            echo -e "${YELLOW}🛑 Arrêt du frontend (PID: $pid)...${NC}"
            kill $pid 2>/dev/null || true
        fi
        rm -f .frontend_pid
    fi
    
    # Nettoyage par nom de processus
    pkill -f "take_it_easy.*" 2>/dev/null || true
    pkill -f "vite.*dev" 2>/dev/null || true
    
    echo -e "${GREEN}✅ Nettoyage terminé${NC}"
    exit 0
}

# Capturer les signaux d'arrêt
trap cleanup SIGINT SIGTERM

# Fonction pour arrêter seulement le backend
stop_backend() {
    if [ -f .rust_pid ]; then
        local pid=$(cat .rust_pid)
        if ps -p $pid > /dev/null 2>&1; then
            echo -e "${YELLOW}🛑 Arrêt du backend actuel (PID: $pid)...${NC}"
            kill $pid
            sleep 2
        fi
        rm -f .rust_pid
    fi
}

# Fonction pour démarrer le frontend
start_frontend() {
    local should_start="$1"
    
    if [[ "$should_start" != "true" ]]; then
        return 0
    fi
    
    echo -e "${GREEN}⚛️ Démarrage du frontend...${NC}"
    
    if [[ ! -d "frontend" ]]; then
        echo -e "${YELLOW}⚠️ Dossier frontend introuvable, backend seul${NC}"
        return 0
    fi
    
    cd frontend
    
    # Installation si nécessaire
    if [[ ! -d "node_modules" ]]; then
        echo -e "${YELLOW}📦 Installation npm...${NC}"
        npm install
    fi
    
    # Build et dev en arrière-plan
    echo -e "${BLUE}🔨 Build frontend...${NC}"
    npm run build
    
    echo -e "${GREEN}🚀 Lancement serveur dev...${NC}"
    (npm run dev > ../frontend.log 2>&1 & echo $! > ../.frontend_pid) &
    
    cd ..
    sleep 3
    
    if [[ -f .frontend_pid ]] && kill -0 "$(cat .frontend_pid)" 2>/dev/null; then
        echo -e "${GREEN}✅ Frontend démarré (PID: $(cat .frontend_pid))${NC}"
        echo -e "${GREEN}⚛️ Frontend: http://localhost:3000${NC}"
    else
        echo -e "${YELLOW}⚠️ Frontend non démarré${NC}"
    fi
}

# Fonction pour lancer le backend
start_backend() {
    local cmd="$1"
    local with_frontend="$2"
    
    echo -e "${GREEN}🚀 Lancement: $cmd${NC}"
    ($cmd > backend.log 2>&1 & echo $! > .rust_pid) &
    
    local pid=$(cat .rust_pid)
    echo -e "${GREEN}✅ Backend démarré (PID: $pid)${NC}"
    
    # Attendre que le serveur démarre
    echo -e "${YELLOW}⏳ Attente du démarrage...${NC}"
    sleep 3
    
    if ps -p $pid > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Serveur opérationnel${NC}"
        if lsof -i :50051 > /dev/null 2>&1; then
            echo -e "${GREEN}🌐 gRPC: http://localhost:50051${NC}"
        fi
        if lsof -i :51051 > /dev/null 2>&1; then
            echo -e "${GREEN}🎯 Interface: http://localhost:51051${NC}"
        fi
        
        # Démarrer le frontend si demandé
        start_frontend "$with_frontend"
        
        # Si frontend lancé, mode monitoring
        if [[ "$with_frontend" == "true" && -f .frontend_pid ]]; then
            echo -e "\n${GREEN}🎉 Take It Easy complet prêt !${NC}"
            echo -e "${BLUE}📋 Logs: tail -f backend.log frontend.log${NC}"
            echo -e "${YELLOW}Ctrl+C pour arrêter tout${NC}\n"
            
            # Monitoring loop
            while true; do
                if [[ -f .rust_pid ]] && ! kill -0 "$(cat .rust_pid)" 2>/dev/null; then
                    echo -e "${RED}❌ Backend arrêté${NC}"
                    cleanup
                fi
                if [[ -f .frontend_pid ]] && ! kill -0 "$(cat .frontend_pid)" 2>/dev/null; then
                    echo -e "${RED}❌ Frontend arrêté${NC}"
                    cleanup
                fi
                sleep 2
            done
        fi
    else
        echo -e "${RED}❌ Échec du démarrage${NC}"
        rm -f .rust_pid
    fi
}

# Fonction pour analyser les options
parse_options() {
    WITH_FRONTEND="true"  # Frontend par défaut
    REBUILD="false"
    
    for arg in "$@"; do
        case $arg in
            --no-frontend|--backend-only|-b)
                WITH_FRONTEND="false"
                ;;
            --rebuild|-r)
                REBUILD="true"
                ;;
        esac
    done
}

# Fonction de rebuild
rebuild_if_needed() {
    if [[ "$REBUILD" == "true" ]]; then
        echo -e "${YELLOW}🧹 Rebuild demandé...${NC}"
        
        echo -e "${BLUE}🔧 Nettoyage backend...${NC}"
        cargo clean
        cargo build --release
        echo -e "${GREEN}✅ Backend rebuilded${NC}"
        
        if [[ -d "frontend" ]]; then
            echo -e "${BLUE}🔧 Nettoyage frontend...${NC}"
            cd frontend
            rm -rf node_modules dist .vite
            npm install
            npm run build
            cd ..
            echo -e "${GREEN}✅ Frontend rebuilded${NC}"
        fi
    fi
}

# Parser les options globales
parse_options "$@"

case "$1" in
    "single"|"1v1"|"solo")
        echo -e "${BLUE}🤖 Mode: UN JOUEUR vs MCTS${NC}"
        rebuild_if_needed
        stop_backend
        start_backend "cargo run --release -- --single-player --num-simulations 300" "$WITH_FRONTEND"
        ;;
        
    "multi"|"multiplayer"|"")
        echo -e "${BLUE}👥 Mode: MULTIJOUEUR + MCTS${NC}"
        rebuild_if_needed
        stop_backend  
        start_backend "cargo run --release -- --mode multiplayer --num-simulations ${2:-150}" "$WITH_FRONTEND"
        ;;
        
    "training"|"train")
        echo -e "${BLUE}🎓 Mode: ENTRAÎNEMENT${NC}"
        rebuild_if_needed
        stop_backend
        start_backend "cargo run --release -- --mode training --num-games ${2:-100}" "$WITH_FRONTEND"
        ;;
        
    "strong"|"fort")
        echo -e "${BLUE}🥊 Mode: UN JOUEUR vs MCTS FORT${NC}"
        rebuild_if_needed
        stop_backend
        start_backend "cargo run --release -- --single-player --num-simulations 1000" "$WITH_FRONTEND"
        ;;
        
    "fast"|"rapide") 
        echo -e "${BLUE}⚡ Mode: UN JOUEUR vs MCTS RAPIDE${NC}"
        rebuild_if_needed
        stop_backend
        start_backend "cargo run --release -- --single-player --num-simulations 50" "$WITH_FRONTEND"
        ;;
        
    "stop"|"kill")
        echo -e "${YELLOW}🛑 Arrêt complet${NC}"
        cleanup
        ;;
        
    "status"|"info")
        echo -e "${BLUE}📊 Statut actuel:${NC}"
        if [ -f .rust_pid ]; then
            pid=$(cat .rust_pid)
            if ps -p $pid > /dev/null 2>&1; then
                echo -e "${GREEN}✅ Backend actif (PID: $pid)${NC}"
            else
                echo -e "${RED}❌ Backend arrêté${NC}"
                rm -f .rust_pid
            fi
        else
            echo -e "${RED}❌ Aucun backend en cours${NC}"
        fi
        
        if [ -f .frontend_pid ]; then
            pid=$(cat .frontend_pid)
            if ps -p $pid > /dev/null 2>&1; then
                echo -e "${GREEN}✅ Frontend actif (PID: $pid)${NC}"
            else
                echo -e "${RED}❌ Frontend arrêté${NC}"
                rm -f .frontend_pid
            fi
        else
            echo -e "${RED}❌ Aucun frontend en cours${NC}"
        fi
        
        echo -e "${BLUE}🔗 Ports:${NC}"
        netstat -tln 2>/dev/null | grep -E ":5005[12]|:3000" || echo "Aucun port actif"
        ;;
        
    *)
        echo -e "${GREEN}Usage: $0 [MODE] [SIMULATIONS] [OPTIONS]${NC}"
        echo ""
        echo -e "${BLUE}Modes disponibles:${NC}"
        echo "  single|1v1|solo    - 1 joueur vs MCTS (défaut: 300 simulations)"
        echo "  multi|multiplayer  - Multijoueur + MCTS (défaut: 150 simulations)"  
        echo "  strong|fort        - 1 joueur vs MCTS FORT (1000 simulations)"
        echo "  fast|rapide        - 1 joueur vs MCTS RAPIDE (50 simulations)"
        echo "  training|train     - Mode entraînement (défaut: 100 parties)"
        echo ""
        echo -e "${BLUE}Options globales:${NC}"
        echo "  --no-frontend, --backend-only, -b  - Backend seul (sans frontend)"
        echo "  --rebuild, -r                      - Rebuild complet avant lancement"
        echo ""
        echo -e "${BLUE}Utilitaires:${NC}"
        echo "  stop|kill          - Arrêter tout (backend + frontend)"
        echo "  status|info        - Voir le statut complet"
        echo ""
        echo -e "${BLUE}Exemples:${NC}"
        echo "  $0 single 500      # 1v1 avec MCTS à 500 simulations + frontend"
        echo "  $0 multi --backend-only  # Multijoueur backend seul"
        echo "  $0 strong --rebuild      # MCTS fort + rebuild complet"
        echo "  $0 training -b           # Entraînement backend seul"
        echo "  $0 status               # Voir le statut"
        ;;
esac