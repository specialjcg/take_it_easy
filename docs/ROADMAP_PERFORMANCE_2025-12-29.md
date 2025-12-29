# Plan d'Amélioration des Performances - Take It Easy AI
**Date:** 2025-12-29
**Contexte:** Après implémentation UCT MCTS et AlphaGo Zero self-play
**Performance actuelle:** 146.92 pts (+97.9% vs baseline 74.22 pts)

---

## 📊 État Actuel (Baseline pour prochaine session)

### Performances Mesurées
```
Batch MCTS (original):        74.22 ± 23.68 pts
UCT MCTS (breakthrough):      149.36 ± 23.25 pts  (+101.2%)
UCT Self-Play (converged):    146.92 ± 25.42 pts  (+97.9%)
```

### Configuration Actuelle
```bash
# Self-play training settings (alphago_zero_trainer)
--iterations 10
--games-per-iter 20              # 380 examples/iteration
--mcts-simulations 150
--epochs-per-iter 10
--learning-rate 0.01
--batch-size 32
--convergence-threshold 2.0      # Trop restrictif!
```

### Métriques de Training
```
Iteration 1: policy_loss=2.9444, value_loss=3.9966, score=148.29
Iteration 2: policy_loss=2.9444, value_loss=1.5045, score=146.92
Status: CONVERGED (amélioration -1.37 < 2.0 seuil)
```

### Problème Identifié: CIRCULAR LEARNING
```
Policy Uniforme (loss=2.9444 = ln(19))
    ↓
UCT utilise priors uniformes
    ↓
Génère données uniformes
    ↓
Training → Policy Uniforme
    ↓
BOUCLE!
```

---

## 🎯 Plan d'Action Prioritisé

### PHASE 1: Quick Wins (Impact Immédiat) ⭐⭐⭐
**Objectif:** 155-170 pts
**Temps estimé:** 1-2 jours
**Complexité:** Moyenne

#### 1.1 Implémenter Dirichlet Noise (PRIORITÉ #1)
**Impact attendu:** +15-30 pts
**Pourquoi:** Brise la boucle circulaire en forçant l'exploration

**Implémentation:**
```rust
// Dans alphago_zero_trainer.rs, fonction generate_self_play_games()
// Ligne ~248, AVANT l'appel MCTS:

// Add Dirichlet noise to root policy (AlphaGo Zero technique)
let epsilon = 0.25;  // Mix ratio: 75% policy + 25% noise
let alpha = 0.3;     // Dirichlet concentration (lower = more uniform)

// Generate Dirichlet noise
use rand_distr::{Dirichlet, Distribution};
let dirichlet = Dirichlet::new_with_size(alpha, legal_moves.len()).unwrap();
let noise: Vec<f64> = dirichlet.sample(&mut rng);

// Apply noise to policy BEFORE MCTS
// Modifier mcts_find_best_position_for_tile_uct() pour accepter noisy_policy
```

**Fichier à modifier:**
- `src/bin/alphago_zero_trainer.rs` (ligne ~248)
- `src/mcts/algorithm.rs` (ajouter paramètre optional `exploration_noise`)

**Test de validation:**
```bash
# Après implémentation, lancer:
./target/release/alphago_zero_trainer \
  --games-per-iter 50 \
  --iterations 10 \
  --convergence-threshold 5.0

# Vérifier: policy_loss devrait diminuer (< 2.90)
```

---

#### 1.2 Temperature-Based Sampling
**Impact attendu:** +5-15 pts
**Pourquoi:** Crée de la diversité dans les données d'entraînement

**Implémentation:**
```rust
// Dans alphago_zero_trainer.rs, ligne ~263
// Au lieu de: plateau.tiles[mcts_result.best_position] = chosen_tile;

// Use temperature-based sampling
let temperature = if iteration < 10 { 1.0 } else { 0.5 };
let selected_position = sample_position_with_temperature(
    &visit_counts,
    temperature
);

// Ajouter fonction:
fn sample_position_with_temperature(
    visit_counts: &HashMap<usize, usize>,
    temperature: f64,
) -> usize {
    // visits_temp = visits^(1/τ)
    // P(a) = visits_temp(a) / sum(visits_temp)
    // Sample from this distribution
}
```

---

#### 1.3 Augmenter Volume de Training
**Impact attendu:** +10-20 pts
**Pourquoi:** Plus de données = meilleur apprentissage

**Commande immédiate (SANS changement de code):**
```bash
./target/release/alphago_zero_trainer \
  --games-per-iter 100 \
  --convergence-threshold 10.0 \
  --iterations 50 \
  --epochs-per-iter 15 \
  --mcts-simulations 200

# Temps: ~8-12h sur CPU
# Résultat attendu: 155-165 pts
```

