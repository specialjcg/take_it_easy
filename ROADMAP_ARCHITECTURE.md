# 🏗️ Roadmap Architecture - Take It Easy

## 📊 Analyse du Code Actuel

**Taille Codebase**: ~55,800 lignes Rust + ~5,000 lignes TypeScript  
**Technologies**: Rust, tokio, gRPC-Web, Solid.js, PyTorch (libtorch), MCTS  
**Couverture Tests**: ~5-10%  

---

## 🔴 CRITIQUES (Bloquent la maintenabilité)

### 1. **Services Monolithiques**
- **Problem**: `game_service.rs` (1,111 lignes), `session_service.rs` (502 lignes)
- **Impact**: Violation SRP, difficile à tester, charge cognitive élevée
- **Solution**: Découper en modules focalisés

```rust
// Structure proposée
mod game_service {
    mod move_handler;      // Gestion des coups
    mod state_manager;     // Gestion état jeu
    mod mcts_integration;  // Logique MCTS
    mod validation;        // Validation mouvements
}
```

### 2. **Couplage Fort Entre Couches**
- **Problem**: Services manipulent directement plusieurs préoccupations
- **Impact**: Changements en cascade, tests nécessitent setup complet
- **Solution**: Injection de dépendances et couches d'abstraction

### 3. **Gestion d'Erreurs Incohérente**
- **Problem**: Mélange String, Status, Result sans hiérarchie
- **Solution**: Hiérarchie d'erreurs structurée avec `thiserror`

```rust
#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error("Game not started")]
    GameNotStarted,
    #[error("Player {0} not found")]
    PlayerNotFound(String),
    #[error("Session error: {0}")]
    SessionError(#[from] SessionError),
}
```

---

## 🟠 HAUTE PRIORITÉ (Impact majeur)

### 1. **Architecture State Management**
- **Problem**: Duplication `GameSession` ↔ `TakeItEasyGameState`
- **Solution**: Event sourcing avec source unique de vérité

```rust
pub struct GameState {
    id: GameId,
    players: PlayerCollection,
    board: Board,
    phase: GamePhase,
}

pub enum GameEvent {
    PlayerJoined { player_id: PlayerId },
    TilePlaced { player_id: PlayerId, position: Position, tile: Tile },
    TurnCompleted { turn: u32 },
}
```

### 2. **Infrastructure de Tests**
- **Problem**: Couverture ~5-10%, pas de tests d'intégration
- **Solution**: Test harness complet avec mocks et fixtures

### 3. **Performance Frontend**
- **Problem**: Polling 500ms au lieu d'événements temps réel
- **Solution**: WebSocket avec gestion d'événements

```typescript
export class GameEventStream {
    subscribe(gameId: string): Observable<GameEvent> {
        return this.websocket.connect(`/games/${gameId}/events`);
    }
}
```

### 4. **Intégration MCTS Complexe**
- **Problem**: MCTS couplé avec logique jeu
- **Solution**: Interface AIPlayer abstraite

```rust
pub trait AIPlayer {
    async fn make_move(&self, game_state: &GameState) -> Result<Move, AIError>;
}

pub struct MCTSPlayer {
    policy_net: Arc<PolicyNet>,
    value_net: Arc<ValueNet>,
    config: MCTSConfig,
}
```

---

## 🟡 MOYENNES PRIORITÉS (Investissements futurs)

### 1. **Configuration Centralisée**
- **Problem**: Valeurs hardcodées (`c_puct = 4.2`, `total_turns = 19`)
- **Solution**: Configuration TOML/YAML avec validation

### 2. **Observabilité Structurée**
- **Problem**: Logs mélangés FR/EN, pas de métriques
- **Solution**: Structured logging + métriques Prometheus

### 3. **Persistence Layer**
- **Problem**: État en mémoire uniquement
- **Solution**: Couche persistance avec SQLite/PostgreSQL

---

## 🟢 BASSES PRIORITÉS (Nice-to-have)

### 1. **Documentation & i18n**
- Documentation API manquante
- Internationalisation

### 2. **Tooling Développement**
- Formatage automatique
- Benchmarking suite

---

## 🚀 PLAN D'IMPLÉMENTATION

### **Phase 1: Fondations** (4-6 semaines)
- [ ] **Extraire MCTS en service séparé** 
- [ ] **Implémenter hiérarchie d'erreurs propre**
- [ ] **Créer infrastructure de tests solide**
- [ ] **Diviser game_service.rs en modules focalisés** ⭐ **DÉBUT ICI**

### **Phase 2: Architecture** (6-8 semaines)
- [ ] **Event sourcing pour état de jeu**
- [ ] **WebSocket pour synchronisation temps réel**
- [ ] **Couche de persistance découplée**
- [ ] **Injection de dépendances propre**

### **Phase 3: Performance** (4-6 semaines)
- [ ] **MCTS asynchrone avec queues**
- [ ] **Caching intelligent des calculs coûteux**
- [ ] **Optimisation mémoire (moins de clones)**

### **Phase 4: Production** (2-3 semaines)
- [ ] **Monitoring et alerting complets**
- [ ] **Documentation développeur**
- [ ] **Pipeline CI/CD automatisé**

---

## 📈 **IMPACT ATTENDU**

| Métrique | Avant | Après | Amélioration |
|----------|-------|-------|--------------|
| **Maintenabilité** | 3/10 | 8/10 | +167% |
| **Couverture Tests** | 5% | 80% | +1500% |
| **Bugs Production** | Élevé | Faible | -70% |
| **Performance** | Baseline | Optimisé | +50% |
| **Temps Développement** | Lent | Rapide | +200% |

---

## 🎯 **PROCHAINE ÉTAPE**

**COMMENCER PAR**: Découpage de `game_service.rs` en modules focalisés  
**OBJECTIF**: Réduire complexité de 1,111 → ~200 lignes par module  
**DURÉE ESTIMÉE**: 3-5 jours  
**BÉNÉFICE IMMÉDIAT**: Code plus lisible et testable  

---

*Dernière mise à jour: 2025-09-01*