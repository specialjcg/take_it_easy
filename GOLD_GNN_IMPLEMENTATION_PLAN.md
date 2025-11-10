# Gold GNN + Curriculum Learning: Plan d'Implémentation

*Plan détaillé pour atteindre 145-154 pts (vs 139.40 baseline)*

---

## 🎯 Objectifs

**Primaire:** Implémenter Gold GNN avec Curriculum Learning
**Score cible:** 145-154 pts (+5-15 pts vs CNN baseline 139.40)
**Timeline:** 24-36 heures (18-20h génération données + 6-8h implémentation)

---

## 📊 État Actuel

### Ce Qui Existe Déjà ✅

1. **Infrastructure GNN** (`src/neural/gnn.rs`)
   - GraphLayer avec residual connections
   - GraphEncoder avec LayerNorm
   - GOLD_HIDDEN défini: `[256, 256, 128, 64]`
   - Adjacency matrix pour topologie hexagonale

2. **System de Benchmark** (`src/bin/compare_mcts.rs`)
   - Support CNN et GNN
   - Logs CSV automatiques
   - RNG seed fixe pour reproductibilité

3. **Baseline Établi**
   - CNN: 139.40 pts (Pattern Rollouts V2)
   - Pure MCTS: 103-116 pts
   - Tests sur 100+ parties

### Ce Qui Manque ❌

1. **Graph Attention Networks (GAT)**
   - Multi-head attention pas implémenté
   - Actuellement: simple aggregation voisins

2. **Expert Data Generator**
   - Pas de binary `expert_data_generator.rs`
   - Besoin beam search width 100/500/1000

3. **Supervised Trainer**
   - Pas de `SupervisedTrainer` struct
   - Training actuel: self-play MCTS only

4. **Curriculum Learning Pipeline**
   - Pas de 3-phase training
   - Pas de progressive loading

---

## 🗺️ Stratégie Recommandée

D'après l'analyse des docs, **2 approches possibles:**

### Approche A: Curriculum Learning D'Abord (RECOMMANDÉ)
```
1. Générer expert data (beam search)
2. Implémenter SupervisedTrainer
3. Entraîner CNN existant avec curriculum
4. Benchmark → devrait atteindre ~149-154 pts
5. (Optionnel) Puis implémenter Gold GNN

Avantages:
✅ Plus bas risque (CNN architecture prouvée)
✅ Gain principal vient des données, pas architecture
✅ Peut valider curriculum learning séparément
✅ Si succès, Gold GNN devient bonus (+2-5 pts)
```

### Approche B: Gold GNN + Curriculum Simultané
```
1. Implémenter GAT layers (multi-head attention)
2. Générer expert data en parallèle
3. Entraîner Gold GNN avec curriculum
4. Benchmark

Avantages:
✅ Gain maximal potentiel (+10-15 pts)
⚠️ Plus de risques (2 changements simultanés)
⚠️ Debugging plus complexe
```

**Recommandation:** **Approche A** puis A→B si succès

---

## 📋 Plan Détaillé (Approche A)

### Phase 1: Expert Data Generation (18-20h)

#### Task 1.1: Créer `expert_data_generator.rs` (2h)

**Fichier:** `src/bin/expert_data_generator.rs`

**Fonctionnalités:**
```rust
#[derive(Parser)]
struct Args {
    beam_width: usize,          // 100, 500, ou 1000
    num_games: usize,           // 50, 100, ou 200
    output_file: String,        // phase1_beam100.json
    seed: u64,                  // Pour reproductibilité
}

struct ExpertGame {
    moves: Vec<ExpertMove>,     // 19 moves par partie
    final_score: i32,
}

struct ExpertMove {
    turn: usize,
    plateau_state: Vec<i32>,    // 19 cells
    tile_drawn: Tile,           // Tuile tirée
    best_position: usize,       // Position choisie par beam search
    best_value: f64,            // Score attendu
    all_positions_values: HashMap<usize, f64>, // Pour policy distribution
}
```

**Algorithme:**
```rust
fn generate_expert_data(beam_width: usize, num_games: usize) -> Vec<ExpertGame> {
    let mut games = Vec::new();

    for game_num in 0..num_games {
        let mut plateau = create_plateau_empty();
        let mut deck = create_deck();
        let mut moves = Vec::new();

        // Shuffle deck avec seed
        deck.shuffle();

        for turn in 0..19 {
            // Tire une tuile
            let tile = deck.tiles[turn];

            // Beam search pour trouver meilleure position
            let (best_pos, value, all_values) = beam_search_best_move(
                &plateau,
                &deck,
                tile,
                beam_width,
                turn,
            );

            // Enregistre le move
            moves.push(ExpertMove {
                turn,
                plateau_state: plateau.tiles.clone(),
                tile_drawn: tile,
                best_position: best_pos,
                best_value: value,
                all_positions_values: all_values,
            });

            // Applique le move
            plateau.tiles[best_pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }

        games.push(ExpertGame {
            moves,
            final_score: result(&plateau),
        });

        // Log progress
        if (game_num + 1) % 10 == 0 {
            println!("Generated {}/{} games", game_num + 1, num_games);
        }
    }

    games
}
```

