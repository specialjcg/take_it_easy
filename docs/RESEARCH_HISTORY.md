# Take It Easy AI - Historique Complet des Recherches

**Projet**: IA pour le jeu de plateau "Take It Easy"
**Période**: Novembre 2025 - Février 2026
**Technologie**: Rust + PyTorch (tch-rs) + gRPC + Elm (MVU)

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

| Approche | Score Moyen | Status |
|----------|-------------|--------|
| **Graph Transformer (Direct)** | **149.38 pts** | **PRODUCTION** |
| GT + Lines + Row bonus | 149.99 pts | Gain négligeable |
| GT + V1Beam (v=1.0) | 151.73 pts | +2.3 pts, haute variance |
| Hybrid MCTS (Q-net + CNN) | 125.14 pts | Ancien production |
| Pattern Rollouts V2 | 139.40 pts | Alternative MCTS |
| Pure MCTS (100 sims) | 104.80 pts | Baseline |
| Random | ~50 pts | Minimum |

### Conclusion Clé

Le Graph Transformer (+24 pts vs Hybrid MCTS) a rendu obsolètes toutes les approches
MCTS précédentes. Les tentatives d'amélioration par stratégies humaines (complétion de
lignes, beam search) n'apportent pas de gain significatif : le modèle a déjà internalisé
la stratégie optimale.

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

### Phase 6: Graph Transformer (Février 2026)

**Percée architecturale** — abandon du MCTS au profit d'un réseau direct :

- **Architecture**: Graph Transformer (multi-head attention sur graphe hexagonal)
  - 4 couches, 4 têtes d'attention, dim=128
  - Entrée: features par noeud (tuile posée, valeurs, voisinage)
  - Sortie: policy (19 positions) — évaluation directe, sans rollouts
- **Entraînement**: Supervisé sur ~45k parties MCTS haute qualité
- **Score**: **149.38 pts** (+24 pts vs Hybrid MCTS)
- Résout le problème fondamental de géométrie hexagonale (attention = topologie native)
- Inférence ~100× plus rapide que MCTS (pas de simulations)

**Pistes d'amélioration testées (sans succès significatif)** :
1. **Plus de données** (45k → 100k parties) — gains marginaux, le modèle saturait déjà
2. **Architecture plus large** (dim=256, 6 couches) — overfitting, pas de gain
3. **Value head** (prédire le score final) — n'améliore pas la policy

### Phase 7: Benchmarks Stratégies Humaines (Février 2026)

Tentative de booster le GT Direct avec des heuristiques humaines :

| Stratégie | Score Moyen | Min | Max | Delta vs GT Direct |
|-----------|-------------|-----|-----|-------------------|
| GT Direct | 149.38 | 56 | 243 | baseline |
| GT + Lines | 149.68 | 50 | 259 | +0.30 (bruit) |
| GT + Lines + Row | 149.99 | 48 | 247 | +0.61 (bruit) |
| V1Beam (v=1.0) | 151.73 | 40 | 263 | +2.35 (haute var.) |
| V1Beam (v=3.0) | 149.77 | 60 | 249 | +0.39 (bruit) |

**Heuristiques testées** :
- **GT + Lines** : bonus pour complétion de lignes (3 directions hexagonales)
- **GT + Lines + Row** : bonus lignes + bonus rangée (valeurs identiques par rangée)
- **V1Beam** : beam search en profondeur avec value estimée par GT

**Conclusion** : Les stratégies humaines n'améliorent pas significativement le GT Direct.
Le modèle a déjà appris la stratégie optimale (complétion de lignes, gestion des conflits).
Les heuristiques explicites ajoutent du bruit plutôt que de l'information.

### Phase 8: Tests Frontend & Stabilisation (Février 2026)

**Migration frontend** : SolidJS → Elm (MVU) pour fiabilité.

**Infrastructure de tests** :
- elm-test avec extraction de logique pure (GameLogic.elm)
- Pattern CmdIntent pour tester les effets de bord
- 66 tests couvrant 8 dead states identifiés (DS1-DS8)

**Bugs de production corrigés** :
1. **DS8 — Freeze au démarrage** : `startTurn` initial sans safety poll → le client
   reste bloqué sur "La partie commence!" si la réponse gRPC se perd.
   Fix : `SchedulePollTurn 3000` après ReadySet/SessionPolled.
2. **State stale après "Retour"** : `BackToModeSelection` ne réinitialisait pas les
   tuiles du plateau → nouvelle partie affiche les tuiles de la précédente.
   Fix : reset complet de l'état gameplay.

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

### 4. Graph Transformer (Meilleur)

```
Architecture: 4 couches Transformer sur graphe hexagonal
Têtes d'attention: 4
Dimension: 128
Entrée: Features par noeud (tuile posée, valeurs, masque voisinage)
Sortie: Policy (19 positions)
Loss: Cross-Entropy supervisée
```

**Résultats**:
- Score direct (sans MCTS): **149.38 pts**
- Inférence: ~1ms par coup (vs ~100ms MCTS 100 sims)
- **Succès**: L'attention capture nativement la topologie hexagonale

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

### Tableau Final (Février 2026)

