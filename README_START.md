# 🎮 Take It Easy - Start Guide

## Quick Start

### Option 1: Scripts Bash 🔧

```bash
# Démarrage développement (rapide)
./dev_start.sh

# Démarrage production (avec build)
./start_all.sh

# Arrêt
./stop_all.sh
```

### Option 2: NPM Commands 📦

```bash
# Démarrage développement
npm run dev

# Démarrage production
npm run start

# Arrêt
npm run stop

# Build seulement
npm run build
```

### Option 3: Makefile 🛠️

```bash
# Voir toutes les commandes
make help

# Démarrage développement
make dev

# Démarrage production
make start

# Arrêt
make stop

# Services individuels
make backend
make frontend

# Maintenance
make build
make clean
make logs
```

## 🚀 Services

Une fois lancé, vous avez accès à :

- **Backend gRPC**: `localhost:50051`
- **Frontend**: `http://localhost:3000`
- **Logs**: `backend.log` et `frontend.log`

## 🎮 Interface de jeu

1. **Modes Solo** (single-player-*) :
   - ✅ Auto-connexion immédiate
   - ✅ Jeu direct contre MCTS
   - ✅ Pas d'interface de session

2. **Mode Multijoueur** :
   - 🔧 Interface de connexion
   - 👥 Gestion des joueurs
   - 🎯 Sessions partagées

## 🔧 Développement

```bash
# Backend seulement
cargo run -- --mode multiplayer

# Frontend seulement
cd frontend && npm run dev

# Build release
cargo build --release
cd frontend && npm run build
```

## 📝 Logs

```bash
# Voir les logs en temps réel
tail -f backend.log frontend.log

# Ou avec make
make logs
```

## 🛑 Arrêt

Toutes les méthodes supportent `Ctrl+C` ou utilisez :

```bash
./stop_all.sh
# ou
npm run stop
# ou
make stop
```