**Beam Search Implementation:**
```rust
fn beam_search_best_move(
    plateau: &Plateau,
    deck: &Deck,
    tile: Tile,
    beam_width: usize,
    turn: usize,
) -> (usize, f64, HashMap<usize, f64>) {
    // 1. Obtenir positions légales
    let legal_positions = get_legal_moves(plateau.clone());

    // 2. Pour chaque position, simuler beam_width séquences futures
    let mut position_scores: HashMap<usize, Vec<i32>> = HashMap::new();

    for &pos in &legal_positions {
        let mut scores = Vec::new();

        // Simule beam_width futurs possibles
        for _ in 0..beam_width {
            let mut sim_plateau = plateau.clone();
            let mut sim_deck = deck.clone();

            // Place la tuile actuelle
            sim_plateau.tiles[pos] = tile;
            sim_deck = replace_tile_in_deck(&sim_deck, &tile);

            // Simule les (19 - turn - 1) coups restants avec beam search réduit
            let final_score = simulate_future_with_beam(
                &sim_plateau,
                &sim_deck,
                turn + 1,
                beam_width / 10, // Beam réduit pour profondeur
            );

            scores.push(final_score);
        }

        position_scores.insert(pos, scores);
    }

    // 3. Calculer moyenne et variance pour chaque position
    let mut position_values: HashMap<usize, f64> = HashMap::new();
    for (pos, scores) in &position_scores {
        let avg = scores.iter().sum::<i32>() as f64 / scores.len() as f64;
        position_values.insert(*pos, avg);
    }

    // 4. Sélectionner meilleure position
    let best_pos = *position_values
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap()
        .0;

    let best_value = position_values[&best_pos];

    (best_pos, best_value, position_values)
}
```

**Optimisations:**
- Utiliser rayon pour parallélisation des simulations
- Cache pour états déjà évalués
- Early stopping si un coup domine clairement

#### Task 1.2: Générer Phase 1 Data (0.5h)

```bash
cargo run --release --bin expert_data_generator -- \
    --beam-width 100 \
    --num-games 50 \
    --output-file data/phase1_beam100.json \
    --seed 2025

Expected:
  - 50 games × 19 moves = 950 training examples
  - Average score: ~150-155 pts (beam 100)
  - Time: ~30 minutes
```

#### Task 1.3: Générer Phase 2 Data (3-4h)

```bash
cargo run --release --bin expert_data_generator -- \
    --beam-width 500 \
    --num-games 100 \
    --output-file data/phase2_beam500.json \
    --seed 2026

Expected:
  - 100 games × 19 moves = 1,900 examples
  - Average score: ~165-168 pts (beam 500)
  - Time: ~3-4 hours
```

#### Task 1.4: Générer Phase 3 Data (12-16h)

```bash
cargo run --release --bin expert_data_generator -- \
    --beam-width 1000 \
    --num-games 200 \
    --output-file data/phase3_beam1000.json \
    --seed 2027

Expected:
  - 200 games × 19 moves = 3,800 examples
  - Average score: ~174-175 pts (quasi-optimal)
  - Time: ~12-16 hours
```

**Total Training Examples:** 950 + 1,900 + 3,800 = **6,650 examples**

---

### Phase 2: Supervised Training Infrastructure (3-4h)

#### Task 2.1: Créer `SupervisedTrainer` (2h)

**Fichier:** `src/neural/training/supervised_trainer.rs`

