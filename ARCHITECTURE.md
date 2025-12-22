# Take It Easy - Architecture Documentation

## Table of Contents
- [Project Overview](#project-overview)
- [System Architecture](#system-architecture)
- [Core Components](#core-components)
- [Data Flow](#data-flow)
- [Technology Stack](#technology-stack)
- [Performance Optimizations](#performance-optimizations)
- [Design Decisions](#design-decisions)
- [Testing Strategy](#testing-strategy)
- [Future Extensibility](#future-extensibility)

---

## Project Overview

**Take It Easy** is a comprehensive AI-powered board game implementation featuring:
- Monte Carlo Tree Search (MCTS) engine with neural network guidance
- Dual neural network architectures (CNN and GNN)
- Multiplayer support via gRPC/WebSocket
- Real-time web UI with async move processing
- Self-play training pipeline

**Metrics:**
- ~16,770 lines of Rust code
- 175 automated tests (64 lib + 103 main + 8 integration)
- Support for 1-4 players + AI opponents

---

## System Architecture

The project follows **Hexagonal Architecture** (Ports & Adapters) principles:

```
┌─────────────────────────────────────────────────────────┐
│                    Presentation Layer                    │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │ gRPC Server │  │ Web UI (Axum)│  │ CLI Binaries  │ │
│  └─────────────┘  └──────────────┘  └───────────────┘ │
└─────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────┐
│                    Application Layer                     │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────┐│
│  │ Game Service │  │Session Manager│  │MCTS Interface││
│  └──────────────┘  └───────────────┘  └──────────────┘│
└─────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────┐
│                      Domain Layer                        │
│  ┌──────────┐  ┌─────────────┐  ┌──────────────────┐  │
│  │Game Logic│  │MCTS Algorithm│  │Neural Networks  │  │
│  │(Pure Rust│  │ + Strategies │  │(CNN/GNN Variants)│  │
│  └──────────┘  └─────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────┐
│                   Infrastructure Layer                   │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Tensor I/O  │  │Model Weights │  │Training Data │  │
│  └─────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Key Architectural Patterns

1. **Dependency Inversion Principle (DIP)**
   - `PolicyEvaluator` and `ValueEvaluator` traits abstract neural network implementations
   - Enables testing with mock implementations
   - Supports future neural architectures without changing MCTS code

2. **Separation of Concerns**
   - Game logic (`src/game`) is pure and deterministic
   - MCTS algorithm (`src/mcts`) is independent of neural networks
   - Services layer (`src/services`) handles async/networking concerns

3. **Immutability Where Possible**
   - Game state clones are minimized (P2 optimization: 26+ clones eliminated)
   - Critical functions take references (`&Plateau`) instead of ownership

---

## Core Components

### 1. Game Logic (`src/game/`)
Pure domain logic with zero dependencies on AI or networking.

**Key Modules:**
- `plateau.rs` - 19-position hexagonal board representation
- `tile.rs` - Tile with 3 directional values (0-9 each)
- `deck.rs` - 27 unique tiles
- `get_legal_moves.rs` - **Optimized**: Takes `&Plateau` (P2 improvement)
- `scoring/` - Line completion and point calculation

**Invariants:**
- Plateau size is always 19
- Tiles are immutable once placed
- Scoring is deterministic and reproducible

### 2. MCTS Engine (`src/mcts/`)
Monte Carlo Tree Search with neural network guidance.

**Key Components:**
- `algorithm.rs` - Main MCTS loop (150-300 simulations per move)
- `gumbel_selection.rs` - Gumbel-Top-k exploration strategy
- `node.rs` - Tree node representation (decision vs chance nodes)
- `hyperparameters.rs` - Configurable C_PUCT, rollout counts, etc.

**Variants:**
- Standard UCB-based MCTS
- Gumbel-MCTS (experimental, see `docs/`)
- Expectimax-MCTS (archived due to poor performance)

**Performance Notes:**
- Hot path: `get_legal_moves()` called 26+ times per simulation
- Plateau/deck clones necessary for tree exploration (legitimate)
- Neural inference is blocking but wrapped in background tasks

### 3. Neural Networks (`src/neural/`)
Dual-network architecture following AlphaZero principles.

**Network Types:**
```rust
pub trait PolicyEvaluator: Send {
    fn forward(&self, input: &Tensor, train: bool) -> Tensor;
    fn arch(&self) -> &NNArchitecture;
}

pub trait ValueEvaluator: Send {
    fn forward(&self, input: &Tensor, train: bool) -> Tensor;
    fn arch(&self) -> &NNArchitecture;
}
```

**Implementations:**
- `PolicyNet` - Predicts move probabilities (19 positions)
- `ValueNet` - Predicts final score (regression)
- Both support CNN and GNN architectures

**Architecture Details:**
- **CNN**: 160→128→96→64 channel progression, ResNet blocks, GroupNorm
- **GNN**: Graph-based with 3 message-passing layers (experimental)
- Input: 8-channel feature stack (5×5 spatial + metadata)
- Training: Supervised learning from MCTS self-play

**PyTorch Constraints:**
- Networks are `Send` but not `Sync` (raw pointers in tensors)
- Must be wrapped in `Arc<Mutex<>>` for async contexts
- Neural inference cannot use `spawn_blocking` safely

### 4. Services Layer (`src/services/`)
Async orchestration and session management.

**Key Services:**
- `game_manager.rs` - Core game state mutations
- `session_manager.rs` - Multi-player session tracking
- `game_service/` - gRPC endpoint implementations
  - `async_move_handler.rs` - Immediate response + background MCTS

**Async Design:**
```rust
pub async fn make_move_async_logic(
    // Returns immediately with "PROCESSING" status
    // Spawns background task for MCTS computation
) -> Result<Response<MakeMoveResponse>, Status>
```

**Benefits:**
- UI remains responsive during 150+ simulation MCTS runs
- Multiple players can submit moves concurrently
- Background tasks update session state asynchronously

### 5. Server Layer (`src/servers/`)
Network interfaces for client connectivity.

**Servers:**
- `grpc.rs` - Native gRPC (port 50051)
- `grpc.rs` - gRPC-Web with CORS (port 8080)
- `web_ui.rs` - Axum REST API + static file serving

**Protocol Buffers:**
- Schema: `proto/takeiteasygame/v1/game.proto`
- Generated: `src/generated/`
- Services: `SessionService`, `GameService`

---

## Data Flow

### Game Move Flow
```
1. Client submits move (gRPC/REST)
        ↓
2. Immediate validation & acknowledgment
        ↓
3. Background: MCTS simulation (150 sims)
        ↓
4. PolicyNet.forward() - get priors
        ↓
5. ValueNet.forward() - evaluate positions
        ↓
6. Select best move via UCB/Gumbel
        ↓
7. Update game state + session
        ↓
8. Client polls for updated state
```

### Training Data Flow
```
1. Self-play games generate MCTS results
        ↓
2. MCTSResult → Tensors (states, positions, scores)
        ↓
3. Save to .pt files (data/)
        ↓
4. Load batch → supervised training
        ↓
5. Backprop → update weights
        ↓
6. Save checkpoint → models/
```

---

## Technology Stack

### Core Dependencies
- **Rust 2021 Edition** - Memory safety, zero-cost abstractions
- **tch 0.19** - PyTorch bindings for neural networks
- **tokio 1.48** - Async runtime (multi-threaded)
- **tonic 0.14** - gRPC server framework
- **axum 0.8** - Web framework for REST API
- **serde 1.0** - Serialization (JSON, Protocol Buffers)

### Key Features Used
- **Async/await** - Non-blocking I/O for multiplayer
- **Traits** - DIP compliance (`PolicyEvaluator`, `ValueEvaluator`)
- **Arc<Mutex<>>** - Thread-safe shared state
- **Rayon** - Data parallelism (potential future use)

### Build Dependencies
- **protoc** - Protocol buffer compiler
- **prost-build** - Rust protobuf code generation

---

## Performance Optimizations

### P0: Error Handling (Reliability)
- **Eliminated 9 critical `unwrap`/`expect` calls**
- Replaced with proper `Result<T, E>` propagation
- Impacts: `data/load_data.rs`, `data/append_result.rs`, `training/session.rs`

### P1: Dependency Inversion (Architecture)
- **Created `PolicyEvaluator` and `ValueEvaluator` traits**
- Enables mock testing without PyTorch
- Future-proof for new neural architectures

### P2: Clone Elimination (Performance)
- **Eliminated 26+ unnecessary `Plateau` clones**
- Changed `get_legal_moves(Plateau)` → `get_legal_moves(&Plateau)`
- **Impact**: Called 7 times in MCTS algorithm.rs hot path per move
- **Benefit**: Plateau has 19 tiles → ~50% reduction in allocations

### Remaining Clones (Legitimate)
Many clones in MCTS are **necessary**:
```rust
// Legitimate: exploring independent tree branches
let mut temp_plateau = plateau.clone();
temp_plateau.tiles[position] = chosen_tile;
simulate_games_smart(temp_plateau, deck.clone(), None);
```

### Future Optimization Opportunities
1. Object pooling for frequently cloned structures
2. SIMD vectorization for scoring calculations
3. Lock-free data structures for session management
4. Neural network quantization (INT8 inference)

---

## Design Decisions

### Why Not `spawn_blocking` for Neural Inference?
**Decision:** Keep neural inference in async context despite blocking CPU work.

**Rationale:**
- PyTorch tensors are not `Send + Sync` (raw C pointers)
- Cannot safely move networks between threads
- Background tasks (`task::spawn`) already isolate work from request handlers
- Mutex serialization acceptable given batch size (1 inference per move)

**Documentation:** See `src/services/game_manager.rs:208-218`

### Why Two Networks Instead of One?
**Decision:** Separate PolicyNet and ValueNet (AlphaZero style).

**Rationale:**
- Policy needs sharp distributions (softmax)
- Value needs bounded regression (tanh × 2 for [-2, 2] range)
- Different training dynamics (cross-entropy vs MSE)
- Easier to debug and tune independently

### Why Hexagonal Architecture?
**Decision:** Strict layer separation (domain → application → infrastructure).

**Benefits:**
- Game logic testable without networking
- MCTS algorithm testable without PyTorch
- Easy to swap gRPC for WebSockets
- Clear dependency directions (no circular imports)

---

## Testing Strategy

### Test Pyramid
```
         ┌─────────────┐
         │ Integration │ 8 tests
         │   (e2e)     │
         └─────────────┘
       ┌─────────────────┐
       │   Main Binary   │ 103 tests
       │  (with state)   │
       └─────────────────┘
     ┌─────────────────────┐
     │   Library (pure)    │ 64 tests
     │ (domain + mcts)     │
     └─────────────────────┘
```

### Test Categories
1. **Unit Tests** (`#[test]` in modules)
   - Pure functions (scoring, legal moves, tile logic)
   - MCTS node operations
   - Neural network forward passes

2. **Integration Tests** (`tests/`)
   - `lib_integration_test.rs` - Library metadata
   - `model_weights_sanity.rs` - Neural net weight checks
   - `ui_reactivity_regression_test.rs` - State mutations

3. **Property Tests** (Future)
   - Game state invariants always hold
   - Scoring is commutative
   - MCTS convergence properties

### Test Coverage Focus
- **High coverage**: Game logic, scoring (100%)
- **Medium coverage**: MCTS algorithm (~70%)
- **Low coverage**: Services layer (~40% - manual testing)

**Rationale:** Focus on deterministic, pure code. Async/network code requires more integration testing.

---

## Future Extensibility

### Planned Enhancements

1. **Alternative Neural Architectures**
   - Transformer-based policy networks
   - Ensemble methods (multiple policies)
   - Easy via `PolicyEvaluator` trait

2. **Advanced MCTS Variants**
   - AlphaZero-style PUCT
   - Rapid Action Value Estimation (RAVE)
   - Already abstracted in `mcts_core()`

3. **Multiplayer Features**
   - Ranked matchmaking
   - Replay systems
   - Spectator mode

4. **Training Improvements**
   - Distributed self-play
   - Curriculum learning
   - Hyperparameter auto-tuning (see `docs/`)

### Extension Points

**Adding a New Neural Architecture:**
```rust
// 1. Implement the traits
impl PolicyEvaluator for MyTransformerPolicy {
    fn forward(&self, input: &Tensor, train: bool) -> Tensor {
        // ... transformer implementation
    }
    fn arch(&self) -> &NNArchitecture {
        &NNArchitecture::Transformer
    }
}

// 2. Update enum
pub enum NNArchitecture {
    CNN,
    GNN,
    Transformer, // New!
}

// 3. Wire into manager
impl NeuralManager {
    pub fn with_arch(arch: NNArchitecture) -> Self {
        match arch {
            // ... existing cases
            NNArchitecture::Transformer => { /* construct */ }
        }
    }
}
```

**Adding a New Server Protocol:**
```rust
// src/servers/websocket.rs
pub struct WebSocketServer {
    session_manager: Arc<SessionManager>,
    // ... reuse existing services
}

impl WebSocketServer {
    pub async fn start(&self) -> Result<()> {
        // Implement WebSocket handler
        // Delegate to existing game_service
    }
}
```

---

## Appendix: File Organization

### Directory Structure
```
src/
├── bin/                # CLI tools (trainers, benchmarks)
├── data/              # Data I/O (load_data, append_result)
├── game/              # Pure game logic (no dependencies)
├── generated/         # Protobuf generated code
├── mcts/              # Monte Carlo Tree Search engine
├── neural/            # Neural networks (CNN/GNN)
│   └── training/      # Training loop, optimizer config
├── scoring/           # Point calculation
├── servers/           # gRPC, web UI servers
├── services/          # Async orchestration layer
│   └── game_service/  # Move handling, MCTS integration
├── strategy/          # Heuristics (contextual boost)
├── training/          # Self-play session management
├── utils/             # Shared utilities
├── lib.rs            # Library entry point
└── main.rs           # Binary entry point
```

### Key Files
- `src/mcts/algorithm.rs` - MCTS core (925 lines)
- `src/neural/policy_value_net.rs` - Neural nets + traits (515 lines)
- `src/services/game_manager.rs` - Game state logic (521 lines)
- `src/servers/grpc.rs` - gRPC server + CORS (346 lines)

---

## References

- [MCTS Research Papers](docs/research_papers_analysis.md)
- [CNN vs Expectimax Decision](docs/cnn_vs_expectimax_decision.md)
- [GNN Architecture Details](docs/silver_gnn_architecture.md)
- [Stochastic MCTS Taxonomy](docs/STOCHASTIC_MCTS_TAXONOMY.md)

---

**Last Updated:** 2025-12-22
**Maintained By:** Project Contributors
**Version:** 0.1.0
