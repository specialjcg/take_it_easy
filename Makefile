# Makefile for Take It Easy - Full Stack Game

.PHONY: help dev start stop build clean backend frontend logs

# Default target
help:
	@echo "🎮 Take It Easy - Available commands:"
	@echo ""
	@echo "Development:"
	@echo "  make dev     - Start both backend + frontend (dev mode)"
	@echo "  make start   - Start both backend + frontend (production build)"
	@echo "  make stop    - Stop all services"
	@echo ""
	@echo "Individual services:"
	@echo "  make backend - Start only backend"
	@echo "  make frontend- Start only frontend"
	@echo ""
	@echo "Build & maintenance:"
	@echo "  make build   - Build backend + frontend"
	@echo "  make clean   - Clean all build artifacts"
	@echo "  make logs    - Show logs in real-time"

# Development mode (quick start)
dev:
	@echo "🚀 Starting development mode..."
	./dev_start.sh

# Production mode (with builds)
start:
	@echo "🚀 Starting production mode..."
	./start_all.sh

# Stop all services
stop:
	@echo "🛑 Stopping all services..."
	./stop_all.sh

# Build everything
build:
	@echo "🔧 Building backend..."
	cargo build --release
	@echo "🔧 Building frontend..."
	cd frontend && npm run build
	@echo "✅ Build complete!"

# Clean everything
clean:
	@echo "🧹 Cleaning..."
	cargo clean
	cd frontend && rm -rf dist node_modules/.vite
	@echo "✅ Clean complete!"

# Start only backend
backend:
	@echo "🤖 Starting backend only..."
	cargo run -- --mode multiplayer

# Start only frontend
frontend:
	@echo "🌐 Starting frontend only..."
	cd frontend && npm run dev

# Show logs
logs:
	@echo "📝 Showing logs (Ctrl+C to exit)..."
	@if [ -f backend.log ] && [ -f frontend.log ]; then \
		tail -f backend.log frontend.log; \
	elif [ -f backend.log ]; then \
		tail -f backend.log; \
	elif [ -f frontend.log ]; then \
		tail -f frontend.log; \
	else \
		echo "No log files found. Start services first."; \
	fi