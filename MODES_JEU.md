# Modes de Jeu - Take It Easy (Simplifié)

## 🎯 Modes Disponibles

### 1. **Multiplayer** (Défaut)
```bash
# Mode multijoueur normal
cargo run -- --mode multiplayer -p 50051

# Mode UN SEUL joueur contre MCTS ⭐ NOUVEAU
cargo run -- --mode multiplayer --single-player -s 200
```
- **Ports** : gRPC 50051, Web 51051
- **Interface** : gRPC + Frontend web (même pour single-player)
- **Avec `--single-player`** : 1 joueur humain + MCTS
- **Sans `--single-player`** : Plusieurs joueurs + MCTS

### 2. **Training** (Entraînement)
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
| `--mode` | Mode de jeu | `multiplayer` |
| `--single-player` | **NOUVEAU** : 1 joueur vs MCTS | `false` |

## 🚀 Comment Lancer

### 🤖 Un Joueur vs MCTS  
```bash
# Arrêter le backend actuel
kill 2794857

# Lancer single-player contre MCTS
cargo run -- --single-player --num-simulations 300
```

### 👥 Multijoueur Normal
```bash  
# Relancer multijoueur normal (défaut actuel)
cargo run -- --mode multiplayer --port 50051
```

### 🎯 MCTS Plus Fort
```bash
# Single-player avec MCTS très fort
cargo run -- --single-player --num-simulations 1000
```

## 🎮 Interface Unifiée

**Tous les modes multiplayer** (single-player et multijoueur) utilisent la **même interface** :
- **Connexion** : gRPC sur port 50051
- **Interface Web** : http://localhost:51051  
- **Client** : Votre frontend React/TypeScript existant
- **Format** : Protobuf via gRPC-Web
- **Avantage** : Aucun changement de code frontend nécessaire

## ⚙️ Configuration Recommandée

### 🥊 Défier MCTS Fort
```bash
cargo run -- --single-player --num-simulations 500
```

### 🎯 MCTS Expert
```bash
cargo run -- --single-player --num-simulations 1000
```

### 👥 Multijoueur avec MCTS Fort
```bash  
cargo run -- --mode multiplayer --num-simulations 300
```

### ⚡ Mode Rapide pour Tests
```bash
cargo run -- --single-player --num-simulations 50
```