```rust
pub struct SupervisedTrainer {
    policy_net: PolicyNet,
    value_net: ValueNet,
    optimizer: nn::Optimizer,
    device: Device,
}

impl SupervisedTrainer {
    pub fn new(
        vs: &nn::VarStore,
        policy_net: PolicyNet,
        value_net: ValueNet,
        learning_rate: f64,
    ) -> Self {
        let optimizer = nn::Adam::default()
            .build(vs, learning_rate)
            .unwrap();

        Self {
            policy_net,
            value_net,
            optimizer,
            device: vs.device(),
        }
    }

    pub fn train_epoch(
        &mut self,
        data: &[ExpertMove],
        batch_size: usize,
    ) -> (f64, f64) {
        let mut total_policy_loss = 0.0;
        let mut total_value_loss = 0.0;
        let mut num_batches = 0;

        // Shuffle data
        let mut rng = thread_rng();
        let mut indices: Vec<usize> = (0..data.len()).collect();
        indices.shuffle(&mut rng);

        // Mini-batch training
        for batch_start in (0..data.len()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(data.len());
            let batch_indices = &indices[batch_start..batch_end];

            // Prepare batch tensors
            let (board_tensors, policy_targets, value_targets) =
                self.prepare_batch(data, batch_indices);

            // Forward pass
            let policy_logits = self.policy_net.forward(&board_tensors, true);
            let value_preds = self.value_net.forward(&board_tensors, true);

            // Policy loss: Cross-entropy
            let policy_loss = policy_logits
                .log_softmax(-1, Kind::Float)
                .nll_loss(&policy_targets);

            // Value loss: MSE
            let value_loss = (value_preds - value_targets).pow_tensor_scalar(2).mean(Kind::Float);

            // Combined loss
            let total_loss = policy_loss + value_loss;

            // Backward pass
            self.optimizer.zero_grad();
            total_loss.backward();
            self.optimizer.step();

            // Track metrics
            total_policy_loss += policy_loss.double_value(&[]);
            total_value_loss += value_loss.double_value(&[]);
            num_batches += 1;
        }

        (
            total_policy_loss / num_batches as f64,
            total_value_loss / num_batches as f64,
        )
    }

    fn prepare_batch(
        &self,
        data: &[ExpertMove],
        indices: &[usize],
    ) -> (Tensor, Tensor, Tensor) {
        // Convert expert moves to tensors
        // ...implementation...
    }
}
```

#### Task 2.2: Implémenter Curriculum Loading (1h)

**Fichier:** `src/neural/training/curriculum.rs`

```rust
pub struct CurriculumDataLoader {
    phases: Vec<CurriculumPhase>,
    current_phase: usize,
}

pub struct CurriculumPhase {
    name: String,
    data_file: PathBuf,
    num_epochs: usize,
    learning_rate: f64,
    batch_size: usize,
}

impl CurriculumDataLoader {
    pub fn new() -> Self {
        let phases = vec![
            CurriculumPhase {
                name: "Phase 1: Beam 100".to_string(),
                data_file: PathBuf::from("data/phase1_beam100.json"),
                num_epochs: 50,
                learning_rate: 1e-3,
                batch_size: 32,
            },
            CurriculumPhase {
                name: "Phase 2: Beam 500".to_string(),
                data_file: PathBuf::from("data/phase2_beam500.json"),
                num_epochs: 30,
                learning_rate: 5e-4, // Réduit pour fine-tuning
                batch_size: 32,
            },
            CurriculumPhase {
                name: "Phase 3: Beam 1000".to_string(),
                data_file: PathBuf::from("data/phase3_beam1000.json"),
                num_epochs: 20,
                learning_rate: 1e-4, // Très réduit
                batch_size: 32,
            },
        ];

        Self {
            phases,
            current_phase: 0,
        }
    }

    pub fn next_phase(&mut self) -> Option<&CurriculumPhase> {
        if self.current_phase < self.phases.len() {
            let phase = &self.phases[self.current_phase];
            self.current_phase += 1;
            Some(phase)
        } else {
            None
        }
    }
}
```

#### Task 2.3: Binary d'Entraînement (1h)

**Fichier:** `src/bin/train_curriculum.rs`

