#!/bin/bash

# start_all.sh - Lancer backend et frontend ensemble
set -e

echo "🚀 Starting Take It Easy - Backend + Frontend"

# Function to kill background processes on exit
cleanup() {
    echo "🛑 Stopping all processes..."
    pkill -f "take_it_easy --mode" 2>/dev/null || true
    pkill -f "npm run dev" 2>/dev/null || true
    exit
}

# Trap to clean up on script exit
trap cleanup EXIT INT TERM

# Build backend (release mode for better performance)
echo "🔧 Building Rust backend..."
cargo build --release

# Build frontend
echo "🔧 Building frontend..."
cd frontend && npm run build && cd ..

echo "✅ Build completed!"

# Start backend in background
echo "🤖 Starting backend (gRPC port 50051)..."
./target/release/take_it_easy --mode multiplayer > backend.log 2>&1 &
BACKEND_PID=$!

# Wait a moment for backend to start
sleep 2

# Start frontend in background
echo "🌐 Starting frontend (http://localhost:3000)..."
cd frontend && npm run dev > ../frontend.log 2>&1 &
FRONTEND_PID=$!
cd ..

echo "✅ All services started!"
echo "📋 Services running:"
echo "   🤖 Backend:  gRPC on port 50051 (PID: $BACKEND_PID)"
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