| Approche | Score | Delta vs Pure MCTS | Status |
|----------|-------|-------------------|--------|
| **Graph Transformer** | **149.38** | **+44.58** | **PRODUCTION** |
| GT + V1Beam (v=1.0) | 151.73 | +46.93 | Gain marginal |
| GT + Lines + Row | 149.99 | +45.19 | Bruit statistique |
| Pattern Rollouts V2 | 139.40 | +34.60 | Obsolète |
| Hybrid MCTS (Q-net) | 125.14 | +20.34 | Obsolète |
| Pure MCTS (150 sims) | ~105 | - | Baseline |
| Pure MCTS (100 sims) | 104.80 | - | Baseline |
| CNN MCTS (seul) | 101.37 | -3.43 | Dégradé |
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

Fév 2026: 149 pts (Graph Transformer — nouvelle architecture)
        → 150-152 pts (stratégies humaines — gain négligeable)
```

---

## Leçons Apprises

### Ce Qui Fonctionne

1. **Graph Transformer** > toutes les autres approches (+24 pts vs Hybrid MCTS)
2. **Attention sur graphe** respecte nativement la topologie hexagonale
3. **Entraînement supervisé sur données MCTS de qualité** — le modèle distille et dépasse
4. **Cross-Entropy** pour tâches de ranking
5. **Inférence directe** (sans MCTS) — plus rapide et plus forte

### Ce Qui Ne Fonctionne Pas

1. **CNN pour géométrie hexagonale** — Architecture inadaptée
2. **GNN** — Gains marginaux, instabilité
3. **Apprentissage circulaire** — Plafond de qualité
4. **Expectimax** — Modèle d'information erroné
5. **MSE pour ranking** — Détruit l'ordre relatif
6. **Heuristiques humaines sur GT** — Le modèle a déjà internalisé la stratégie
7. **Plus de données / architecture plus large / value head** — Rendements décroissants

### Insights Stratégiques

1. **Architecture > Heuristiques** : GT Direct (149 pts) > MCTS + patterns (139 pts)
2. **Le modèle sait mieux** : Les stratégies humaines (lignes, beam search) n'aident pas un GT bien entraîné
3. **Profilage avant optimisation** : CoW a causé régression
4. **Valider les baselines** : 159 pts était aspirationnel, pas réel
5. **Scaling laws limitées** : Plus de données et plus de paramètres ne garantissent pas de gain

---

## État Actuel et Recommandations

### Configuration Production

```bash
# Lancer le serveur avec Graph Transformer (défaut)
RUST_LOG=info ./target/release/take_it_easy \
  --mode multiplayer \
  --single-player

# Benchmark stratégies
cargo run --release --bin benchmark_strategies -- \
  --games 500 --strategies gt-direct,gt-lines,v1-beam
```

### Fichiers Clés

| Fichier | Rôle |
|---------|------|
| `src/neural/graph_transformer.rs` | Graph Transformer (production) |
| `src/neural/policy_value_net.rs` | Wrapper PolicyNet |
| `src/neural/manager.rs` | Gestion des architectures |
| `src/mcts/algorithm.rs` | MCTS (legacy) |
| `src/strategy/mod.rs` | Stratégies (GT Direct, GT+Lines, V1Beam) |
| `frontend-elm/src/GameLogic.elm` | Logique de jeu pure (testable) |
| `frontend-elm/tests/GameLogicTest.elm` | 66 tests dead states |
| `model_weights/graph_transformer_policy.safetensors` | Poids GT (VPS) |

### Améliorations Testées (rendements décroissants)

Les 3 pistes d'amélioration évidentes ont été tentées :

| Option | Résultat | Conclusion |
|--------|----------|------------|
| Plus de données (45k → 100k) | Gains marginaux | Le modèle sature |
| Architecture plus large (dim=256) | Overfitting | Pas assez de diversité |
| Value head (prédire score) | Pas de gain policy | Policy et value découplés |
| Heuristiques humaines (lignes, beam) | +0 à +2 pts | Le GT sait déjà |

### Ne Pas Poursuivre

- GNN (gains marginaux, instabilité)
- Expectimax MCTS (structurellement inadapté)
- CNN standard pour hexagonal (géométrie cassée)
- Boosting par heuristiques humaines (le GT a déjà internalisé la stratégie)
- Scaling naïf (plus de données / paramètres sans changement qualitatif)

---

## Conclusion

Après 4 mois de recherche (novembre 2025 — février 2026) :

1. **Le Graph Transformer** est l'approche définitive : **149 pts**, +24 pts vs Hybrid MCTS
2. **L'attention sur graphe** résout le problème fondamental de géométrie hexagonale
3. **Les heuristiques humaines sont inutiles** sur un GT bien entraîné — le modèle a déjà
   appris la stratégie optimale (complétion de lignes, gestion des conflits)
4. **Les pistes classiques d'amélioration** (plus de données, plus de paramètres, value head)
   sont en rendements décroissants

Pour progresser significativement au-delà de 150 pts, il faudrait :
- Un changement qualitatif dans les données d'entraînement (parties humaines expertes)
- Une approche d'apprentissage fondamentalement différente (RL avec exploration ciblée)
- Ou accepter que ~150 pts est proche du plafond pour cette taille de modèle

**Score production stable** : **149.38 pts** (Graph Transformer Direct)

---

*Document mis à jour le 18 février 2026*
*Auteurs: Claude Opus 4.5/4.6 + équipe de développement*