```rust
#[derive(Parser)]
struct Args {
    /// Architecture (cnn or gnn)
    #[arg(long, default_value = "cnn")]
    architecture: String,

    /// Start from phase (1, 2, or 3)
    #[arg(long, default_value_t = 1)]
    start_phase: usize,

    /// Model checkpoint to load (optional, for resuming)
    #[arg(long)]
    checkpoint: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Initialize neural network
    let vs = nn::VarStore::new(Device::cuda_if_available());
    let (policy_net, value_net) = match args.architecture.as_str() {
        "cnn" => load_cnn_architecture(&vs),
        "gnn" => load_gnn_architecture(&vs), // Gold GNN
        _ => panic!("Unknown architecture"),
    };

    // Load checkpoint if resuming
    if let Some(checkpoint_path) = args.checkpoint {
        vs.load(&checkpoint_path)?;
        println!("Loaded checkpoint: {}", checkpoint_path);
    }

    // Initialize trainer
    let mut trainer = SupervisedTrainer::new(
        &vs,
        policy_net,
        value_net,
        1e-3, // Initial LR, will be updated per phase
    );

    // Load curriculum phases
    let mut curriculum = CurriculumDataLoader::new();

    // Skip to start_phase if specified
    for _ in 1..args.start_phase {
        curriculum.next_phase();
    }

    // Train each phase
    while let Some(phase) = curriculum.next_phase() {
        println!("\n========================================");
        println!("Starting {}", phase.name);
        println!("========================================");

        // Load phase data
        let data = load_expert_data(&phase.data_file)?;
        println!("Loaded {} training examples", data.len());

        // Update learning rate
        trainer.set_learning_rate(phase.learning_rate);

        // Train for num_epochs
        for epoch in 0..phase.num_epochs {
            let (policy_loss, value_loss) = trainer.train_epoch(&data, phase.batch_size);

            println!(
                "Epoch {}/{}: policy_loss={:.4}, value_loss={:.4}",
                epoch + 1,
                phase.num_epochs,
                policy_loss,
                value_loss
            );

            // Save checkpoint every 10 epochs
            if (epoch + 1) % 10 == 0 {
                let checkpoint_name = format!(
                    "checkpoints/{}_phase{}_epoch{}.ot",
                    args.architecture,
                    curriculum.current_phase,
                    epoch + 1
                );
                vs.save(&checkpoint_name)?;
                println!("Saved checkpoint: {}", checkpoint_name);
            }
        }

        // Save final phase checkpoint
        let phase_checkpoint = format!(
            "checkpoints/{}_phase{}_final.ot",
            args.architecture,
            curriculum.current_phase
        );
        vs.save(&phase_checkpoint)?;
        println!("Phase complete! Saved: {}", phase_checkpoint);
    }

    println!("\n========================================");
    println!("Curriculum training complete!");
    println!("========================================");

    Ok(())
}
```

---

### Phase 3: Training (2-3h compute)

**Commandes:**

```bash
# Train CNN avec curriculum (recommandé first)
cargo run --release --bin train_curriculum -- \
    --architecture cnn

# Ou train Gold GNN (si implémenté)
cargo run --release --bin train_curriculum -- \
    --architecture gnn
```

**Outputs attendus:**
```
Phase 1 (50 epochs):
  Epoch 1: policy_loss=2.944, value_loss=0.823
  Epoch 10: policy_loss=1.234, value_loss=0.456
  Epoch 50: policy_loss=0.567, value_loss=0.123

Phase 2 (30 epochs, fine-tuning):
  Epoch 1: policy_loss=0.512, value_loss=0.098
  Epoch 30: policy_loss=0.234, value_loss=0.045

Phase 3 (20 epochs, final refinement):
  Epoch 1: policy_loss=0.198, value_loss=0.034
  Epoch 20: policy_loss=0.089, value_loss=0.012
```

---

### Phase 4: Benchmarking (2-3h)

#### Task 4.1: Benchmark vs Baseline (1h)

```bash
# Baseline CNN (current)
cargo run --release --bin compare_mcts -- \
    --games 100 \
    --simulations 150 \
    --nn-architecture cnn \
    --seed 2025 \
    --log-path benchmarks/baseline_cnn.csv

# CNN avec Curriculum Learning
cargo run --release --bin compare_mcts -- \
    --games 100 \
    --simulations 150 \
    --nn-architecture cnn \
    --seed 2025 \
    --log-path benchmarks/curriculum_cnn.csv \
    --checkpoint checkpoints/cnn_phase3_final.ot

# Gold GNN (si implémenté)
cargo run --release --bin compare_mcts -- \
    --games 100 \
    --simulations 150 \
    --nn-architecture gnn \
    --seed 2025 \
    --log-path benchmarks/gold_gnn.csv \
    --checkpoint checkpoints/gnn_phase3_final.ot
```

#### Task 4.2: Analyse Statistique (1h)

**Script:** `scripts/analyze_benchmarks.py`

```python
import pandas as pd
import scipy.stats as stats

baseline = pd.read_csv("benchmarks/baseline_cnn.csv")
curriculum = pd.read_csv("benchmarks/curriculum_cnn.csv")
gold_gnn = pd.read_csv("benchmarks/gold_gnn.csv")

print("=== Benchmark Results ===")
print(f"Baseline CNN: {baseline['score'].mean():.2f} ± {baseline['score'].std():.2f}")
print(f"Curriculum CNN: {curriculum['score'].mean():.2f} ± {curriculum['score'].std():.2f}")
print(f"Gold GNN: {gold_gnn['score'].mean():.2f} ± {gold_gnn['score'].std():.2f}")

# Statistical significance test
t_stat, p_value = stats.ttest_ind(baseline['score'], curriculum['score'])
print(f"\nCurriculum vs Baseline: t={t_stat:.3f}, p={p_value:.4f}")

if p_value < 0.05:
    print("✅ Statistically significant improvement!")
else:
    print("⚠️ Not statistically significant")
```

