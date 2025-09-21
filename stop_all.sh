#!/bin/bash

# stop_all.sh - Arrêter tous les services Take It Easy
echo "🛑 Stopping Take It Easy services..."

# Stop backend
echo "🤖 Stopping backend..."
pkill -f "take_it_easy --mode" 2>/dev/null || echo "   No backend process found"

# Stop frontend
echo "🌐 Stopping frontend..."
pkill -f "npm run dev" 2>/dev/null || echo "   No frontend process found"

# Clean up any remaining node processes
pkill -f "vite" 2>/dev/null || true

# Clean up ports
echo "🔧 Cleaning up ports..."
lsof -ti:3000,3001,50051 | xargs kill -9 2>/dev/null || true

echo "✅ All services stopped!"

# Show status
echo "📋 Port status:"
netstat -tulpn 2>/dev/null | grep -E ":300[01]|:50051" || echo "   All ports free"
