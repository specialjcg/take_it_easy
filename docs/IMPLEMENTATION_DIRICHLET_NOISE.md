# Implementation Guide: Dirichlet Noise

**Priorité:** #1 (Highest Impact)
**Impact attendu:** +15-30 pts
**Difficulté:** Moyenne
**Temps:** 2-3h

---

## 🎯 Objectif

Ajouter du bruit Dirichlet à la politique racine pendant le self-play pour briser la boucle d'apprentissage circulaire (policy uniforme → données uniformes → policy uniforme).

## 📚 Contexte Théorique

### AlphaGo Zero Approach
```
P'(a) = (1 - ε) * P(a) + ε * η_a

où:
- P(a) = probabilité de la policy network
- η ~ Dirichlet(α) = bruit aléatoire
- ε = 0.25 = ratio de mélange (25% bruit)
- α = 0.3 = paramètre de concentration
```

### Pourquoi ça marche
- **Avec policy uniforme:** P(a) = 1/19 pour toutes positions
- **Avec bruit:** P'(a) varie, forçant exploration différenciée
- **UCT explore différemment** → données non-uniformes → policy apprend!

---

## 🔧 Implémentation Étape par Étape

### Étape 1: Ajouter dépendance Rust

**Fichier:** `Cargo.toml`

```toml
[dependencies]
# ... dépendances existantes ...
rand_distr = "0.4"
```

Puis:
```bash
cargo build --release
```

---

### Étape 2: Modifier alphago_zero_trainer.rs

**Fichier:** `src/bin/alphago_zero_trainer.rs`

#### 2.1 Ajouter imports (ligne ~10)
```rust
use rand_distr::{Dirichlet, Distribution};
```

#### 2.2 Ajouter paramètre CLI (ligne ~75)
```rust
/// Dirichlet noise epsilon (exploration mix ratio)
#[arg(long, default_value_t = 0.25)]
dirichlet_epsilon: f64,

/// Dirichlet alpha (concentration parameter)
#[arg(long, default_value_t = 0.3)]
dirichlet_alpha: f64,
```

#### 2.3 Passer paramètres à generate_self_play_games (ligne ~150)
```rust
let training_data = generate_self_play_games(
    &manager,
    args.games_per_iter,
    args.mcts_simulations,
    args.seed + iteration as u64,
    args.dirichlet_epsilon,  // NOUVEAU
    args.dirichlet_alpha,    // NOUVEAU
)?;
```

#### 2.4 Modifier signature de generate_self_play_games (ligne ~225)
```rust
fn generate_self_play_games(
    manager: &NeuralManager,
    num_games: usize,
    mcts_sims: usize,
    seed: u64,
    dirichlet_epsilon: f64,  // NOUVEAU
    dirichlet_alpha: f64,    // NOUVEAU
) -> Result<Vec<TrainingExample>, Box<dyn std::error::Error>> {
```

#### 2.5 Générer et appliquer le bruit (ligne ~248, AVANT l'appel MCTS)
```rust
// Get legal moves count for this position
let legal_moves = take_it_easy::game::plateau::get_legal_moves(&plateau);
let num_legal_moves = legal_moves.len();

// Generate Dirichlet noise
let dirichlet = Dirichlet::new_with_size(dirichlet_alpha, num_legal_moves)
    .expect("Failed to create Dirichlet distribution");
let noise: Vec<f64> = dirichlet.sample(&mut rng);

// Use UCT MCTS with Dirichlet noise
let mcts_result = mcts_find_best_position_for_tile_uct(
    &mut plateau,
    &mut deck,
    chosen_tile,
    manager.policy_net(),
    manager.value_net(),
    mcts_sims,
    turn,
    turns_per_game,
    None, // Hyperparameters
    Some((dirichlet_epsilon, &noise)),  // NOUVEAU: exploration noise
);
```

---

### Étape 3: Modifier algorithm.rs

**Fichier:** `src/mcts/algorithm.rs`

#### 3.1 Modifier signature de mcts_find_best_position_for_tile_uct (ligne ~1428)
```rust
pub fn mcts_find_best_position_for_tile_uct(
    plateau: &mut Plateau,
    deck: &mut Deck,
    chosen_tile: Tile,
    policy_net: &PolicyNet,
    value_net: &ValueNet,
    num_simulations: usize,
    current_turn: usize,
    total_turns: usize,
    hyperparams: Option<&MCTSHyperparameters>,
    exploration_noise: Option<(f64, &[f64])>,  // NOUVEAU: (epsilon, noise)
) -> MCTSResult {
```

#### 3.2 Appliquer le bruit aux priors de policy (ligne ~1495, APRÈS normalisation)
```rust
// Normalize policy probabilities (only over legal moves)
let sum: f64 = policy_vec.iter().sum();
if sum > 0.0 {
    for prob in &mut policy_vec {
        *prob /= sum;
    }
} else {
    // Uniform if all zero
    let uniform = 1.0 / legal_moves.len() as f64;
    policy_vec = vec![uniform; legal_moves.len()];
}

// NOUVEAU: Apply Dirichlet noise if provided
if let Some((epsilon, noise)) = exploration_noise {
    for (idx, prob) in policy_vec.iter_mut().enumerate() {
        *prob = (1.0 - epsilon) * (*prob) + epsilon * noise[idx];
    }
}
```

---

### Étape 4: Mettre à jour les autres appels

**Fichier:** `src/bin/alphago_zero_trainer.rs`

#### 4.1 Benchmark (ligne ~390): PAS de bruit
```rust
// Benchmark: no exploration noise (pure exploitation)
let mcts_result = mcts_find_best_position_for_tile_uct(
    &mut plateau,
    &mut deck,
    chosen_tile,
    manager.policy_net(),
    manager.value_net(),
    mcts_sims,
    turn,
    turns_per_game,
    None,
    None,  // NO noise during evaluation
);
```

