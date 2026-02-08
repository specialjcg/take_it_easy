# Take It Easy

A comprehensive **Take It Easy** board game implementation featuring:
- Rust backend with gRPC API
- Advanced AI with neural networks (CNN + Q-Net or **GAT** - Graph Attention Network)
- Two frontend options: Elm (recommended) and SolidJS
- User authentication (email/password + OAuth)
- Multiplayer support

> 🏆 **Record**: The **Graph Transformer** achieves **149.38 pts** in evaluation (200 games), surpassing the CNN+MCTS hybrid (127.30 pts) by **+17.4%**!

![Game Screenshot](docs/images/game_finished.png)

---

## Game Rules

**Take It Easy** is a strategic tile-placement game for 1-4 players designed by Peter Burley.

### Objective

Score the most points by creating complete lines of matching numbers across your hexagonal board.

### Components

- **Board**: A hexagonal grid with 19 spaces arranged in a honeycomb pattern
- **Tiles**: 27 unique hexagonal tiles, each displaying 3 colored bands with numbers:
  - **Vertical band** (top to bottom): 1, 5, or 9
  - **Diagonal left band**: 2, 6, or 7
  - **Diagonal right band**: 3, 4, or 8

### Gameplay

1. **Tile Draw**: Each turn, a tile is randomly drawn and announced to all players
2. **Placement**: Every player simultaneously places the same tile on any empty space of their own board
3. **No Rotation**: Tiles cannot be rotated - they must be placed with their orientation preserved
4. **19 Turns**: The game ends after all 19 spaces are filled

### Scoring

Points are calculated for each complete line across the board:

| Direction | Number of lines |
|-----------|-----------------|
| Vertical (top-bottom) | 5 lines (3-4-5-4-3 spaces) |
| Diagonal left | 5 lines |
| Diagonal right | 5 lines |

**Scoring a line:**
- If ALL tiles in a line share the **same number** for that direction: `number × tiles in line`
- If numbers differ: **0 points**

**Example**: A vertical line of 4 tiles all showing "9" scores `9 × 4 = 36 points`

### Strategy Tips

- Plan ahead for longer lines (5 tiles = maximum multiplier)
- The center positions are most valuable (intersect all 3 directions)
- Balance between completing safe short lines vs. risky long lines
- Watch which tiles have been played to estimate remaining probabilities

---

## 1. Prerequisites

| Component | Version | Notes |
|-----------|---------|-------|
| Rust toolchain | 1.70+ | `rustup default stable` |
| Node.js + npm | Node 18+ | Required for frontends |
| Elm | 0.19.1 | For Elm frontend |
| protoc | 3.21+ | gRPC/tonic code generation |
| libtorch | 2.1+ (CPU) | Required by `tch` crate for neural inference |

> **Linux/macOS**: Export libtorch path:
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

# Elm frontend (recommended)
cd frontend-elm
npm install
cd ..

# OR SolidJS frontend
cd frontend
npm install
cd ..
```

---

## 3. Running the Application

### Backend

```bash
# Development
cargo run -- --mode multiplayer --port 50051 --num-simulations 300

# Production (release build)
cargo run --release -- --mode multiplayer --port 50051 --num-simulations 300
```

The backend exposes:
- **gRPC API** on `localhost:50051` (game sessions)
- **Auth REST API** on `localhost:51051/auth` (login, register, OAuth)

### Frontend (Elm - Recommended)

```bash
cd frontend-elm

# Development (with hot reload)
./dev.sh

# Build for production
./build.sh
```

Then open `http://localhost:8000` (dev) or serve `public/` folder.

### Frontend (SolidJS - Alternative)

```bash
cd frontend
npm run dev -- --host 0.0.0.0 --port 3000
```

Visit `http://localhost:3000`

---

## 4. Game Modes

| Mode | MCTS Simulations | Description |
|------|------------------|-------------|
| Solo Facile | 150 | Easy AI opponent |
| Solo Normal | 300 | Moderate AI challenge |
| Solo Difficile | 1000 | Strong AI opponent |
| Multijoueur | - | Play against other players online |

