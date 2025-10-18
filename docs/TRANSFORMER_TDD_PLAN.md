# Plan d'Implémentation TDD des Transformers

## 1. Objectif Final
Intégrer un Transformer léger dans le MCTS existant pour améliorer les prédictions de politique et de valeur.

## 2. Graphe Mikado Initial

```
But Final : MCTS guidé par Transformer
├── Transformer minimal fonctionnel
│   ├── Couche d'attention de base
│   │   ├── Calcul des scores d'attention
│   │   ├── Softmax sur les scores
│   │   └── Multiplication avec les valeurs
│   ├── Encodage des états du jeu
│   │   ├── Représentation du plateau
│   │   ├── Encodage des tuiles restantes
│   │   └── Positional encoding
│   └── Forward pass complet
├── Intégration MCTS
│   ├── Prédiction de politique
│   ├── Prédiction de valeur
│   └── Modification du calcul UCB
└── Pipeline d'entraînement
    ├── Génération de données
    ├── Boucle d'entraînement
    └── Évaluation des performances
```

## 3. Approche TDD par Phases

### Phase 1 : Couche d'Attention (RED)
```rust
#[test]
fn test_attention_scores() {
    let attention = AttentionLayer::new(64, 2);
    let query = Tensor::rand(&[1, 4, 64], (Kind::Float, Device::Cpu));
    let key = Tensor::rand(&[1, 4, 64], (Kind::Float, Device::Cpu));
    let value = Tensor::rand(&[1, 4, 64], (Kind::Float, Device::Cpu));
    
    let scores = attention.compute_scores(&query, &key);
    assert_eq!(scores.size(), &[1, 4, 4]);
}
```

### Phase 2 : Encodage d'État (RED)
```rust
#[test]
fn test_game_state_encoding() {
    let encoder = GameStateEncoder::new(64);
    let state = GameState {
        plateau: create_test_plateau(),
        remaining_tiles: vec![Tile(1), Tile(5)],
        move_history: vec![(Tile(9), 7)],
    };
    
    let encoded = encoder.encode(&state);
    assert_eq!(encoded.size(), &[1, 19, 64]); // 19 positions
}
```

### Phase 3 : Prédictions (RED)
```rust
#[test]
fn test_transformer_predictions() {
    let transformer = TransformerModel::new(64, 2, 2);
    let state = create_test_game_state();
    
    let (policy, value) = transformer.predict(&state);
    assert_eq!(policy.size(), &[1, 19]); // proba pour chaque position
    assert_eq!(value.size(), &[1, 1]);
}
```

## 4. Plan d'Implémentation Incrémental

### Étape 1 : Structure de Base
```rust
// tests/transformer/mod.rs
mod attention_tests;
mod encoding_tests;
mod transformer_tests;

// src/neural/transformer/
mod attention.rs
mod encoding.rs
mod model.rs
```

### Étape 2 : Cycle TDD pour Attention
1. Test : calcul des scores d'attention
2. Test : softmax sur les scores
3. Test : multiplication avec les valeurs
4. Test : multi-head attention

### Étape 3 : Cycle TDD pour Encodage
1. Test : encodage du plateau
2. Test : encodage des tuiles restantes
3. Test : positional encoding
4. Test : encodage complet

### Étape 4 : Cycle TDD pour Prédictions
1. Test : forward pass
2. Test : prédiction de politique
3. Test : prédiction de valeur
4. Test : intégration MCTS

## 5. Commandes de Validation

À chaque étape :
```bash
cargo check  # Vérification des types
cargo test transformer -- --nocapture  # Tests spécifiques
cargo clippy  # Analyse statique
```

## 6. Points d'Attention

### Performances
- Profiler chaque composant séparément
- Benchmarker l'impact sur le MCTS
- Optimiser les calculs tensoriels

### Sécurité Mémoire
- Vérifier les fuites avec ASAN
- Valider les accès tensoriels
- Gérer proprement les resources GPU/CPU

### Testabilité
- Tests unitaires pour chaque composant
- Tests d'intégration pour l'ensemble
- Tests de performance avec criterion.rs