---

## ✅ Checklist de Validation

### Avant Compilation
- [ ] `rand_distr` ajouté à Cargo.toml
- [ ] Imports ajoutés dans alphago_zero_trainer.rs
- [ ] Paramètres CLI ajoutés
- [ ] Signature generate_self_play_games modifiée
- [ ] Signature mcts_find_best_position_for_tile_uct modifiée
- [ ] Bruit appliqué AVANT MCTS (self-play)
- [ ] Bruit ABSENT du benchmark

### Compilation
```bash
cargo build --release
# Doit compiler sans erreurs
```

### Test Initial
```bash
# Test avec bruit faible (devrait marcher)
./target/release/alphago_zero_trainer \
  --games-per-iter 10 \
  --iterations 5 \
  --dirichlet-epsilon 0.25 \
  --dirichlet-alpha 0.3 \
  --output test_dirichlet.csv

# Vérifier test_dirichlet.csv:
# - policy_loss doit commencer à diminuer (< 2.90)
# - Pas de crash
```

---

## 📊 Validation des Résultats

### Métriques à Surveiller

```bash
cat training_history.csv
```

**Avant Dirichlet (baseline):**
```
iteration,policy_loss,value_loss,benchmark_score_mean
1,2.9444,3.9966,148.29
2,2.9444,1.5045,146.92
```

**Après Dirichlet (attendu):**
```
iteration,policy_loss,value_loss,benchmark_score_mean
1,2.9444,3.8500,149.00  ← value_loss peut rester similaire
2,2.8900,2.1000,153.50  ← policy_loss DOIT diminuer!
3,2.8200,1.5500,158.20  ← progression continue
4,2.7500,1.2000,162.00
...
```

### Critères de Succès

**SUCCÈS si:**
- ✅ policy_loss < 2.90 après iteration 2
- ✅ policy_loss < 2.80 après iteration 5
- ✅ benchmark_score > 155 pts après iteration 10

**ÉCHEC si:**
- ❌ policy_loss reste à 2.9444
- ❌ Training crash ou NaN losses
- ❌ benchmark_score < 145 pts

---

## 🐛 Troubleshooting

### Problème: policy_loss reste à 2.9444

**Causes possibles:**
1. Bruit pas appliqué (vérifier code)
2. Epsilon trop faible (essayer 0.5)
3. Alpha trop élevé (essayer 0.15)

**Solution:**
```bash
# Augmenter le bruit
./target/release/alphago_zero_trainer \
  --dirichlet-epsilon 0.5 \
  --dirichlet-alpha 0.15
```

### Problème: Losses deviennent NaN

**Cause:** Bruit trop fort ou problème de normalisation

**Solution:**
```bash
# Réduire le bruit
./target/release/alphago_zero_trainer \
  --dirichlet-epsilon 0.15 \
  --dirichlet-alpha 0.5
```

### Problème: Compilation fails "Dirichlet not found"

**Cause:** `rand_distr` pas dans Cargo.toml

**Solution:**
```bash
cargo clean
# Vérifier Cargo.toml
cargo build --release
```

---

## 🧪 Tests Recommandés

### Test 1: Baseline (sans bruit)
```bash
./target/release/alphago_zero_trainer \
  --games-per-iter 20 \
  --iterations 10 \
  --dirichlet-epsilon 0.0 \
  --output baseline_no_noise.csv
```

### Test 2: AlphaGo Zero standard
```bash
./target/release/alphago_zero_trainer \
  --games-per-iter 50 \
  --iterations 20 \
  --dirichlet-epsilon 0.25 \
  --dirichlet-alpha 0.3 \
  --output test_alphago_params.csv
```

### Test 3: High exploration
```bash
./target/release/alphago_zero_trainer \
  --games-per-iter 50 \
  --iterations 20 \
  --dirichlet-epsilon 0.5 \
  --dirichlet-alpha 0.15 \
  --output test_high_exploration.csv
```

### Comparaison
```bash
# Comparer les résultats
echo "Baseline:"
tail -1 baseline_no_noise.csv

echo "AlphaGo params:"
tail -1 test_alphago_params.csv

echo "High exploration:"
tail -1 test_high_exploration.csv
```

---

## 📈 Résultats Attendus

### Timeline Réaliste

**Iterations 1-5:** Initialisation
- policy_loss: 2.94 → 2.85
- score: 148 → 155 pts

**Iterations 6-15:** Apprentissage
- policy_loss: 2.85 → 2.60
- score: 155 → 165 pts

**Iterations 16-30:** Convergence
- policy_loss: 2.60 → 2.40
- score: 165 → 175 pts

**Iterations 30+:** Fine-tuning
- policy_loss: 2.40 → 2.20
- score: 175 → 180+ pts

---

## 🚀 Commande Finale Recommandée

Après implémentation et tests de validation:

```bash
# Long training run (overnight)
./target/release/alphago_zero_trainer \
  --games-per-iter 100 \
  --iterations 50 \
  --mcts-simulations 200 \
  --epochs-per-iter 15 \
  --dirichlet-epsilon 0.25 \
  --dirichlet-alpha 0.3 \
  --convergence-threshold 5.0 \
  --learning-rate 0.008 \
  --batch-size 64 \
  --output training_history_dirichlet_full.csv

# Laisser tourner 8-12h
# Résultat attendu: 165-175 pts
```

---

**Date:** 2025-12-29
**Status:** Ready to implement
**Next:** Suivre ce guide étape par étape
