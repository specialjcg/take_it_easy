# Instructions pour le Nouveau Claude

## 🎯 Objectif
Déboguer pourquoi le titre frontend affiche toujours "Multiplayer vs MCTS" au lieu de "Single vs MCTS" en mode single-player.

## 📋 Contexte Complet
**Lire d'abord** : `SESSION_CONTEXT.md` - Tout le travail déjà fait avec méthode TDD-Mikado.

## 🚀 Actions Immédiates

### 1. Nettoyer l'environnement
```bash
./CLEANUP_SCRIPT.sh
```

### 2. Lancer en mode single-player
```bash
./launch_modes.sh single
```

### 3. Tester et déboguer
- Ouvrir `http://localhost:3000` ou `http://localhost:3001`
- Ouvrir console navigateur (F12)
- Chercher logs debug : `🔍 DEBUG gameTitle` et `🔍 DEBUG convertSessionState`

## 🔍 Hypothèses à Vérifier

### Hypothèse 1: gameMode pas transmis du backend
```bash
# Vérifier logs backend
tail -f backend.log | grep -E "single-player|gameMode"
```

### Hypothèse 2: Sérialisation protobuf incorrecte
- Vérifier que `sessionState.gameMode` arrive bien dans `convertSessionState()`
- Logs console doivent montrer la valeur

### Hypothèse 3: Timing de mise à jour
- Le memo `gameTitle()` se met peut-être pas à jour
- Forcer re-render ou vérifier dépendances

## 🛠️ Outils Disponibles

### Configuration Claude
```bash
# Méthode TDD-Mikado harmonisée
claude rust-tdd "Debug gameMode transmission"
claude fix "Réparer titre frontend"
```

### Debug Frontend
```typescript
// Logs déjà ajoutés dans :
// - GameStateManager.convertSessionState()
// - MultiplayerApp.gameTitle()
```

### Validation Backend
```bash
# Tests toujours verts
cargo test  # 49/49 passing

# Compilation propre
cargo check --quiet
```

## 📊 Status
- **Code**: Toutes modifications faites (commit 3be5c02)
- **Tests**: ✅ Passing (49/49)
- **Architecture**: ✅ TDD-Mikado appliqué
- **Problème**: gameMode pas affiché côté frontend

## 🎯 Expected Result
Titre qui change dynamiquement :
- Single-player mode → "🎮 Take It Easy - Single vs MCTS"
- Multiplayer mode → "🎮 Take It Easy - Multiplayer vs MCTS"

---
*Context préservé pour continuation efficace*