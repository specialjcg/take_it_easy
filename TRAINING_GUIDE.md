# Guide d'entraînement du Transformer - Take It Easy

## 📊 Données disponibles

Vous avez **608 exemples d'entraînement** prêts à utiliser :

### Fichiers de données:
- `game_data_states_transformer.pt`: États de jeu (608 × 1 × 5 × 47 × 1) - 560 KB
- `game_data_policy_raw_transformer.pt`: Politiques MCTS brutes (608 × 19) - 47 KB
- `game_data_policy_boosted_transformer.pt`: Politiques MCTS boostées (608 × 19) - 47 KB
- `game_data_boosts_transformer.pt`: Intensités de boost (608 × 1) - 4.6 KB
- `game_data_positions_transformer.pt`: Positions jouées (608 × 1) - 6.8 KB
- `game_data_subscores_transformer.pt`: Sous-scores (608 × 1) - 4.7 KB

### Historique:
- **632 parties** enregistrées dans `results.csv` (scores: 126-148)
- **100 epochs** d'entraînement déjà effectués (voir `transformer_training_history.csv`)
- Score Transformer actuel: ~11.35 (vs baseline MCTS: 142.85)

## 🚀 Commandes pour entraîner

### 1. Vérifier les données
```bash
cargo run --release --bin inspect_pt
```

### 2. Entraîner le Transformer (si le trainer existe)
```bash
# Option A: Via le binaire principal
cargo run --release --bin take_it_easy -- --mode train-transformer --epochs 100

# Option B: Via un example dédié (si créé)
cargo run --release --example train_transformer

# Option C: Continuer l'entraînement existant
cargo run --release --bin take_it_easy -- --mode train-transformer --epochs 50 --resume
```

### 3. Générer plus de données (optionnel)
```bash
# Générer 200 parties supplémentaires
cargo run --release --bin take_it_easy -- --mode autotest --num-games 200 --num-simulations 150

# Monitoring en temps réel
tail -f training.log
```

### 4. Valider les performances

#### Test rapide (5 parties, ~5 min)
```bash
cargo test test_quick_validation \
  --test transformer_validation_quick_test \
  --release -- --ignored --nocapture
```

#### Test complet (20 parties, ~30 min)
```bash
cargo test test_transformer_vs_baseline_validation \
  --test transformer_validation_test \
  --release -- --ignored --nocapture
```

## 📈 Objectifs

**Baseline actuelle**: ~143 points (avec boosts MCTS)
**Transformer non-entraîné**: ~11 points
**Objectif**: >140 points avec le Transformer

## 🎯 Étapes recommandées

1. **Vérifier que l'entraînement fonctionne** (priorité haute)
2. **Entraîner 100-200 epochs supplémentaires**
3. **Valider avec le test rapide**
4. **Si bon: générer plus de données et ré-entraîner**
5. **Validation finale avec le test complet**

## 📝 Notes

- Les données sont au format PyTorch (`.pt`)
- Le Transformer a 3 têtes: policy, value, boost
- L'hybride combine Transformer + MCTS avec α=0.5
- Le boost prédit quelles positions sont boostées par les heuristiques