---

## 5. Architecture

```
take_it_easy/
├── src/                    # Rust backend
│   ├── auth/               # Authentication (JWT, OAuth, email)
│   ├── game/               # Game logic (tiles, plateau, scoring)
│   ├── mcts/               # Monte Carlo Tree Search engine
│   ├── neural/             # Neural networks (CNN, GAT, Q-Net)
│   ├── services/           # gRPC services (game, session)
│   └── servers/            # HTTP + gRPC server setup
├── frontend-elm/           # Elm frontend (MVU architecture)
│   ├── src/Main.elm        # Main application
│   ├── src/TileSvg.elm     # SVG tile rendering
│   └── public/             # Static assets + JS ports
├── frontend/               # SolidJS frontend (alternative)
├── model_weights/          # Neural network weights
│   ├── cnn/                # CNN policy & value networks
│   ├── graph_transformer_policy.safetensors  # Best model (154.26 pts) ⭐
│   ├── gat_jk_max_max_policy.safetensors  # GAT-JK MaxPool (147.16 pts)
│   ├── gat_weighted_cosine_policy.pt  # GAT (147.13 pts)
│   ├── gat_elite150/       # GAT trained on elite games (≥150 pts)
│   └── qvalue_net.params   # Q-Value network (MCTS pruning)
├── protos/                 # gRPC protocol definitions
└── docs/                   # Documentation
```

---

## 5.1 Neural Network Architectures

The AI uses neural networks to guide decision-making. Two architectures are available:

### CNN (Convolutional Neural Network)

Traditional approach treating the hexagonal board as a 5×5 grid with 47 feature channels:
- **Input**: Board state encoded as spatial tensor
- **Architecture**: 3 convolutional layers + fully connected heads
- **Usage**: Combined with MCTS and Q-Net for position pruning

### GAT (Graph Attention Network)

Graph-based approach respecting the hexagonal topology:
- **Input**: 19 nodes (hex positions) with 47 features each
- **Architecture**: Multi-head attention layers learning neighbor relationships
- **Advantage**: Naturally models hexagonal adjacency without grid distortion

### Graph Transformer ⭐ *New - Best Performance*

Full self-attention between ALL 19 nodes (not just neighbors):
- **Architecture**: Standard Transformer encoder with learnable positional encoding
- **Key insight**: Full attention captures long-range dependencies (e.g., diagonal lines spanning opposite corners)
- **Benefit**: No adjacency constraints - learns which positions are strategically related

| Config | Peak Score | Eval Score | Description |
|--------|------------|------------|-------------|
| **2 layers, dropout=0.2** | 151.30 pts | **149.38 pts** 🏆 | Best generalization, 98.5% ≥100 |
| 2 layers, dropout=0.1 | **154.26 pts** | 142.58 pts | Highest peak during training |
| 3 layers, dropout=0.1 | 150.87 pts | 145.04 pts | More layers, similar performance |

### GAT-JK (GAT + Jumping Knowledge)

Enhanced GAT with Jumping Knowledge Networks that combine representations from ALL layers:
- **Architecture**: 2-layer GAT with layer aggregation via MaxPool, Attention, or Concat
- **Key insight**: MaxPool aggregation (element-wise max across layers) works best
- **Benefit**: Captures both local (early layers) and global (later layers) patterns

| JK Mode | Best Score | Description |
|---------|------------|-------------|
| **MaxPool** | 147.16 pts | Element-wise maximum across layer outputs |
| Attention | 145.65 pts | Learned attention weights per layer |
| Concat | 143.63 pts | Concatenate all layer outputs |

### Benchmark Results

