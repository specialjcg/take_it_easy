# Take It Easy – Installation & Runbook

This repository hosts the Rust backend (gRPC + MCTS AI) and the SolidJS frontend for the **Take It Easy** multiplayer board game.  
The goal of this README is to help you clone the project from GitHub, install dependencies, and launch the full stack in just a few commands.

---

## 1. Prerequisites

| Component | Version | Notes |
|-----------|---------|-------|
| Rust toolchain | 1.70+ | `rustup default stable` |
| Node.js + npm | Node 18 / npm 9+ | Required for the SolidJS client |
| protoc | 3.21+ | Needed because the backend uses gRPC/tonic |
| libtorch | 2.1+ (CPU build is enough) | Required by the `tch` crate for neural inference |

> **Linux/macOS**: After extracting libtorch, export the path (adjust to your install):
> ```bash
> export LIBTORCH_HOME="$HOME/libtorch"
> export LD_LIBRARY_PATH="$LIBTORCH_HOME/lib:$LD_LIBRARY_PATH"
> ```

---

## 2. Clone & Install

```bash
git clone https://github.com/specialjcg/take_it_easy.git
cd take_it_easy

# Backend dependencies
cargo fetch

# Frontend dependencies
cd frontend
npm install
cd ..
```

---

## 3. Running the Application

### Option A – One-liner (recommended for dev)
```bash
make dev
```
This starts:
- the Rust backend (gRPC server on `localhost:50051`)
- the SolidJS frontend (Vite dev server on `localhost:3000`)

### Option B – Manual terminals
1. **Backend**
   ```bash
   cargo run -- --mode multiplayer --port 50051 --num-simulations 200
   ```
2. **Frontend**
   ```bash
   cd frontend
   npm run dev -- --host 0.0.0.0 --port 3000
   ```

Visit `http://localhost:3000` to play. The frontend talks to the backend through gRPC-Web.

---

## 4. Production Build / Deployment

```bash
make build
# Outputs:
#   - backend binary (release)
#   - frontend static bundle in frontend/dist
```

Run the backend in release mode:
```bash
./target/release/take_it_easy --mode multiplayer --port 50051 --num-simulations 300 --single-player false
```

Serve the frontend bundle (`frontend/dist`) with any static server (nginx, Vite preview, etc.).

---

## 5. Useful Commands

| Command | Description |
|---------|-------------|
| `cargo test` | Run Rust unit + integration tests |
| `npm test -- --watch=false` | Run frontend tests (Vitest) |
| `cargo run -- --single-player` | Quick solo game vs MCTS |
| `cargo run --bin compare_mcts -- --games 50` | Benchmark neural vs pure MCTS |
| `make fmt` / `cargo fmt` | Apply Rust formatting |
| `npm run lint` | Lint SolidJS code |

---

## 6. Repository Layout

```
src/                # Rust backend (game logic, services, MCTS, neural bindings)
frontend/           # SolidJS app (Vite)
model_weights/      # CNN weights used in production
docs/               # Architecture notes, experiment archives
docs/archive/       # Legacy experiments/scripts (not needed to run)
scripts/            # Active Python helpers for data analysis
Makefile            # Common dev + build shortcuts
dev_start.sh        # Legacy helper (make dev wraps this)
```

---

## 7. Troubleshooting

| Issue | Fix |
|-------|-----|
| **`libtorch` not found** | Ensure `LD_LIBRARY_PATH` points to `libtorch/lib` before launching the backend. |
| **`protoc` missing`** | Install via package manager (`apt install protobuf-compiler`, `brew install protobuf`). |
| **Frontend can’t reach backend** | Confirm backend is on `localhost:50051` and run `npm run dev -- --host 0.0.0.0`. |
| **Slow simulations** | Lower `--num-simulations` (e.g., 150) or build with `cargo run --release`. |

---

## 8. Next Steps

- Explore `docs/` for detailed AI/MCTS notes.
- Run benchmarks with `cargo run --bin compare_mcts`.
- Contribute fixes via pull requests (use `cargo fmt && cargo clippy && cargo test` before pushing).

Have fun playing and hacking on **Take It Easy**! 🦀🎮
