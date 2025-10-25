#!/bin/bash
# Monitoring script for Pattern Rollouts benchmark
LOG_FILE="pattern_rollouts_benchmark.log"

echo "=== Pattern Rollouts Benchmark Monitor ==="
echo "Démarré à: $(date)"
echo ""

while true; do
    clear
    echo "=== Monitoring Pattern Rollouts Benchmark ==="
    echo "Heure actuelle: $(date '+%H:%M:%S')"
    echo ""

    # Check if process is still running
    if pgrep -f "compare_mcts.*-g 50" > /dev/null; then
        RUNTIME=$(ps -o etime= -p $(pgrep -f "compare_mcts.*-g 50" | tail -1) | tr -d ' ')
        CPU=$(ps -o %cpu= -p $(pgrep -f "compare_mcts.*-g 50" | tail -1) | tr -d ' ')
        echo "✅ Processus actif | Durée: $RUNTIME | CPU: $CPU%"
    else
        echo "❌ Processus terminé"
        echo ""
        echo "=== RÉSULTATS FINAUX ==="
        tail -30 "$LOG_FILE"
        break
    fi

    echo ""
    echo "=== Dernières lignes du log (30 dernières) ==="
    tail -30 "$LOG_FILE"

    echo ""
    echo "=== Nombre de parties terminées ==="
    GAMES_DONE=$(grep -c "Game [0-9]" "$LOG_FILE" 2>/dev/null || echo "0")
    echo "Parties: $GAMES_DONE / 50"

    # Check if benchmark is complete
    if grep -q "Average score" "$LOG_FILE" 2>/dev/null; then
        echo ""
        echo "✅ BENCHMARK TERMINÉ !"
        echo ""
        echo "=== RÉSULTATS ==="
        grep -A 5 "Average score" "$LOG_FILE"
        break
    fi

    sleep 60  # Refresh every minute
done
