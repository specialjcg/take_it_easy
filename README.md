# Take It Easy - Multiplayer Game with MCTS AI

A tile-placement strategy game with real-time multiplayer support and advanced MCTS AI powered by neural networks.

## Features

- **Real-time Multiplayer:** gRPC-based multiplayer sessions with automatic session management
- **MCTS AI Integration:** Advanced Monte Carlo Tree Search with neural network guidance  
- **Single-player Mode:** Play against AI with automatic session creation and flow
- **Independent Gameplay:** Players can move at their own pace, no turn-based waiting
- **Auto-progression:** Automatic tile drawing and turn advancement when all players finish
- **Web Interface:** Modern SolidJS frontend with real-time game state updates
- **Performance Optimized:** Async architecture with optimized session lookups and caching

## Quick Start

### Single-player vs AI
```bash
cargo run -- --single-player --num-simulations 300
```
- Access at: http://localhost:51051
- Auto-connects to game session
- MCTS plays automatically when tiles are drawn

### Multiplayer 
```bash
cargo run -- --mode multiplayer --port 50051
```
- Players join with session codes
- MCTS participates as additional player
- Independent player progression

### Training Mode
```bash  
cargo run -- --mode training --num-games 500
```
- Neural network training via self-play
- WebSocket monitoring on port 9000

## Architecture

### Core Services
- **SessionManager:** Functional session state management with immutable operations
- **GameService:** gRPC service handling gameplay, moves, and state queries
- **GameManager:** Pure functions for game logic, MCTS integration, and state transitions
- **MCTS Algorithm:** Neural network-guided tree search for optimal moves

### Key Optimizations
- **Async Mutex:** tokio::sync::Mutex for better async performance
- **Session Lookup Cache:** Optimized UUID vs code detection
- **Image Generation Cache:** Static caching for tile image names  
- **JSON Optimization:** Reduced recreation in hot paths

## Prerequisites

- **Rust 1.70+:** Install from [rust-lang.org](https://www.rust-lang.org/)
- **PyTorch C++ (libtorch):** Required for neural networks via `tch` crate
- **Node.js 18+:** For frontend development and building