---

## 📊 Résultats Attendus

### Scénario Optimiste ✅

```
Baseline CNN:                 139.40 ± 8.2 pts
CNN + Curriculum Learning:    149.50 ± 7.1 pts  (+10.1 pts, +7.2%)
Gold GNN + Curriculum:        154.20 ± 6.8 pts  (+14.8 pts, +10.6%)

Beam Search (upper bound):    174.80 pts
```

**Verdict:** ✅ **Succès** - Atteint l'objectif 145-154 pts

### Scénario Réaliste ⚠️

```
Baseline CNN:                 139.40 ± 8.2 pts
CNN + Curriculum Learning:    145.80 ± 7.5 pts  (+6.4 pts, +4.6%)
Gold GNN + Curriculum:        148.30 ± 7.2 pts  (+8.9 pts, +6.4%)
```

**Verdict:** ⚠️ **Partiel** - Amélioration significative mais sous-optimal

### Scénario Pessimiste ❌

```
Baseline CNN:                 139.40 ± 8.2 pts
CNN + Curriculum Learning:    141.20 ± 8.0 pts  (+1.8 pts, +1.3%)
Gold GNN + Curriculum:        143.50 ± 7.8 pts  (+4.1 pts, +2.9%)
```

**Verdict:** ❌ **Échec** - Gain insuffisant pour justifier l'effort

---

## 🎯 Milestones & Validation

### Milestone 1: Expert Data Quality ✓
**Critères de validation:**
- Phase 1 average score: 150-155 pts ✓
- Phase 2 average score: 165-168 pts ✓
- Phase 3 average score: 174-175 pts ✓

**Si échec:** Beam search implementation bugguée → Debug avant de continuer

### Milestone 2: Training Convergence ✓
**Critères de validation:**
- Policy loss < 0.1 après Phase 3 ✓
- Value loss < 0.02 après Phase 3 ✓
- Training curves monotone decroissantes ✓

**Si échec:** Problème d'optimisation → Ajuster learning rates ou architecture

### Milestone 3: Benchmark Improvement ✓
**Critères de validation:**
- Gain ≥ +5 pts vs baseline (scénario réaliste) ✓
- p-value < 0.05 (significance statistique) ✓
- Pas de régression sur aucun cas de test ✓

**Si échec:** Retour à baseline, analyse post-mortem

---

## 🚀 Prochaines Étapes

**Si succès (≥145 pts):**
1. ✅ Documenter les résultats
2. ✅ Publier les checkpoints entraînés
3. ✅ Intégrer dans production
4. 🔬 Explorer Gold GNN (si pas déjà fait)
5. 🔬 Tester ensemble methods (CNN + GNN voting)

**Si échec (<145 pts):**
1. 📊 Analyse post-mortem détaillée
2. 🐛 Debug beam search quality
3. 🔬 Tester curriculum learning seul (sans nouvelles données)
4. 🤔 Reconsidérer approche (peut-être Pattern Rollouts V3?)

---

## 📝 Notes Importantes

### Leçons d'Expectimax

L'échec d'Expectimax MCTS (1.33 pts, -99% régression) nous enseigne:

1. **Simplicité > Complexité théorique**
   - Curriculum learning est conceptuellement simple
   - Mais basé sur données de qualité (beam search)
   - Pas de modélisation stochastique complexe

2. **Data Quality Matters**
   - Expert data (beam 1000) → 174 pts
   - Training sur ces données devrait transférer une partie de cette performance
   - Vs Expectimax qui modelait une incertitude non pertinente

3. **Validation Empirique Essentielle**
   - Ne pas supposer que ça marchera
   - Benchmarks rigoureux sur 100+ parties
   - Tests statistiques de significance

### Risques Identifiés

1. **Beam Search Too Expensive**
   - Phase 3 prend 12-16h
   - Solution: Générer en background, ou utiliser moins de games

2. **Overfitting Risk**
   - 6,650 examples might not be enough
   - Solution: Data augmentation (rotations, symétries)

3. **Transfer Learning Failure**
   - Curriculum might not transfer to MCTS context
   - Solution: Fine-tune avec self-play après supervised

---

*Créé: 2025-10-30*
*Prêt pour implémentation*
*Estimé: 24-36 heures total*
