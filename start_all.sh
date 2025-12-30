#!/bin/bash

# start_all.sh - Lancer backend et frontend ensemble
set -e

# Add protoc to PATH
export PATH="$HOME/.local/bin:$PATH"

# Load NVM and use compatible Node.js version
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
# Ensure we have Node.js 22.12.0 installed and use it
if ! nvm ls 22.12.0 > /dev/null 2>&1; then
    echo "📦 Installing Node.js v22.12.0..."
    nvm install 22.12.0
fi
nvm use 22.12.0 > /dev/null 2>&1 || {
    echo "❌ Failed to activate Node.js v22.12.0. Please install NVM and Node.js v22.12.0"
    exit 1
}

echo "🚀 Starting Take It Easy - Backend + Frontend"
echo "📦 Using Node.js version: $(node --version)"

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
GRPC_PORT=50051
GRPC_WEB_PORT=$((GRPC_PORT + 1))

# Install frontend dependencies if needed
if [ ! -d "frontend/node_modules" ]; then
    echo "📦 Installing frontend dependencies..."
    cd frontend && npm install && cd ..
fi

echo "🔧 Building frontend..."
cd frontend && VITE_GRPC_WEB_BASE_URL="http://localhost:${GRPC_WEB_PORT}" npm run build && cd ..

echo "✅ Build completed!"

# Start backend in background
echo "🤖 Starting backend (gRPC port ${GRPC_PORT}, gRPC-Web port ${GRPC_WEB_PORT})..."
./target/release/take_it_easy --mode multiplayer --port ${GRPC_PORT} > backend.log 2>&1 &
BACKEND_PID=$!

# Wait a moment for backend to start
sleep 2

# Start frontend in background (from project root)
echo "🌐 Starting frontend (http://localhost:3000)..."
(cd frontend && VITE_GRPC_WEB_BASE_URL="http://localhost:${GRPC_WEB_PORT}" npm run dev > ../frontend.log 2>&1) &
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