| Method | Avg Score | ≥100 pts | ≥140 pts | ≥150 pts |
|--------|-----------|----------|----------|----------|
| **Graph Transformer (dropout=0.2)** | **149.38** | **98.5%** | **63.5%** | **48.5%** |
| Graph Transformer (3 layers) | 145.04 | 95.5% | 60.0% | 40.5% |
| Graph Transformer (dropout=0.1) | 142.58 | 94.0% | 55.5% | 40.0% |
| GAT-JK MaxPool | 147.16 | 95.5% | 53.5% | 36.5% |
| GAT + Cosine LR | 147.13 | 95.0% | 63.0% | 47.0% |
| GAT-JK Attention | 145.65 | 94.5% | 54.5% | 38.0% |
| GAT Weighted (fixed LR) | 144.03 | 97.0% | 55.5% | 43.0% |
| GAT-JK Concat | 143.63 | 96.0% | 52.0% | 39.5% |
| GAT + Augmentation (6x) | 139.26 | 93.5% | 52.0% | 34.5% |
| GAT Policy (elite 150) | 137.75 | 92% | - | 30% |
| CNN + Q-net + MCTS | 127.30 | 82% | 27% | - |
| GAT + MCTS | 120.89 | 82% | - | 12% |
| Pure MCTS (200 sim) | 99.48 | 52% | - | 5% |
| Greedy | 21.81 | 0% | 0% | 0% |

> **Key finding**: The Graph Transformer with dropout=0.2 outperforms the CNN+MCTS hybrid by **+22.08 points** (+17.4%), with faster inference (no MCTS simulations needed).

#### Training Insights

| Technique | Effect |
|-----------|--------|
| **Cosine LR Scheduler** | +3.1 pts - better convergence in late training |
| **Weighted Loss** (power=3.0) | Higher scores contribute more to learning |
| **Dropout** (0.2 for GT, 0.1 for GAT) | +6.8 pts eval improvement for Graph Transformer |
| **Weight Decay** (1e-4) | Helps generalization to game play |
| **Data Augmentation** (6x rotations) | Did NOT help - board edges have asymmetric value |

> **Note**: For Graph Transformer, dropout=0.2 gives better evaluation scores (149.38 pts) than dropout=0.1 (142.58 pts), despite lower peak during training.

#### Graph Transformer Optimization Experiments

Extensive experiments were conducted to improve the Graph Transformer beyond dropout=0.2:

| Experiment | Eval Score | ≥150 pts | Result |
|------------|------------|----------|--------|
| **Dropout 0.2 (baseline)** | **149.38** | **48.5%** | ✅ **Best** |
| 3 layers | 145.04 | 40.5% | ❌ More layers didn't help |
| Label smoothing 0.1 | 147.29 | 46.5% | ❌ Too much regularization |
| Embed 256, heads 8 | 141.74 | 38.5% | ❌ Larger model overfit |
| Warmup 5 epochs | 145.02 | 42.5% | ❌ No improvement |
| Batch size 32 | 143.35 | 38.5% | ❌ Smaller batches hurt |
| Self-play training | 134.46 | 29.5% | ❌ Lost generalization |
| Curriculum learning | 142.87 | 41.5% | ❌ No benefit |

**Conclusion**: The base configuration (embed=128, heads=4, 2 layers, dropout=0.2) is optimal. The model has reached its performance ceiling with current architecture and data.

#### Multi-Seed Training Results

Training with different random seeds shows seed sensitivity:

| Seed | Val Acc | Game Score | Games ≥140 |
|------|---------|------------|------------|
| 42   | 61.85%  | **144.72** | 57.5% |
| 123  | 62.11%  | 143.41 | 60.5% |
| 456  | 61.47%  | 137.65 | 46.0% |
| 789  | 61.88%  | 141.41 | 56.0% |
| 2024 | 61.49%  | 141.50 | 53.0% |

**Summary**: Average 141.74 pts, range 137.65-144.72 pts (~7 pts variance due to initialization).
Note: Validation accuracy does not correlate with game performance.

### Training Neural Networks

