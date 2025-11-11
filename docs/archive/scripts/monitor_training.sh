#!/bin/bash
# Monitor Silver GNN training progress

LOG_FILE="training_silver_gnn_from_scratch.log"

echo "=== Silver GNN Training Monitor ==="
echo "Log file: $LOG_FILE"
echo ""

while true; do
    clear
    echo "=== Silver GNN Training Monitor (auto-refresh every 30s) ==="
    echo "Time: $(date '+%H:%M:%S')"
    echo ""

    # Extract progress lines
    echo "📊 Recent Progress:"
    tail -100 "$LOG_FILE" | grep -E "Progress [0-9]+/1000" | tail -10
    echo ""

    # Extract game results
    echo "🎮 Recent Scores:"
    tail -100 "$LOG_FILE" | grep "GAME_RESULT" | tail -5
    echo ""

    # Extract checkpoint summaries
    echo "📈 Checkpoint Summaries:"
    tail -200 "$LOG_FILE" | grep -A5 "Average Scores by First Position" | tail -10
    echo ""

    # Check if training is complete
    if grep -q "Progress 1000/1000" "$LOG_FILE"; then
        echo "✅ Training complete!"
        echo ""
        echo "Final summary:"
        tail -50 "$LOG_FILE" | grep -A10 "Average Scores by First Position" | tail -15
        break
    fi

    sleep 30
done
