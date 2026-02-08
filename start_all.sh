#!/bin/bash

# start_all.sh - Lancer backend et frontend ensemble
set -e

# Add protoc to PATH
export PATH="$HOME/.local/bin:$PATH"

echo "🚀 Starting Take It Easy - Backend + Frontend (Elm)"

# Function to kill background processes on exit
cleanup() {
    echo "🛑 Stopping all processes..."
    pkill -f "take_it_easy --mode" 2>/dev/null || true
    pkill -f "python3 -m http.server" 2>/dev/null || true
    exit
}

# Trap to clean up on script exit
trap cleanup EXIT INT TERM

# Build backend (release mode for better performance)
echo "🔧 Building Rust backend..."
cargo build --release

# Build frontend Elm
GRPC_PORT=50051
GRPC_WEB_PORT=$((GRPC_PORT + 1))

echo "🔧 Building Elm frontend..."
cd frontend-elm
elm make src/Main.elm --optimize --output=public/main.js
npm run build:grpc
cd ..

echo "✅ Build completed!"

# Start backend in background
echo "🤖 Starting backend (gRPC port ${GRPC_PORT}, gRPC-Web port ${GRPC_WEB_PORT})..."
echo "   Architecture: Graph Transformer Direct (149.38 pts, sans MCTS)"
./target/release/take_it_easy --mode multiplayer --port ${GRPC_PORT} --nn-architecture graph-transformer > backend.log 2>&1 &
BACKEND_PID=$!

# Wait a moment for backend to start
sleep 2

# Start frontend in background (Python HTTP server)
echo "🌐 Starting Elm frontend (http://localhost:3000)..."
(cd frontend-elm/public && python3 -m http.server 3000 > ../../frontend.log 2>&1) &
FRONTEND_PID=$!

echo "✅ All services started!"
echo "📋 Services running:"
echo "   🤖 Backend:  gRPC on port ${GRPC_PORT} (gRPC-Web ${GRPC_WEB_PORT}) (PID: $BACKEND_PID)"
echo "   🌐 Frontend: http://localhost:3000 (PID: $FRONTEND_PID)"
echo ""
echo "📝 Logs:"
echo "   Backend:  tail -f backend.log"
echo "   Frontend: tail -f frontend.log"
echo ""
echo "🛑 Press Ctrl+C to stop all services"

# Keep script running and monitor processes
while true; do
    # Check if processes are still running
    if ! kill -0 $BACKEND_PID 2>/dev/null; then
        echo "❌ Backend crashed! Check backend.log"
        exit 1
    fi

    if ! kill -0 $FRONTEND_PID 2>/dev/null; then
        echo "❌ Frontend crashed! Check frontend.log"
        exit 1
    fi

    sleep 5
done
