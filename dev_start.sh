#!/bin/bash

# dev_start.sh - Version développement rapide (pas de build release)
set -e

echo "🚀 Starting Take It Easy - DEV MODE"

# Function to kill background processes on exit
cleanup() {
    echo "🛑 Stopping all processes..."
    pkill -f "take_it_easy --mode" 2>/dev/null || true
    pkill -f "npm run dev" 2>/dev/null || true
    exit
}

trap cleanup EXIT INT TERM

# Quick check build (dev mode)
echo "🔧 Checking Rust backend..."
cargo check

echo "✅ Backend ready!"

# Start backend in background
echo "🤖 Starting backend (gRPC port 50051)..."
cargo run -- --mode multiplayer > backend.log 2>&1 &
BACKEND_PID=$!

# Wait for backend
sleep 3

# Start frontend in background
echo "🌐 Starting frontend..."
cd frontend && npm run dev > ../frontend.log 2>&1 &
FRONTEND_PID=$!
cd ..

echo "✅ All services started!"
echo "📋 Services:"
echo "   🤖 Backend:  gRPC on port 50051"
echo "   🌐 Frontend: http://localhost:3000"
echo ""
echo "🛑 Press Ctrl+C to stop all"

# Wait and monitor
while true; do
    sleep 5
done