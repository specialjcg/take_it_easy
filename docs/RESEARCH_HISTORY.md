# Take It Easy AI - Historique Complet des Recherches

**Projet**: IA pour le jeu de plateau "Take It Easy"
**Période**: Novembre 2025 - Janvier 2026
**Technologie**: Rust + PyTorch (tch-rs) + gRPC + SolidJS

---

## Table des Matières

1. [Résumé Exécutif](#résumé-exécutif)
2. [Chronologie du Projet](#chronologie-du-projet)
3. [Architectures Neuronales Testées](#architectures-neuronales-testées)
4. [Approches d'Entraînement](#approches-dentraînement)
5. [Variantes MCTS](#variantes-mcts)
6. [Bugs Majeurs Résolus](#bugs-majeurs-résolus)
7. [Résultats Comparatifs](#résultats-comparatifs)
8. [Leçons Apprises](#leçons-apprises)
9. [État Actuel et Recommandations](#état-actuel-et-recommandations)

---

## Résumé Exécutif

### Meilleur Résultat Actuel

| Approche | Score Moyen | Win Rate | Status |
|----------|-------------|----------|--------|
| **Hybrid MCTS (Q-net + CNN)** | **125.14 pts** | **74%** | **PRODUCTION** |
| Pattern Rollouts V2 | 139.40 pts | 72% | Alternative stable |
| Pure MCTS (100 sims) | 104.80 pts | - | Baseline |
| Random | ~50 pts | - | Minimum |

### Contribution des Composants

- **Q-Value Network**: +20.34 pts (pruning adaptatif early-game)
- **CNN Policy/Value**: -3.43 pts seul, mais utile en late-game
- **MCTS Rollouts**: Base solide (+55 pts vs random)

---

## Chronologie du Projet

### Phase 1: Infrastructure (Octobre-Novembre 2025)

- Implémentation du jeu Take It Easy en Rust
- Architecture hexagonale (Ports & Adapters)
- Serveur gRPC + frontend SolidJS
- MCTS de base avec UCB

### Phase 2: Optimisation Hyperparamètres (Novembre 2025)

**Session du 7 novembre 2025** - Grid Search Phase 1

- 19 configurations testées, 380 parties
- **Résultat**: 147 pts → **158 pts** (+7.5%)
- Poids optimaux: CNN 65%, Rollout 25%, Heuristic 5%, Contextual 5%

### Phase 3: Exploration Architectures (Décembre 2025)

**Tentatives CNN/GNN**:
- CNN AlphaZero-style: 147-152 pts (documenté, non reproductible)
- GNN Bronze: ~144 pts (instable)
- GNN Silver: Abandonné
- AlphaGo Zero self-play: Échec (convergence trop rapide)

**Optimisations**:
- Copy-on-Write (CoW): -10% performance (régression)
- Expectimax MCTS: 1.33 pts (catastrophe)
- Pattern Rollouts V2: **139.40 pts** (succès)

### Phase 4: Diagnostic et Corrections (Janvier 2026)

**Bugs critiques découverts**:
1. GroupNorm initialization → poids à zéro
2. Géométrie hexagonale cassée dans tenseur 5×5
3. CNN polluant les value estimates
4. Données d'entraînement circulaires

**Fix appliqués**:
- 47 canaux (17 base + 30 features lignes)
- CNN contourné (w_cnn=0)
- Rollouts pour value estimates

### Phase 5: Q-Value Network (Janvier 2026)

**Percée majeure le 24 janvier 2026**:
- Q-net pour pruning adaptatif
- Cross-entropy loss (pas MSE)
- Top-K=6, turns 0-9 seulement
- **+20.34 pts** vs Pure MCTS

---

## Architectures Neuronales Testées

### 1. CNN (Convolutional Neural Network)

```
Architecture: ResNet 3 blocs
Canaux: [128, 128, 96]
Entrée: 47 × 5 × 5 (après fix)
Sorties: Policy (19 positions), Value (score)
```

**Résultats**:
- Score initial: 12 pts (CNN seul, avant fix)
- Score après fix: 99-103 pts (CNN contourné)
- **Problème**: Géométrie hexagonale incompatible avec convolutions 2D

### 2. GNN (Graph Neural Network)

```
Architecture: 3 couches message-passing
Canaux: [64, 64, 64]
```

**Résultats**:
- GNN Bronze: ~144 pts (gains marginaux)
- GNN Supervisé: 60.97 pts (échec complet)
- **Problème**: Haute entropie (0.6-0.8), instabilité

### 3. Q-Value Network

```
Architecture: CNN 3 couches + FC
Entrée: 47 × 5 × 5
Sortie: 19 Q-values (une par position)
Loss: Cross-Entropy (pas MSE!)
```

**Résultats**:
- Pruning top-6: +20.34 pts
- **Succès**: Ranking relatif préservé

---

## Approches d'Entraînement

### 1. AlphaGo Zero Self-Play

| Tentative | Itérations | Score | Résultat |
|-----------|------------|-------|----------|
| Déc 2025 | 20-50 | 79→83 pts | Convergence trop rapide |
| Jan 2026 | 50 | 52→54 pts | Bug sauvegarde poids |

**Problème**: Apprentissage circulaire - le réseau génère ses propres données.

### 2. Entraînement Supervisé

- Données: 500 parties MCTS 1000 sims
- Filtrage: ≥110 pts (82 parties)
- **Résultat**: +0.40 pts (quasi nul)
- **Cause**: Données "expert" = données auto-générées

### 3. Q-Value Supervised

- Données: 20k positions avec Q-values MCTS
- Loss: Softmax + Cross-Entropy
- Temperature: 0.1 (ranking sharp)
- **Résultat**: Ranking accuracy restaurée

---

## Variantes MCTS

### 1. MCTS Standard (UCB)

- 100-300 simulations par coup
- C_PUCT = 1.0
- **Score**: 84-105 pts selon config

### 2. Pattern Rollouts V2

- Évaluation par patterns de positions
- Scoring de complétion de lignes
- Détection de conflits
- **Score**: 139.40 pts (+35 pts vs base)

### 3. Expectimax MCTS

- Nœuds chance pour 27 tuiles possibles
- **Score**: 1.33 pts
- **Échec**: Modèle d'information erroné pour Take It Easy

### 4. Gumbel MCTS

- Exploration Gumbel-Top-k
- **Score**: -83 pts vs baseline
- **Échec**: Non adapté au problème

### 5. Hybrid MCTS (Q-net)

- Q-net pruning early-game (turns 0-9)
- CNN MCTS late-game
- Top-K = 6 positions
- **Score**: 125.14 pts (+20 pts vs pure)

---

## Bugs Majeurs Résolus

### 1. GroupNorm Initialization (Jan 2026)

**Symptôme**: Policy loss = ln(19) = 2.944 (distribution uniforme)

**Cause**:
```rust
// Problème
if size.len() == 1 { param.f_zero_() }  // Zeroes GroupNorm.weight!

// Solution
if name.ends_with(".bias") { param.f_zero_() }  // Seulement les biais
```

**Impact**: 8h45 d'entraînement perdues, 38 itérations = 0 apprentissage

### 2. Géométrie Hexagonale Cassée

**Symptôme**: CNN aveugle à 67% du scoring

**Cause**: Lignes diagonales en zigzag dans tenseur 5×5
```
Dir1 (vertical):  5/5 lignes droites  ✓
Dir2 (diagonale): 5/5 lignes en zigzag ✗
Dir3 (diagonale): 5/5 lignes en zigzag ✗
```

**Solution**: 30 canaux additionnels de features lignes explicites

### 3. MSE Loss pour Q-Values

**Symptôme**: Q-net prédictions quasi-constantes (range 0.009)

**Cause**: MSE optimise erreur absolue, pas ranking relatif

**Solution**: Softmax targets + Cross-Entropy loss

---

## Résultats Comparatifs

### Tableau Final (Janvier 2026)

| Approche | Score | Delta vs Pure | Status |
|----------|-------|---------------|--------|
| **Hybrid MCTS (Q-net)** | **125.14** | **+20.34** | **PRODUCTION** |
| Pattern Rollouts V2 | 139.40 | +34.60 | Alternative |
| GNN Bronze | 144 | +39.20 | Instable |
| Pure MCTS (150 sims) | ~105 | - | Baseline |
| CNN MCTS (seul) | 101.37 | -3.43 | Dégradé |
| Pure MCTS (100 sims) | 104.80 | - | Baseline |
| GNN Supervisé | 60.97 | -43.83 | Échec |
| Random | ~50 | -54.80 | Minimum |
| CNN (avant fix) | 12 | -92.80 | Catastrophe |
| Expectimax | 1.33 | -103.47 | Catastrophe |

### Évolution des Scores

```
Nov 2025: 147 pts (baseline documenté)
        → 158 pts (hyperparamètres optimisés)

Déc 2025: 139 pts (Pattern Rollouts V2)
        → 12 pts (régression CNN cassé)

Jan 2026: 99 pts (CNN contourné)
        → 125 pts (Hybrid Q-net)
```

---

## Leçons Apprises

### Ce Qui Fonctionne

1. **MCTS + Heuristiques** > Réseaux seuls
2. **Pruning adaptatif** via Q-net efficace
3. **Cross-Entropy** pour tâches de ranking
4. **Séparation des rôles**: Q-net pruning, CNN late-game
5. **Early game focus**: Pruning utile turns 0-9 seulement

### Ce Qui Ne Fonctionne Pas

1. **CNN pour géométrie hexagonale** - Architecture inadaptée
2. **GNN** - Gains marginaux, instabilité
3. **Apprentissage circulaire** - Plafond de qualité
4. **Expectimax** - Modèle d'information erroné
5. **MSE pour ranking** - Détruit l'ordre relatif
6. **Copy-on-Write** - Overhead > économies pour petits structs

### Insights Stratégiques

1. **Simplicité > Élégance**: Pattern Rollouts (139 pts) > GNN complexe (60 pts)
2. **Profilage avant optimisation**: CoW a causé régression
3. **Valider les baselines**: 159 pts était aspirationnel, pas réel
4. **Tester les composants isolément**: CNN seul = -3 pts

---

## État Actuel et Recommandations

### Configuration Production

```bash
# Lancer le meilleur AI
RUST_LOG=info ./target/release/take_it_easy \
  --mode multiplayer \
  --single-player \
  --hybrid-mcts \
  --top-k 6

# Benchmark
cargo run --release --bin compare_mcts_hybrid -- \
  --games 100 --simulations 100 --top-k 6
```

### Fichiers Clés

| Fichier | Rôle |
|---------|------|
| `src/mcts/algorithm.rs` | MCTS + Hybrid |
| `src/neural/qvalue_net.rs` | Q-Value Network |
| `src/neural/tensor_conversion.rs` | Encodage 47 canaux |
| `model_weights/qvalue_net.params` | Poids Q-net |

### Améliorations Futures (si ressources)

| Priorité | Option | Gain Attendu | Effort |
|----------|--------|--------------|--------|
| 1 | Plus de données Q-net (50k+) | +5-10 pts | Faible |
| 2 | Architecture Attention/Transformer | +20-40 pts | Élevé |
| 3 | MCTS parallèle | 6-8× speedup | Moyen |
| 4 | Données humaines expertes | +30-50 pts | Très élevé |

### Ne Pas Poursuivre

- GNN (gains marginaux, instabilité)
- Expectimax MCTS (structurellement inadapté)
- CNN standard pour hexagonal (géométrie cassée)
- Optimisations mémoire prématurées

---

## Conclusion

Après 3 mois de recherche intensive:

1. **Le Q-net Hybrid MCTS** est l'approche la plus performante (+20 pts)
2. **Le CNN/GNN n'apporte pas de valeur** significative seul
3. **Le problème de géométrie hexagonale** est fondamental
4. **L'apprentissage circulaire** limite le self-play pur

Pour progresser au-delà de 125-140 pts:
- Architecture respectant la topologie hexagonale (Transformer?)
- Données externes de qualité (parties humaines)
- Algorithme d'apprentissage différent

**Score actuel stable**: 125 pts (Hybrid) / 139 pts (Pattern Rollouts)

---

*Document consolidé le 25 janvier 2026*
*Auteur: Claude Opus 4.5 + équipe de développement*
