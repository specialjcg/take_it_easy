# Curriculum Learning - Plan d'Implémentation Complet

## Contexte

Après l'échec de Gold GNN (127.74 pts, -11.66 pts vs baseline 139.40), nous passons au **Curriculum Learning avec Beam Search** pour améliorer l'IA de façon significative.

**Objectif:** +10-15 pts → 149-154 pts (20-30 jours d'implémentation)

---

## Phase 1: Génération de Données Expertes ✅ EN COURS

### Outils Créés

1. **`src/bin/expert_data_generator.rs`** ✅ COMPLÉTÉ
   - Génère des parties avec beam search (configurable: 100, 500, 1000)
   - Sauvegarde au format JSON: plateau_state, tile_played, position_played, turn, score_after
   - Chaque partie génère 19 exemples d'entraînement (un par coup)

2. **`curriculum_learning.sh`** ✅ COMPLÉTÉ
   - Script orchestrant les 3 phases de génération de données
   - Détection automatique des fichiers déjà générés (évite regénération)

### Génération de Données

**Phase 1: Données Faciles (Beam 100)** 🔄 EN COURS
```bash
cargo run --release --bin expert_data_generator -- \
  -g 50 -b 100 \
  -o expert_data/phase1_beam100.json \
  -s 2025
```
- 50 parties × Beam 100
- Score attendu: ~150 pts (vs 139 baseline, vs 175 optimal)
- Durée: ~30 minutes
- Exemples générés: 950 (50 × 19)

**Phase 2: Données Moyennes (Beam 500)** ⏳ À FAIRE
```bash
cargo run --release --bin expert_data_generator -- \
  -g 100 -b 500 \
  -o expert_data/phase2_beam500.json \
  -s 2026
```
- 100 parties × Beam 500
- Score attendu: ~165 pts
- Durée: ~3-4 heures
- Exemples générés: 1900 (100 × 19)

**Phase 3: Données Difficiles (Beam 1000)** ⏳ À FAIRE
```bash
cargo run --release --bin expert_data_generator -- \
  -g 200 -b 1000 \
  -o expert_data/phase3_beam1000.json \
  -s 2027
```
- 200 parties × Beam 1000
- Score attendu: ~175 pts (quasi-optimal)
- Durée: ~12-16 heures
- Exemples générés: 3800 (200 × 19)

**Total: 6650 exemples d'entraînement d'expert**

---

## Phase 2: Implémentation de l'Entraînement Supervisé ⏳ À FAIRE

### Modification Requise: Nouveau Mode d'Entraînement

Créer `src/neural/training/supervised_trainer.rs`:

```rust
pub struct SupervisedTrainer {
    policy_net: PolicyNet,
    value_net: ValueNet,
    optimizer_policy: Optimizer,
    optimizer_value: Optimizer,
}

impl SupervisedTrainer {
    /// Charge les données d'entraînement depuis un fichier JSON
    pub fn load_expert_data(path: &str) -> Vec<TrainingExample> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)?
    }

    /// Entraîne PolicyNet à prédire les coups experts
    pub fn train_policy_epoch(&mut self, examples: &[TrainingExample]) -> f32 {
        let mut total_loss = 0.0;

        for batch in examples.chunks(32) {
            // 1. Convert plateau_state to tensor
            let states = self.batch_states_to_tensor(batch);

            // 2. Forward pass
            let predictions = self.policy_net.forward(&states, true);

            // 3. Target: one-hot sur position_played
            let targets = self.batch_positions_to_one_hot(batch);

            // 4. Loss: CrossEntropy
            let loss = predictions.cross_entropy_for_logits(&targets);

            // 5. Backward + update
            self.optimizer_policy.zero_grad();
            loss.backward();
            self.optimizer_policy.step();

            total_loss += loss.double_value(&[]);
        }

        total_loss / (examples.len() as f32 / 32.0)
    }

    /// Entraîne ValueNet à prédire le score final normalisé
    pub fn train_value_epoch(&mut self, examples: &[TrainingExample]) -> f32 {
        let mut total_loss = 0.0;

        for batch in examples.chunks(32) {
            let states = self.batch_states_to_tensor(batch);
            let predictions = self.value_net.forward(&states, true);

            // Target: score_after normalisé [-1, 1]
            // 0 pts → -1, 175 pts (optimal) → +1
            let targets = self.batch_scores_to_normalized(batch);

            // Loss: MSE
            let loss = (predictions - targets).pow(2).mean(Kind::Float);

            self.optimizer_value.zero_grad();
            loss.backward();
            self.optimizer_value.step();

            total_loss += loss.double_value(&[]);
        }

        total_loss / (examples.len() as f32 / 32.0)
    }
}
```

### Modification du main.rs

Ajouter option CLI:
```rust
#[arg(long)]
expert_data_path: Option<String>,
```

Dans la boucle d'entraînement:
```rust
if let Some(expert_path) = args.expert_data_path {
    // MODE SUPERVISÉ
    let examples = SupervisedTrainer::load_expert_data(&expert_path)?;

    for epoch in 0..50 {
        let policy_loss = trainer.train_policy_epoch(&examples);
        let value_loss = trainer.train_value_epoch(&examples);

        println!("Epoch {}/50 - Policy Loss: {:.4}, Value Loss: {:.4}",
                 epoch+1, policy_loss, value_loss);

        if epoch % 10 == 0 {
            // Sauvegarder checkpoint
            manager.save_weights()?;

            // Évaluation intermédiaire
            let score = evaluate_model()?;
            println!("  Evaluation Score: {:.2} pts", score);
        }
    }
} else {
    // MODE SELF-PLAY (code actuel)
    // ...
}
```

---

## Phase 3: Curriculum Learning - Entraînement Progressif ⏳ À FAIRE

### Phase 1: Entraînement sur Données Faciles

```bash
cargo run --release --bin take_it_easy -- \
  --mode training \
  --expert-data-path expert_data/phase1_beam100.json \
  --nn-architecture cnn \
  --epochs 50
```

**Attendu après Phase 1:**
- Score: 142-145 pts (+3-6 vs baseline)
- PolicyNet apprend les coups "bons" (pas optimaux)
- ValueNet prédit scores ~150 pts

### Phase 2: Fine-tuning sur Données Moyennes

```bash
cargo run --release --bin take_it_easy -- \
  --mode training \
  --expert-data-path expert_data/phase2_beam500.json \
  --nn-architecture cnn \
  --epochs 30 \
  --load-weights model_weights  # Reprend Phase 1
```

**Attendu après Phase 2:**
- Score: 145-150 pts (+6-11 vs baseline)
- PolicyNet apprend les coups "très bons"
- ValueNet prédit scores ~165 pts

### Phase 3: Fine-tuning sur Données Difficiles

```bash
cargo run --release --bin take_it_easy -- \
  --mode training \
  --expert-data-path expert_data/phase3_beam1000.json \
  --nn-architecture cnn \
  --epochs 20 \
  --load-weights model_weights  # Reprend Phase 2
```

**Attendu après Phase 3:**
- Score: **149-154 pts** (+10-15 vs baseline) 🎯 OBJECTIF
- PolicyNet apprend les coups quasi-optimaux
- ValueNet prédit scores ~175 pts

---

## Phase 4: Évaluation Finale ⏳ À FAIRE

```bash
# Benchmark sur 100 parties pour statistiques robustes
cargo run --release --bin compare_mcts -- \
  -g 100 \
  -s 150 \
  --nn-architecture cnn

# Analyse de l'écart avec l'optimal
cargo run --release --bin optimal_solver
```

**Métriques de Succès:**
- Score moyen ≥ 149 pts (objectif minimum: +10 vs baseline)
- Écart vs optimal < 15% (vs 20.5% actuellement)
- Victoires vs baseline ≥ 70%

---

## Timeline Estimée

| Phase | Tâche | Durée | Statut |
|-------|-------|-------|--------|
| 1.1 | Créer expert_data_generator.rs | 2h | ✅ COMPLÉTÉ |
| 1.2 | Créer curriculum_learning.sh | 30min | ✅ COMPLÉTÉ |
| 1.3 | Générer Phase 1 data (Beam 100) | 30min | 🔄 EN COURS |
| 1.4 | Générer Phase 2 data (Beam 500) | 4h | ⏳ À FAIRE |
| 1.5 | Générer Phase 3 data (Beam 1000) | 16h | ⏳ À FAIRE |
| 2.1 | Implémenter SupervisedTrainer | 4h | ⏳ À FAIRE |
| 2.2 | Modifier main.rs pour CLI | 1h | ⏳ À FAIRE |
| 2.3 | Tests unitaires | 2h | ⏳ À FAIRE |
| 3.1 | Entraînement Phase 1 | 1h | ⏳ À FAIRE |
| 3.2 | Entraînement Phase 2 | 1h | ⏳ À FAIRE |
| 3.3 | Entraînement Phase 3 | 1h | ⏳ À FAIRE |
| 4.1 | Benchmark final | 2h | ⏳ À FAIRE |
| 4.2 | Documentation résultats | 1h | ⏳ À FAIRE |

**Total: ~36 heures (~5 jours ouvrés)**

---

## Risques et Mitigation

### Risque 1: Overfitting sur Données Expertes
**Symptôme:** Score d'entraînement élevé mais score de test faible
**Mitigation:**
- Split train/val 80/20
- Early stopping basé sur validation score
- Dropout 0.3 pendant entraînement

### Risque 2: Beam Search Trop Lent
**Symptôme:** Génération Phase 3 prend > 24h
**Mitigation:**
- Paralléliser génération (multi-threading)
- Réduire nombre de parties (200 → 150)
- Accepter Beam 800 au lieu de 1000

### Risque 3: Amélioration Insuffisante
**Symptôme:** Score final < 145 pts (+6 seulement)
**Mitigation:**
- Ajouter Phase 4 avec mix MCTS + Beam (hybride)
- Augmenter capacité CNN (plus de filtres)
- Entraîner plus longtemps (100 epochs au lieu de 50)

---

## Prochaines Étapes Immédiates

1. ✅ Attendre fin de génération Phase 1 (~30min)
2. ⏳ Vérifier qualité données (inspecter phase1_beam100.json)
3. ⏳ Implémenter SupervisedTrainer (4h)
4. ⏳ Tester entraînement Phase 1 (1h)
5. ⏳ Si succès: lancer génération Phase 2+3 en parallèle (overnight)

---

## Comparaison avec Autres Approches

| Approche | Gain Estimé | Durée | Complexité |
|----------|-------------|-------|------------|
| Gold GNN | ❌ -11.66 pts | 12h | Moyenne |
| Pattern Rollouts V3 | ❌ -51.28 pts | 2h | Faible |
| **Curriculum Learning** | **+10-15 pts** 🎯 | **5j** | **Élevée** |
| Expert Data Simple | +8-12 pts | 3j | Moyenne |
| Hybrid Training | +5-8 pts | 2j | Faible |

**Curriculum Learning est l'approche la plus prometteuse mais aussi la plus complexe.**

---

## Références

- `docs/beam_search_learning_improvement.md` - 4 approches avec beam search
- `docs/optimality_gap_analysis.md` - IA à 79.5% de l'optimal
- `src/bin/expert_data_generator.rs` - Générateur de données
- `curriculum_learning.sh` - Script d'orchestration