```bash
# Best configuration: Graph Transformer (149.38 pts eval) ⭐
cargo run --release --bin train_graph_transformer -- \
  --epochs 80 \
  --embed-dim 128 \
  --num-layers 2 \
  --heads 4 \
  --lr 0.0005 \
  --dropout 0.2 \
  --save-path model_weights/graph_transformer

# GAT-JK with MaxPool aggregation (147.16 pts)
cargo run --release --bin train_gat_jk -- \
  --jk-mode max \
  --epochs 80 \
  --save-path model_weights/gat_jk_max

# Alternative JK modes: concat, attention
cargo run --release --bin train_gat_jk -- --jk-mode concat --epochs 80
cargo run --release --bin train_gat_jk -- --jk-mode attention --epochs 80

# Standard GAT with cosine LR (147.13 pts)
cargo run --release --bin train_gat_weighted -- \
  --min-score 100 \
  --weight-power 3.0 \
  --epochs 80 \
  --dropout 0.2 \
  --weight-decay 0.0001 \
  --lr-scheduler cosine \
  --save-path model_weights/gat_weighted_cosine

# Alternative: train on elite games only (score ≥ 150)
cargo run --release --bin train_gat_supervised -- --min-score 150 --epochs 50

# Evaluate GAT policy
cargo run --release --bin eval_gat_supervised -- --games 200 --model model_weights/gat_weighted_best_policy.pt
```

---

## 6. Authentication

The backend supports:
- **Email/Password** authentication with Argon2 hashing
- **JWT tokens** for session management
- **OAuth2** (Google, GitHub) - configure in environment variables

Database: SQLite (`data/users.db`)

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/auth/register` | POST | Create new account |
| `/auth/login` | POST | Login with email/password |
| `/auth/me` | GET | Get current user (requires Bearer token) |
| `/auth/oauth/google` | GET | OAuth2 with Google |
| `/auth/oauth/github` | GET | OAuth2 with GitHub |

---

## 7. Useful Commands

| Command | Description |
|---------|-------------|
| `cargo test` | Run Rust tests |
| `cargo run --bin compare_mcts_hybrid -- --games 50` | Benchmark AI |
| `cargo fmt && cargo clippy` | Format and lint |
| `cd frontend-elm && elm make src/Main.elm` | Compile Elm |

---

## 8. Troubleshooting

| Issue | Fix |
|-------|-----|
| **libtorch not found** | Export `LD_LIBRARY_PATH` to `libtorch/lib` |
| **protoc missing** | `apt install protobuf-compiler` or `brew install protobuf` |
| **Frontend can't reach backend** | Check backend is running on ports 50051 and 51051 |
| **Elm compilation error** | Run `elm make src/Main.elm --output=public/elm.js` |

---

## 9. Development

### Building Elm Frontend

```bash
cd frontend-elm
elm make src/Main.elm --optimize --output=public/elm.js
```

### Building for Production

```bash
# Backend
cargo build --release

