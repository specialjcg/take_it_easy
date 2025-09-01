# Modes de Jeu - Take It Easy

## 🎯 Modes Disponibles

### 1. **MCTS vs Humain** (1v1)
```bash
cargo run -- --mode mcts-vs-human -s 150
```
- **Port WebSocket** : 9001
- **Un seul joueur** contre MCTS
- **Interface** : WebSocket direct
- **Simulations MCTS** : configurable avec `-s`

### 2. **Multiplayer** (Plusieurs joueurs + MCTS) - **ACTUEL**
```bash
cargo run -- --mode multiplayer -p 50051
```
- **Ports** : gRPC 50051, Web 51051
- **Plusieurs joueurs humains** + MCTS automatique
- **Interface** : gRPC + Frontend web
- **Flow indépendant** : Comme implémenté

### 3. **Training** (Entraînement)
```bash
cargo run -- --mode training -g 200
```
- **Port WebSocket** : 9000  
- **Mode entraînement** des réseaux de neurones
- **Parties** : configurable avec `-g`

## 🔧 Options Communes

| Option | Description | Défaut |
|--------|-------------|--------|
| `-g, --num-games` | Nombre de parties (training) | 200 |
| `-s, --num-simulations` | Simulations MCTS par coup | 150 |
| `-p, --port` | Port gRPC (multiplayer) | 50051 |
| `--mode` | Mode de jeu | `mcts-vs-human` |

## 🚀 Comment Changer de Mode

### Arrêter le Backend Actuel
```bash
# Tuer le processus actuel (multiplayer)
kill 2794857
```

### Lancer MCTS vs Humain
```bash
cargo run -- --mode mcts-vs-human --num-simulations 200
```

### Lancer Multiplayer avec Plus de Simulations  
```bash
cargo run -- --mode multiplayer --port 50051 --num-simulations 300
```

## 🎮 Interfaces

### Mode MCTS vs Humain
- **Connexion** : WebSocket sur port 9001
- **Client** : Interface WebSocket custom
- **Format** : Messages JSON directs

### Mode Multiplayer
- **Connexion** : gRPC sur port 50051
- **Interface Web** : http://localhost:51051
- **Client** : Frontend React/TypeScript
- **Format** : Protobuf via gRPC-Web

## ⚙️ Configuration Recommandée

### Pour Jouer Contre MCTS Fort
```bash
cargo run -- --mode mcts-vs-human --num-simulations 500
```

### Pour Développement/Test
```bash  
cargo run -- --mode multiplayer --port 50051 --num-simulations 100
```

### Pour Analyse Poussée
```bash
cargo run -- --mode mcts-vs-human --num-simulations 1000
```