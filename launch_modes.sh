#!/bin/bash

# launch_modes.sh - Script pour lancer facilement les différents modes

echo "🎮 Take It Easy - Lanceur de Modes"
echo "=================================="

# Fonction pour arrêter le backend actuel
stop_backend() {
    if [ -f .rust_pid ]; then
        local pid=$(cat .rust_pid)
        if ps -p $pid > /dev/null 2>&1; then
            echo "🛑 Arrêt du backend actuel (PID: $pid)..."
            kill $pid
            sleep 2
        fi
        rm -f .rust_pid
    fi
}

# Fonction pour lancer et enregistrer le PID
start_backend() {
    local cmd="$1"
    echo "🚀 Lancement: $cmd"
    $cmd &
    local pid=$!
    echo $pid > .rust_pid
    echo "✅ Backend démarré (PID: $pid)"
    
    # Attendre que le serveur démarre
    echo "⏳ Attente du démarrage du serveur..."
    sleep 3
    
    if ps -p $pid > /dev/null 2>&1; then
        echo "✅ Serveur opérationnel"
        if lsof -i :50051 > /dev/null 2>&1; then
            echo "🌐 gRPC: http://localhost:50051"
        fi
        if lsof -i :51051 > /dev/null 2>&1; then
            echo "🎯 Interface: http://localhost:51051"
        fi
    else
        echo "❌ Échec du démarrage"
        rm -f .rust_pid
    fi
}

case "$1" in
    "single"|"1v1"|"solo")
        echo "🤖 Mode: UN JOUEUR vs MCTS"
        stop_backend
        start_backend "cargo run --release -- --single-player --num-simulations ${2:-300}"
        ;;
        
    "multi"|"multiplayer"|"")
        echo "👥 Mode: MULTIJOUEUR + MCTS"
        stop_backend  
        start_backend "cargo run --release -- --mode multiplayer --num-simulations ${2:-150}"
        ;;
        
    "training"|"train")
        echo "🎓 Mode: ENTRAÎNEMENT"
        stop_backend
        start_backend "cargo run --release -- --mode training --num-games ${2:-100}"
        ;;
        
    "strong"|"fort")
        echo "🥊 Mode: UN JOUEUR vs MCTS FORT"
        stop_backend
        start_backend "cargo run --release -- --single-player --num-simulations 1000"
        ;;
        
    "fast"|"rapide") 
        echo "⚡ Mode: UN JOUEUR vs MCTS RAPIDE"
        stop_backend
        start_backend "cargo run --release -- --single-player --num-simulations 50"
        ;;
        
    "stop"|"kill")
        echo "🛑 Arrêt du backend"
        stop_backend
        ;;
        
    "status"|"info")
        echo "📊 Statut actuel:"
        if [ -f .rust_pid ]; then
            local pid=$(cat .rust_pid)
            if ps -p $pid > /dev/null 2>&1; then
                echo "✅ Backend actif (PID: $pid)"
                echo "🔗 Ports ouverts:"
                netstat -tln 2>/dev/null | grep -E ":5005[12]"
            else
                echo "❌ Backend arrêté"
                rm -f .rust_pid
            fi
        else
            echo "❌ Aucun backend en cours"
        fi
        ;;
        
    *)
        echo "Usage: $0 [MODE] [SIMULATIONS]"
        echo ""
        echo "Modes disponibles:"
        echo "  single|1v1|solo    - 1 joueur vs MCTS (défaut: 300 simulations)"
        echo "  multi|multiplayer  - Multijoueur + MCTS (défaut: 150 simulations)"  
        echo "  strong|fort        - 1 joueur vs MCTS FORT (1000 simulations)"
        echo "  fast|rapide        - 1 joueur vs MCTS RAPIDE (50 simulations)"
        echo "  training|train     - Mode entraînement (défaut: 100 parties)"
        echo ""
        echo "Utilitaires:"
        echo "  stop|kill          - Arrêter le backend"
        echo "  status|info        - Voir le statut actuel"
        echo ""
        echo "Exemples:"
        echo "  $0 single 500      # 1v1 avec MCTS à 500 simulations"
        echo "  $0 multi 200       # Multijoueur avec MCTS à 200 simulations"
        echo "  $0 strong          # 1v1 contre MCTS très fort"
        echo "  $0 status          # Voir le statut"
        ;;
esac