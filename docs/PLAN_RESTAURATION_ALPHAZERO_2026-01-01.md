# Plan de Restauration AlphaZero - 2026-01-01

## État des Lieux

### Problèmes Identifiés
1. **Réseau Non Fonctionnel**
   - Policy loss = 2.9444 (uniforme, ln(19))
   - Value learning limité
   - Performance actuelle : ~80 pts (provient de MCTS + Rollouts, pas du réseau)

2. **Architecture Incomplète**
   - ResNet blocks désactivés pour Policy Network (POLICY_STAGE_CHANNELS = &[])
   - Architecture très simple : conv1 → GN → LeakyReLU → policy_conv
   - Capacité d'apprentissage limitée

3. **Circular Learning Problem**
   - Réseau uniforme → MCTS génère données uniformes → Training ne change rien
   - Besoin de Dirichlet noise pour casser le cycle

4. **Training Prématuré**
   - Convergence après 3 itérations seulement
   - Pas assez de données (20 games/iter)
   - Threshold de convergence trop strict (2.0 pts)

### Ce Qui Fonctionne
- ✅ MCTS UCT algorithm (~80 pts)
- ✅ Rollouts et heuristics
- ✅ Infrastructure AlphaGo Zero (self-play, benchmark, convergence)
- ✅ ResNet blocks implémentés (juste désactivés)
- ✅ Separation Policy/Value networks
- ✅ Value network avec 6 ResNet blocks

---

## Objectif : Architecture AlphaZero Stable

### Architecture Cible

#### Policy Network (avec ResNet)
```
Input [8, 5, 5]
    ↓
Conv1 (8 → 128 channels) + GroupNorm + LeakyReLU
    ↓
ResNet Block 1 (128 → 128)
    ↓
ResNet Block 2 (128 → 128)
    ↓
ResNet Block 3 (128 → 96)
    ↓
Policy Head Conv (96 → 1) + Flatten → [19 logits]
```

#### Value Network (avec ResNet)
```
Input [8, 5, 5]
    ↓
Conv1 (8 → 160 channels) + GroupNorm + LeakyReLU
    ↓
ResNet Block 1 (160 → 160)
    ↓
ResNet Block 2 (160 → 128)
    ↓
ResNet Block 3 (128 → 128)
    ↓
ResNet Block 4 (128 → 96)
    ↓
ResNet Block 5 (96 → 96)
    ↓
ResNet Block 6 (96 → 64)
    ↓
Value Head: Flatten → FC(1600 → 256) → FC(256 → 1) + Tanh → [-1, 1]
```

### Différences avec Architecture Actuelle

| Aspect | Actuel | Nouveau |
|--------|--------|---------|
| Policy ResNet blocks | 0 | 3 |
| Policy channels | 160 → 1 | 128 → 128 → 96 → 1 |
| Value ResNet blocks | 6 | 6 (inchangé) |
| Total layers | Simple | Deep (AlphaZero-like) |

---

## Plan d'Action en 4 Phases

### Phase 1 : Restaurer Architecture AlphaZero (PRIORITÉ HAUTE) ⭐⭐⭐⭐⭐

**Objectif** : Ajouter ResNet blocks au Policy Network

**Fichier à modifier** : `src/neural/policy_value_net.rs`

**Changements** :
```rust
// Ligne 112-115 (actuel)
const INITIAL_CONV_CHANNELS: i64 = 160;
const POLICY_STAGE_CHANNELS: &[i64] = &[]; // No ResNet blocks

// NOUVEAU (AlphaZero-like)
const INITIAL_CONV_CHANNELS: i64 = 128;
const POLICY_STAGE_CHANNELS: &[i64] = &[128, 128, 96]; // 3 ResNet blocks
```

**Impact** :
- Augmente capacité d'apprentissage du Policy Network
- Architecture plus proche d'AlphaZero original
- Permet d'apprendre des patterns géométriques complexes

**Validation** :
```bash
cargo build --release
./target/release/test_network_forward  # Vérifier forward pass
./target/release/test_gradient_flow    # Vérifier gradient flow
```

**Temps estimé** : 15 minutes

---

### Phase 2 : Implémenter Dirichlet Noise (PRIORITÉ HAUTE) ⭐⭐⭐⭐⭐

**Objectif** : Briser le circular learning avec exploration forcée

**Fichier à modifier** : `src/bin/alphago_zero_trainer.rs`