---

### PHASE 2: Optimisation Qualité (Impact Moyen) ⭐⭐
**Objectif:** 165-180 pts
**Temps estimé:** 2-3 jours
**Complexité:** Moyenne-Élevée

#### 2.1 Augmenter Rollout Count
**Fichier:** `src/mcts/algorithm.rs` ligne 1547
```rust
// Actuel:
let rollout_count = 5;

// Nouveau:
let rollout_count = 15;  // 3x plus de rollouts = meilleures estimations
```

**Trade-off:** 3x plus lent, mais meilleures valeurs

---

#### 2.2 Améliorer Value Normalization
**Fichier:** `src/mcts/algorithm.rs` ligne 1562
```rust
// Actuel (assume max=200):
let normalized_value = ((rollout_value / 200.0).clamp(0.0, 1.0) * 2.0) - 1.0;

// Nouveau (adaptatif basé sur performance actuelle ~147):
let normalized_value = ((rollout_value - 80.0) / 70.0).clamp(-1.0, 1.0);
// Centre à 80, range ±70 pour couvrir [10, 150]
```

---

#### 2.3 Learning Rate Schedule
**Fichier:** `src/bin/alphago_zero_trainer.rs`
```rust
// Ajouter dans la boucle d'itération (ligne ~143):
let current_lr = if iteration < 10 {
    0.01  // Début: learning rapide
} else if iteration < 30 {
    0.005  // Milieu: stabilisation
} else {
    0.001  // Fin: fine-tuning
};

// Update optimizer learning rate
manager.set_learning_rate(current_lr);
```

---

### PHASE 3: Architecture & Algorithmes (Long Terme) ⭐
**Objectif:** 180-200+ pts
**Temps estimé:** 1-2 semaines
**Complexité:** Élevée

#### 3.1 Essayer Architecture GNN
```bash
# L'architecture GNN existe déjà dans le code!
./target/release/alphago_zero_trainer \
  --nn-architecture GNN \
  --games-per-iter 100 \
  --iterations 50

# GNN peut capturer mieux les relations spatiales
```

#### 3.2 Réseau Plus Profond
**Fichier:** `src/neural/policy_value_net.rs`
- Ajouter ResNet blocks
- Passer de 3 conv layers à 5-7
- Ajouter batch normalization

#### 3.3 Experience Replay Buffer
- Garder les N meilleures games en mémoire
- Réentraîner sur mix de nouvelles + anciennes données
- Évite l'oubli catastrophique

#### 3.4 Progressive Widening
**Note:** Code existe déjà dans `src/mcts/progressive_widening.rs`!
- À intégrer avec UCT
- Peut améliorer early-game exploration

---

## 🚀 Plan d'Exécution Recommandé

### Session 1 (2-3 heures): Quick Test
```bash
# 1. Test immédiat SANS code change
cargo build --release

# 2. Lancer training étendu
./target/release/alphago_zero_trainer \
  --games-per-iter 100 \
  --convergence-threshold 10.0 \
  --iterations 50 \
  --mcts-simulations 200 \
  --epochs-per-iter 15 \
  --output training_history_extended.csv

# 3. Laisser tourner overnight
# 4. Analyser résultats le lendemain
```

### Session 2 (4-6 heures): Dirichlet Noise
```bash
# 1. Implémenter Dirichlet noise (1.1)
# 2. Implémenter temperature sampling (1.2)
# 3. Tester sur 20 iterations
# 4. Comparer avec baseline

# Commandes de test:
cargo build --release
./target/release/alphago_zero_trainer \
  --games-per-iter 100 \
  --iterations 20 \
  --output training_history_with_noise.csv
```

### Session 3 (2-4 heures): Fine-Tuning
```bash
# 1. Implémenter rollout count increase (2.1)
# 2. Améliorer value normalization (2.2)
# 3. Ajouter learning rate schedule (2.3)
# 4. Long training run (50-100 iterations)
```

---

## 📈 Métriques de Succès

### Objectifs par Phase
```
Phase 1 Réussie si:
  - policy_loss < 2.80 (commence à apprendre!)
  - value_loss < 1.00
  - benchmark_score > 160 pts

Phase 2 Réussie si:
  - policy_loss < 2.50
  - value_loss < 0.70
  - benchmark_score > 170 pts

Phase 3 Réussie si:
  - policy_loss < 2.00
  - value_loss < 0.50
  - benchmark_score > 185 pts
```

### Commandes de Validation
```bash
# Benchmark current model
MODEL_PATH=model_weights/cnn ./target/release/compare_batch_vs_uct

# Vérifier distribution policy
./target/release/test_uct_distribution

# Analyser training history
cat training_history.csv
# Regarder évolution policy_loss et value_loss
```

---

