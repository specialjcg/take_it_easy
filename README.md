# Take It Easy

A comprehensive **Take It Easy** board game implementation featuring:
- Rust backend with gRPC API
- MCTS AI with neural network (CNN + Q-Net hybrid)
- Two frontend options: Elm (recommended) and SolidJS
- User authentication (email/password + OAuth)
- Multiplayer support

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
│   ├── neural/             # CNN + Q-Net neural networks
│   ├── services/           # gRPC services (game, session)
│   └── servers/            # HTTP + gRPC server setup
├── frontend-elm/           # Elm frontend (MVU architecture)
│   ├── src/Main.elm        # Main application
│   ├── src/TileSvg.elm     # SVG tile rendering
│   └── public/             # Static assets + JS ports
├── frontend/               # SolidJS frontend (alternative)
├── model_weights/          # Neural network weights
│   ├── cnn_policy/         # Policy network
│   ├── cnn_value/          # Value network
│   └── qvalue/             # Q-Value network (hybrid MCTS)
├── protos/                 # gRPC protocol definitions
└── docs/                   # Documentation
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

## License

MIT

---

Have fun playing **Take It Easy**!