**Implémentation** :

1. **Ajouter dépendance `rand_distr`** dans `Cargo.toml` :
```toml
rand_distr = "0.4"
```

2. **Ajouter Dirichlet noise à la racine MCTS** (ligne ~248) :
```rust
// Add Dirichlet noise to root policy (AlphaGo Zero technique)
use rand_distr::{Dirichlet, Distribution};

let epsilon = 0.25;  // Mix ratio: 75% policy + 25% noise
let alpha = 0.3;     // Dirichlet concentration (lower = more uniform)

// Generate Dirichlet noise
let dirichlet = Dirichlet::new_with_size(alpha, 19).unwrap();
let noise: Vec<f64> = dirichlet.sample(&mut rng);

// Create noisy policy for root exploration
let mut noisy_priors = vec![0.0; 19];
for i in 0..19 {
    noisy_priors[i] = (1.0 - epsilon) * policy_priors[i] + epsilon * noise[i];
}

// Pass noisy_priors to MCTS instead of raw policy_priors
```

3. **Modifier signature MCTS** pour accepter priors externes :
```rust
// Dans src/mcts/algorithm.rs
pub fn mcts_find_best_position_for_tile_uct(
    // ... existing params
    root_noise: Option<Vec<f64>>,  // NEW parameter
) -> MCTSResult {
    // Use root_noise if provided, else use policy network
}
```

**Impact** :
- Force l'exploration de positions non-uniformes
- Génère des données variées pour training
- Casse le circular learning

**Validation** :
```bash
./target/release/alphago_zero_trainer --games-per-iter 10 --iterations 5
# Vérifier: policy_loss devrait commencer à descendre (< 2.90)
```

**Temps estimé** : 1-2 heures

---

### Phase 3 : Relancer Training AlphaGo Zero (PRIORITÉ HAUTE) ⭐⭐⭐⭐⭐

**Objectif** : Training long avec bons paramètres

**Configuration Recommandée** :
```bash
cargo build --release

./target/release/alphago_zero_trainer \
  --iterations 50 \
  --games-per-iter 100 \
  --mcts-simulations 200 \
  --epochs-per-iter 15 \
  --learning-rate 0.01 \
  --batch-size 32 \
  --convergence-threshold 5.0 \
  --output training_history_alphazero_stable.csv
```

**Paramètres Clés** :
- `games-per-iter: 100` (était 20) → 5× plus de données
- `iterations: 50` (était 20) → Training long
- `epochs-per-iter: 15` (était 10) → Plus d'entraînement par itération
- `convergence-threshold: 5.0` (était 2.0) → Évite convergence prématurée

**Métriques de Succès** :
```
Itération 1-5:  policy_loss devrait commencer à descendre
Itération 10:   policy_loss < 2.80
Itération 20:   policy_loss < 2.50, score > 100 pts
Itération 50:   policy_loss < 2.00, score > 130 pts
```

**Temps estimé** : 8-12 heures de compute (laisser tourner overnight)

---

### Phase 4 : Fine-Tuning et Optimisation (PRIORITÉ MOYENNE) ⭐⭐⭐

**Objectifs Secondaires** :

#### 4.1 Temperature-Based Sampling
```rust
// Dans alphago_zero_trainer.rs, après MCTS selection
let temperature = if iteration < 10 { 1.0 } else { 0.5 };
let selected_position = sample_position_with_temperature(&visit_counts, temperature);
```

#### 4.2 Learning Rate Schedule
```rust
let current_lr = match iteration {
    0..=10 => 0.01,   // Early: fast learning
    11..=30 => 0.005, // Mid: stabilization
    _ => 0.001,       // Late: fine-tuning
};
manager.set_learning_rate(current_lr);
```

#### 4.3 Augmenter Rollout Count
```rust
// Dans src/mcts/algorithm.rs ligne 1547
let rollout_count = 15;  // était 5, augmente qualité des évaluations
```

#### 4.4 Experience Replay Buffer
- Garder les N meilleures games en mémoire
- Réentraîner sur mix de nouvelles + anciennes données
- Évite catastrophic forgetting

**Temps estimé** : 2-3 jours

---

## Roadmap de Mise en Œuvre