## 📁 Fichiers Clés à Modifier

### Priorité HAUTE (Phase 1)
```
src/bin/alphago_zero_trainer.rs
  - Ligne 248: Ajouter Dirichlet noise
  - Ligne 263: Temperature sampling
  - Ligne 33-62: Augmenter default params
```

### Priorité MOYENNE (Phase 2)
```
src/mcts/algorithm.rs
  - Ligne 1547: Rollout count
  - Ligne 1562: Value normalization

src/bin/alphago_zero_trainer.rs
  - Ligne 143: Learning rate schedule
```

### Priorité BASSE (Phase 3)
```
src/neural/policy_value_net.rs
  - Architecture réseau

src/mcts/progressive_widening.rs
  - Intégration avec UCT
```

---

## 🔍 Debug & Monitoring

### Vérifier l'Apprentissage
```bash
# Si policy_loss reste à 2.9444:
# → Dirichlet noise pas assez fort (augmenter epsilon)
# → Ou temperature trop basse

# Si value_loss diverge (augmente):
# → Learning rate trop élevé
# → Ou value normalization incorrecte

# Si benchmark stagne:
# → Pas assez de diversité dans les données
# → Augmenter games_per_iter
```

### Logs à Surveiller
```bash
tail -f selfplay_uct.log

# Chercher:
# - "policy_loss=" doit diminuer au fil des iterations
# - "value_loss=" doit diminuer et stabiliser
# - "Score:" doit augmenter progressivement
```

---

## 💾 Sauvegarder les Résultats

### Avant chaque expérience
```bash
# Backup current model
cp -r model_weights/cnn model_weights/cnn_backup_$(date +%Y%m%d_%H%M)

# Sauvegarder training history
cp training_history.csv training_history_backup_$(date +%Y%m%d_%H%M).csv
```

### Après expérience réussie
```bash
# Tag git
git tag -a v0.2.0-dirichlet -m "UCT + Dirichlet noise: XXX pts"
git push origin v0.2.0-dirichlet

# Documenter résultats
echo "Date: $(date)" >> docs/PERFORMANCE_LOG.md
echo "Config: ..." >> docs/PERFORMANCE_LOG.md
echo "Score: XXX pts" >> docs/PERFORMANCE_LOG.md
```

---

## 🎓 Références Techniques

### AlphaGo Zero Paper
- Dirichlet noise: α=0.3, ε=0.25 (Section 4.1)
- Temperature: τ=1.0 pour 30 premiers coups, puis τ→0
- MCTS: 1600 simulations par coup (nous: 150-200)

### Code Existant à Réutiliser
```bash
# Progressive Widening (déjà implémenté)
src/mcts/progressive_widening.rs

# GNN Architecture (déjà disponible)
--nn-architecture GNN

# Hyperparameters adaptifs
src/mcts/hyperparameters.rs
```

---

## ⚠️ Pièges à Éviter

1. **Ne pas augmenter learning_rate au-dessus de 0.01**
   - Cause: value_loss divergence

2. **Ne pas utiliser batch_size trop petit (<16)**
   - Cause: training instable

3. **Ne pas skip le Dirichlet noise**
   - C'est LA clé pour briser la boucle circulaire

4. **Ne pas oublier de rebuild après changement de code**
   ```bash
   cargo build --release  # Toujours en --release!
   ```

5. **Ne pas interrompre training au milieu d'une iteration**
   - Risque de corrompre les weights
   - Utiliser Ctrl+C seulement entre iterations

---

## 📞 Quick Reference Commands

```bash
# Build
cargo build --release

# Training standard (rapide, 2h)
./target/release/alphago_zero_trainer \
  --games-per-iter 50 --iterations 20

# Training long (overnight, 8-12h)
./target/release/alphago_zero_trainer \
  --games-per-iter 100 --iterations 50 \
  --convergence-threshold 10.0

# Benchmark current model
./target/release/compare_batch_vs_uct

# Vérifier policy distribution
./target/release/test_uct_distribution

# Monitor training
tail -f training_history.csv
```

---

## 🎯 Objectif Final

**Target Performance:** 180-200+ pts
**Actuel:** 146.92 pts
**Gap:** +33-53 pts (+22-36%)

**Timeline Réaliste:**
- Phase 1 (Quick Wins): +15-25 pts → **162-172 pts**
- Phase 2 (Optimisation): +10-15 pts → **172-187 pts**
- Phase 3 (Architecture): +5-15 pts → **180-200+ pts**

---

**Date de création:** 2025-12-29
**Auteur:** Jean-Charles GOULEAU
**Baseline:** UCT Self-Play 146.92 pts (commit 1846218)
**Prochaine session:** Commencer par Phase 1.1 (Dirichlet Noise)