# Frontend
cd frontend-elm && ./build.sh
```

---

## 10. Production Deployment

### Live Demo

🎮 **Play now**: https://takeitasy.mooo.com

### Overview

The game can be deployed on a minimal VPS (1GB RAM, €1/month) using Docker cross-compilation for glibc compatibility.

```
┌─────────────────┐     HTTPS      ┌─────────────────┐
│   Browser       │◄──────────────►│   nginx         │
│   (Elm SPA)     │                │   (reverse      │
└─────────────────┘                │    proxy)       │
                                   └────────┬────────┘
                                            │
                    ┌───────────────────────┼───────────────────────┐
                    │                       │                       │
                    ▼                       ▼                       ▼
            ┌───────────────┐      ┌───────────────┐      ┌───────────────┐
            │ Static Files  │      │ Auth API      │      │ gRPC-Web      │
            │ /             │      │ /auth/*       │      │ /takeiteasygame.*
            │ port 80/443   │      │ port 51051    │      │ port 50052    │
            └───────────────┘      └───────────────┘      └───────────────┘
                                            │
                                            ▼
                                   ┌───────────────┐
                                   │ Rust Backend  │
                                   │ + Graph       │
                                   │   Transformer │
                                   │ + libtorch    │
                                   └───────────────┘
```

### Prerequisites

| Component | Version | Purpose |
|-----------|---------|---------|
| Docker | 20+ | Cross-compilation for glibc 2.35 |
| VPS | Ubuntu 22.04 | Production server |
| Domain | Any | FreeDNS offers free subdomains |

### Step 1: Build with Docker

Docker ensures the binary is compatible with Ubuntu 22.04 (glibc 2.35):

```bash
# Copy template and configure
cp build-docker.sh.example build-docker.sh
cp deploy.sh.example deploy.sh

# Edit deploy.sh with your VPS details:
# VPS_USER="root"
# VPS_HOST="your-vps.example.com"
# VPS_PORT="22"

# Build (first run takes ~10 min, subsequent builds ~2 min)
./build-docker.sh
```

This creates:
- `target/release/take_it_easy` - Binary (15 MB)
- `docker-libs/` - libtorch libraries (420 MB)

### Step 2: Deploy to VPS

```bash
# Create deployment package
./deploy.sh package

# Deploy (uploads ~450 MB)
./deploy.sh deploy
```

The deploy script:
1. Uploads binary, libs, frontend, model weights
2. Creates `takeitasy` system user
3. Installs systemd service
4. Configures nginx reverse proxy

### Step 3: Configure HTTPS (Let's Encrypt)

```bash
# SSH to your VPS
ssh user@your-vps.example.com

# Install certbot
apt install certbot python3-certbot-nginx

# Get certificate (auto-configures nginx)
certbot --nginx -d yourdomain.example.com
```

### Step 4: Set JWT Secret (Security)

```bash
# On VPS, edit the service file
sudo systemctl edit takeitasy

# Add secure JWT secret:
[Service]
Environment=JWT_SECRET=your-random-32-char-secret
Environment=RUST_ENV=production

# Restart
sudo systemctl restart takeitasy
```

> ⚠️ **Security**: In production (`RUST_ENV=production`), the server will refuse to start without `JWT_SECRET` set.

### Deployment Commands

| Command | Description |
|---------|-------------|
| `./build-docker.sh` | Build with Docker (glibc 2.35 compat) |
| `./deploy.sh package` | Create deployment package |
| `./deploy.sh deploy` | Full deploy (build + package + upload) |
| `./deploy.sh status` | Check service status |
| `./deploy.sh logs` | View service logs |
| `./deploy.sh restart` | Restart the service |

### File Structure on VPS

```
/opt/takeitasy/
├── take_it_easy          # Rust binary
├── lib/                  # libtorch libraries
│   ├── libtorch_cpu.so
│   ├── libc10.so
│   └── libgomp-*.so
├── model_weights/        # Neural network weights
│   └── graph_transformer_policy.safetensors
├── frontend/             # Elm SPA (static files)
└── data/
    ├── auth.db           # User database (SQLite)
    └── recorded_games/   # Game recordings for AI training
```

### Game Recording

All games are automatically recorded for future AI improvement:

```bash
# Download recorded games from VPS
scp user@vps:/opt/takeitasy/data/recorded_games/*.csv ./recorded_games/

# CSV format: game_id, turn, player_type, plateau_state, tile, position, score
```

### Free Domain with FreeDNS

1. Create account at https://freedns.afraid.org
2. Add subdomain → Type: `AAAA` (for IPv6) or `A` (for IPv4)
3. Point to your VPS IP
4. Update nginx `server_name` directive

### Troubleshooting Deployment

| Issue | Solution |
|-------|----------|
| `libtorch_cpu.so not found` | Check `LD_LIBRARY_PATH` in systemd service |
| `GLIBC_2.xx not found` | Rebuild with Docker (ensures glibc 2.35) |
| `JWT_SECRET must be set` | Set `JWT_SECRET` environment variable |
| `502 Bad Gateway` | Check if backend is running: `systemctl status takeitasy` |
| gRPC-Web errors | Verify nginx proxies `/takeiteasygame.*` to port 50052 |

---

## License

MIT

---

Have fun playing **Take It Easy**!
