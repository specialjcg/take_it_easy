# Comment le Beam Search Peut Améliorer l'Apprentissage

## 🎯 Problème Actuel

L'IA s'entraîne par **self-play** : elle joue contre elle-même avec MCTS et apprend de ses propres parties.

**Limitation** : L'IA n'atteint que 79.5% de l'optimal → elle apprend de données sous-optimales, ce qui crée un **plafond de performance**.

## 💡 Solutions avec Beam Search

### 1. 🏅 Expert Data Generation (Solution Recommandée)

**Concept** : Générer des données d'entraînement de haute qualité en utilisant le beam search comme "expert".

**Pipeline** :
```
1. Jouer N parties avec MCTS (exploration rapide)
2. Pour chaque partie, rejouer avec beam search (beam width 1000)
3. Extraire les meilleurs coups du beam search
4. Entraîner le PolicyNet sur ces coups optimaux
```

**Avantages** :
- ✅ Données de meilleure qualité (jusqu'à 175 pts au lieu de 139)
- ✅ Apprentissage supervisé au lieu de reinforcement learning pur
- ✅ Convergence plus rapide
- ✅ Plafond de performance plus élevé

**Inconvénients** :
- ❌ Coût computationnel élevé (beam search lent)
- ❌ Nécessite beaucoup de stockage (sauvegarder les trajectoires)

### 2. 🎓 Curriculum Learning

**Concept** : Commencer avec des objectifs simples et augmenter progressivement la difficulté.

**Phases** :
```
Phase 1 (100 parties) : Beam width 100 (rapide, ~160 pts)
  → PolicyNet apprend les bases

Phase 2 (100 parties) : Beam width 500 (moyen, ~170 pts)
  → PolicyNet apprend à optimiser

Phase 3 (200 parties) : Beam width 1000 (lent, ~175 pts)
  → PolicyNet apprend l'excellence
```

**Avantages** :
- ✅ Progression graduelle (évite l'overfitting)
- ✅ Meilleur équilibre vitesse/qualité
- ✅ L'IA apprend à généraliser

**Inconvénients** :
- ❌ Long à entraîner (3 phases)
- ❌ Complexe à implémenter

### 3. 🔄 Hybrid Training: MCTS + Beam Replay

**Concept** : Mélanger données MCTS (exploration) et beam search (exploitation).

**Mix de données** :
- 70% MCTS self-play (exploration, variété)
- 30% Beam search optimal (exploitation, qualité)

**Workflow** :
```
1. Générer 100 parties MCTS (rapide)
2. Sélectionner les 30 meilleures parties
3. Rejouer ces 30 avec beam search
4. Entraîner sur: 70 MCTS + 30 Beam
5. Répéter
```

**Avantages** :
- ✅ Équilibre exploration/exploitation
- ✅ Coût modéré (beam sur 30% seulement)
- ✅ Évite le sur-apprentissage

**Inconvénients** :
- ❌ Complexité d'implémentation
- ❌ Hyperparamètres à tuner (ratio MCTS/Beam)

### 4. 📊 Value Net Training avec Scores Beam

**Concept** : Utiliser le beam search pour obtenir des **labels de score précis**.

**Problème actuel** :
- ValueNet apprend le score final de la partie
- Mais ces scores ne reflètent pas le **vrai potentiel** (optimal)

**Solution** :
```
Pour chaque position de jeu:
  1. Calculer le score final réel: s_real
  2. Calculer le score optimal (beam): s_optimal
  3. Entraîner ValueNet à prédire: s_optimal

→ ValueNet apprend le "vrai" potentiel, pas juste le score MCTS
```

**Avantages** :
- ✅ ValueNet plus précis
- ✅ Meilleure évaluation des positions
- ✅ MCTS plus efficace (moins de simulations nécessaires)

**Inconvénients** :
- ❌ Beam search très coûteux (pour chaque position !)
- ❌ Faisable seulement en offline

## 📈 Gain Estimé par Approche

| Approche | Complexité | Coût Calcul | Gain Estimé | Temps Entraînement |
|----------|------------|-------------|-------------|-------------------|
| **Expert Data** | Moyenne | Élevé | **+8-12 pts** | 48h (500 parties) |
| **Curriculum** | Élevée | Très élevé | **+10-15 pts** | 72h (3 phases) |
| **Hybrid 70/30** | Élevée | Moyen | **+5-8 pts** | 36h (400 parties) |
| **ValueNet Optimal** | Faible | Très élevé | **+3-5 pts** | 24h (offline) |

## 🚀 Plan d'Implémentation Recommandé

### Phase 1 : Expert Data Generation (Prioritaire)

**Objectif** : Atteindre 145-150 pts

**Steps** :
1. Générer 200 parties avec MCTS (self-play actuel)
2. Sélectionner les 100 meilleures parties (score > 140)
3. Rejouer ces 100 parties avec beam search (width 1000)
4. Extraire les placements optimaux comme labels
5. Entraîner Gold GNN (256-256-128-64) sur ces données

**Estimation** :
- Génération MCTS : 6h (200 parties × 150 sims)
- Beam search replay : 12h (100 parties × beam 1000)
- Entraînement : 8h (Gold GNN)
- **Total : ~26h**

### Phase 2 : Gold GNN avec Plus de Données (Parallèle)

**Objectif** : Améliorer la capacité du réseau

**Configuration Gold GNN** :
```rust
Architecture: [256, 256, 128, 64]  // vs Silver [128, 128, 64]
Dropout: 0.2
Learning rate: 0.0005 (plus faible pour stabilité)
Batch size: 64
Parties: 500 (vs 200 actuellement)
```

**Gain estimé** : +3-5 pts (grâce à la capacité accrue)

### Phase 3 : Combine Les Deux (Optimal)

**Expert Data + Gold GNN** → Gain total estimé : **+10-15 pts**

**Score final attendu** : 139.40 + 10-15 = **149-154 pts** ✅

## 🛠️ Implémentation Technique

### 1. Module `beam_data_generator.rs`

```rust
pub struct BeamDataGenerator {
    beam_width: usize,
}

impl BeamDataGenerator {
    /// Génère des données optimales à partir d'une partie MCTS
    pub fn generate_expert_data(
        &self,
        game_history: &[GameState]
    ) -> Vec<(Position, Tile, f32)> {
        // Pour chaque état de jeu:
        // 1. Lancer beam search pour trouver le meilleur coup
        // 2. Retourner (position, tuile, score_optimal)
    }
}
```

### 2. Module `curriculum_trainer.rs`

```rust
pub struct CurriculumConfig {
    phase1_games: usize,
    phase1_beam: usize,
    phase2_games: usize,
    phase2_beam: usize,
    // ...
}

pub fn train_with_curriculum(config: CurriculumConfig) {
    // Phase 1 : Beam faible
    // Phase 2 : Beam moyen
    // Phase 3 : Beam fort
}
```

### 3. Integration dans `trainer.rs`

```rust
pub struct TrainingConfig {
    // ... existing fields
    use_beam_guidance: bool,
    beam_width: usize,
    beam_data_ratio: f64,  // 0.3 pour 30% beam, 70% MCTS
}
```

## 📊 Métriques de Suivi

Pour mesurer l'amélioration :

1. **Gap d'optimalité** : Comparer score IA vs beam search
2. **Taux d'apprentissage** : Mesurer la vitesse de convergence
3. **Variance** : Vérifier la stabilité des performances
4. **Overfitting** : Tester sur données jamais vues

## ⚠️ Risques et Mitigation

| Risque | Impact | Probabilité | Mitigation |
|--------|--------|-------------|------------|
| Overfitting sur beam data | Élevé | Moyenne | Mix 70% MCTS + 30% Beam |
| Coût calcul prohibitif | Moyen | Élevée | Beam seulement sur best games |
| Pas d'amélioration | Élevé | Faible | Benchmark à chaque phase |
| Régression vs baseline | Critique | Faible | Toujours garder baseline CNN |

## 🎯 Recommandation Finale

### Option Conservatrice : Accepter 139.40 pts
- ✅ Objectifs dépassés
- ✅ Pas de risque
- ✅ Code production-ready

### Option Ambitieuse : Expert Data + Gold GNN
- ⭐ Gain estimé : +10-15 pts → **149-154 pts**
- ⏱️ Temps : ~26h d'entraînement
- 💰 Coût : Élevé mais réalisable
- 📈 ROI : Excellent si objectif 145+ pts important

### Approche Hybride : Gold GNN seul d'abord
- 🎯 Gain estimé : +3-5 pts → **142-144 pts**
- ⏱️ Temps : ~12h d'entraînement
- 💰 Coût : Modéré
- 📈 ROI : Bon compromis

**Ma recommandation** : **Approche Hybride** (Gold GNN seul)
- Lancer entraînement Gold GNN (256-256-128-64) sur 500 parties
- Si résultats prometteurs (143+ pts), investir dans Expert Data
- Si échec, accepter 139.40 pts comme optimal

---

*Document rédigé le 2025-10-26*
*Basé sur l'analyse du gap d'optimalité (20.5% gap, beam search 174.8 pts)*