### Session 1 (Aujourd'hui - 2h)
```bash
# 1. Phase 1: Restaurer ResNet blocks
# Modifier src/neural/policy_value_net.rs
const POLICY_STAGE_CHANNELS: &[i64] = &[128, 128, 96];

# 2. Rebuild et tests
cargo build --release
./target/release/test_network_forward
./target/release/test_gradient_flow
```

### Session 2 (Aujourd'hui - 2h)
```bash
# 3. Phase 2: Implémenter Dirichlet noise
# Modifier Cargo.toml, alphago_zero_trainer.rs, mcts/algorithm.rs

# 4. Test rapide (10 games, 5 iterations)
./target/release/alphago_zero_trainer \
  --games-per-iter 10 \
  --iterations 5 \
  --output test_dirichlet.csv
```

### Session 3 (Overnight - 8-12h)
```bash
# 5. Phase 3: Training long AlphaGo Zero
./target/release/alphago_zero_trainer \
  --iterations 50 \
  --games-per-iter 100 \
  --mcts-simulations 200 \
  --epochs-per-iter 15 \
  --convergence-threshold 5.0 \
  --output training_history_alphazero_stable.csv
```

### Session 4 (Lendemain - 4h)
```bash
# 6. Analyser résultats
cat training_history_alphazero_stable.csv

# 7. Benchmark final
./target/release/compare_batch_vs_uct

# 8. Phase 4 si nécessaire (fine-tuning)
```

---

## Métriques de Validation

### Critères de Succès par Phase

**Phase 1 (Architecture)** ✅
- [ ] Build réussit sans erreurs
- [ ] test_network_forward passe
- [ ] test_gradient_flow montre gradients non-nuls

**Phase 2 (Dirichlet Noise)** ✅
- [ ] Training démarre sans crash
- [ ] policy_loss commence à varier (pas constant à 2.9444)
- [ ] Logs montrent exploration variée

**Phase 3 (Training Long)** ✅
- [ ] Iteration 10: policy_loss < 2.80
- [ ] Iteration 20: policy_loss < 2.50, score > 100 pts
- [ ] Iteration 50: policy_loss < 2.00, score > 130 pts
- [ ] Pas de convergence prématurée avant iteration 30+

**Phase 4 (Fine-Tuning)** ✅
- [ ] Score final > 140 pts
- [ ] Variance réduite (std < 25 pts)
- [ ] Stable sur 100 games

---

## Commandes de Référence Rapide

### Build
```bash
cargo build --release
```

### Tests Architecture
```bash
./target/release/test_network_forward
./target/release/test_gradient_flow
./target/release/test_simple_policy
```

### Training Court (Test)
```bash
./target/release/alphago_zero_trainer \
  --games-per-iter 10 \
  --iterations 5 \
  --output test.csv
```

### Training Long (Production)
```bash
./target/release/alphago_zero_trainer \
  --iterations 50 \
  --games-per-iter 100 \
  --mcts-simulations 200 \
  --epochs-per-iter 15 \
  --convergence-threshold 5.0 \
  --output training_history_alphazero_stable.csv
```

### Benchmark
```bash
./target/release/compare_batch_vs_uct
./target/release/benchmark_baseline_cnn
```

### Monitoring
```bash
tail -f training_history_alphazero_stable.csv
```

---

## Références

### Documents Clés
- `ROADMAP_2025.md` : Vision long terme
- `ROADMAP_PERFORMANCE_2025-12-29.md` : Plan détaillé avec Dirichlet noise
- `FINAL_CONCLUSION_2025-12-27.md` : Diagnostic problème actuel
- `ALPHAGO_TRAINING_RESULTS_2025-12-27.md` : Résultats précédents
- `.claude/codex_rust_prompts.txt` : Guidelines architecture Rust

### Papers
- Silver et al. (2017) - AlphaGo Zero
- Schrittwieser et al. (2020) - MuZero
- AlphaGo Zero: Dirichlet noise α=0.3, ε=0.25

---

## Checklist Avant Implémentation

- [ ] Backup model_weights actuel
- [ ] Backup training_history.csv
- [ ] Git commit état actuel
- [ ] Lire ce document en entier
- [ ] Préparer machine pour training overnight

---

**Créé** : 2026-01-01
**Auteur** : Claude Code + Jean-Charles GOULEAU
**Baseline actuelle** : ~80 pts (MCTS pur, réseau non fonctionnel)
**Objectif Phase 3** : 130-150 pts (réseau AlphaZero stable)
**Objectif Final** : 150-180 pts (avec fine-tuning)