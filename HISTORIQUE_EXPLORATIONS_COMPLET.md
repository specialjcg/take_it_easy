# Historique Complet des Explorations - Take It Easy AI

**Date de consolidation**: 18 janvier 2026
**Période couverte**: Décembre 2025 - Janvier 2026

---

## Table des Matières

1. [Résumé Exécutif](#résumé-exécutif)
2. [Architectures Neuronales](#architectures-neuronales)
3. [Approches d'Entraînement](#approches-dentraînement)
4. [Améliorations MCTS](#améliorations-mcts)
5. [Bugs et Investigations](#bugs-et-investigations)
6. [Tableau Comparatif Final](#tableau-comparatif-final)
7. [Leçons Apprises](#leçons-apprises)
8. [Recommandations](#recommandations)

---

## Résumé Exécutif

### Meilleur Résultat Reproductible
| Approche | Score | Status |
|----------|-------|--------|
| **Pattern Rollouts V2** | **139.40 pts** | ✅ BASELINE OPTIMAL |
| Pure MCTS (100 sims) | 103.3 pts | ✅ Référence fiable |
| Random | ~50 pts | Baseline minimum |

### État Actuel (Janvier 2026)
- **CNN**: Contourné (w_cnn=0.00), détruisait MCTS (12 pts vs 100 pts)
- **GNN**: Exploré, abandonné (60 pts, haute entropie)
- **MCTS**: Fonctionne bien avec rollouts heuristiques
- **Problème géométrique identifié**: 10/15 lignes de scoring invisibles au CNN

---

## Architectures Neuronales

### 1. CNN (Convolutional Neural Network) - Style AlphaZero

**Configuration:**
- ResNet avec 3 blocs convolutionnels
- Canaux: [128, 128, 96]
- Entrée: Grille 5×5 (mapping hexagonal)
- Têtes séparées policy (19) et value

**Résultats Historiques:**
- Score documenté: 147-152 pts (NON REPRODUCTIBLE)
- Score actuel: **12 pts** (avant fix), **99.4 pts** (après fix, CNN contourné)

**Problème Identifié (Janvier 2026):**
```
SCORING LINES IN TENSOR GRID:
Dir1 (vertical):  5/5 lignes droites  ✓
Dir2 (diagonale): 5/5 lignes en zigzag ✗
Dir3 (diagonale): 5/5 lignes en zigzag ✗

TOTAL: 5 droites, 10 cassées → CNN aveugle à 67% du jeu
```

**Fix Appliqué:**
- Ajout 30 canaux de features de lignes explicites (47 canaux total)
- Désactivation du CNN (w_cnn=0) jusqu'à architecture alternative

**Status:** ❌ ÉCHEC - Architecture inadaptée à la géométrie hexagonale

---

### 2. GNN Bronze (Graph Neural Network - Spatial 2D)

**Configuration:**
- Entrée: [1, 5, 5, 5] (batch, channels, height, width)
- Architecture: [64, 64, 64] canaux
- Mapping hexagonal vers grille 2D avec padding

**Résultats:**
- Score: ~144 pts (50 parties)
- Amélioration vs baseline: +1.8 pts

**Observations:**
- Meilleures positions: edges (7, 13, 2)
- Structure spatiale 2D > aplatissement 1D

**Status:** ⚠️ MODESTE - Gains marginaux, instabilité

---

### 3. GNN Silver (Capacité Augmentée)

**Configuration:**
- Canaux: [128, 128, 64] (3× Bronze)
- Connexions résiduelles (couche 2)
- BatchNorm par couche
- Dropout adaptatif (0.3)

**Résultats:**
- Score attendu: 135-142 pts
- Score réel: Non complété

**Status:** ⚠️ EN COURS - Abandonné avant validation

---

### 4. GNN Supervisé (Janvier 2026)

**Configuration:**
- 50 époques d'entraînement supervisé
- Poids adaptatifs hybrides
- Monitoring entropie

**Résultats:**
- Score: **60.97 ± 29.24 pts** (30 parties)
- Delta vs cible: **-57%**
- Entropie: 0.6-0.8 (reste haute = aucun apprentissage)

**Cause Racine:**
- GNN fondamentalement instable pour ce problème
- Pas de confiance dans les prédictions
- Dégradation pendant entraînement AlphaGo Zero

**Status:** ❌ ÉCHEC COMPLET

---

## Approches d'Entraînement

### 1. AlphaGo Zero - Self-Play (Décembre 2025)

**Configuration Tentative 1:**
- 20-50 itérations
- 20-50 parties par itération
- 150 simulations MCTS par coup
- 10 époques d'entraînement par itération
- LR: 0.01-0.03

**Résultats:**
| Itération | Score | Value Loss |
|-----------|-------|------------|
| 1 | 79.11 pts | 0.1370 |
| 2 | 82.86 pts | 0.0702 (-49%) |
| 3 | 80.97 pts | Convergence déclenchée |

**Amélioration:** +3.75 pts puis régression

**Status:** ❌ ÉCHEC - Convergence trop rapide

---

### 2. AlphaGo Zero - 50 Itérations (Janvier 2026)

**Configuration (après fix GroupNorm):**
- 50 itérations complètes
- 20 parties par itération
- 200 simulations MCTS

**Résultats:**
| Itération | Policy Loss | Value Loss | Score |
|-----------|-------------|------------|-------|
| 1 | 1.70 | 2.31 | 52.36 pts |
| 16 | 1.08 | 0.13 | — |
| 50 | 1.05 | 0.01 | 54.38 pts |

**Amélioration:** Aucune (+2 pts sur 50 itérations)

**Bug Découvert:** Poids jamais sauvegardés (alphago_zero_trainer.rs:216)

**Status:** ❌ ÉCHEC COMPLET - 8h45 perdues

---

### 3. Entraînement Supervisé sur Données Expert

**Génération des Données:**
- 500 parties MCTS 1000 simulations
- Filtrage: ≥110 pts
- 82 parties sélectionnées (16.4%)
- 1,558 exemples d'entraînement
- Score moyen sélectionné: 126.05 pts

**Résultats:**
| Métrique | Avant | Après | Delta |
|----------|-------|-------|-------|
| Score | 80.86 pts | 81.26 pts | +0.40 pts (+0.5%) |
| Policy Loss | 2.9445 | 2.9445 | 0% (CONSTANT) |
| Value Loss | 0.0628 | 0.0598 | -4.8% |

**Cause Racine - Apprentissage Circulaire:**
- Le réseau génère les données pour lui-même
- Données "expert" = données auto-générées
- Parties de qualité = tuiles chanceuses, pas meilleure stratégie
- Plafond d'optimisation atteint

**Status:** ❌ ÉCHEC - Pas de signal d'apprentissage

---

### 4. Pattern Rollouts V2 (Référence)

**Configuration:**
- CNN + MCTS avec rollouts heuristiques
- Évaluation par patterns de positions
- Scoring de complétion de lignes
- Détection de conflits

**Résultats:**
- Score: **139.40 pts** (50 parties, 150 simulations)
- Amélioration vs Pure MCTS: **+22.96 pts (+19.7%)**
- Taux de victoire: 72% (36/50 parties)
- Stabilité: -21% variance

**Composants:**
- 80% stratégie rollout greedy
- Scaling quadratique complétion lignes
- Bonus ×3 complétion immédiate
- Détection conflits évite gaspillage tuiles

**Status:** ✅ SUCCÈS - BASELINE OPTIMAL REPRODUCTIBLE

---

## Améliorations MCTS

### 1. Expectimax MCTS - Approche Stochastique

**Configuration:**
- Modélisation incertitude tuiles futures
- Nœuds chance pour 27 tuiles possibles
- Espérance sur tous les scénarios futurs
- 150 simulations MCTS

**Résultats:**
- Score: **1.33 pts**
- Régression: **-99.0%** (138 pts perdus)

**Analyse des 4 Niveaux d'Échec:**

| Niveau | Problème | Impact | Corrigeable |
|--------|----------|--------|-------------|
| 1 | Progressive Widening cassé | -90% | ✅ Oui |
| 2 | Explosion combinatoire (19×27=513 nœuds/niveau) | -80% | ⚠️ 500× plus de sims |
| 3 | Modèle d'information erroné | -50% | ❌ Non |
| 4 | Convergence des valeurs (SNR=0.003) | -95% | ❌ Non |

**Insight Clé:**
- Expectimax fonctionne pour Backgammon/Poker (incertitude pertinente)
- **Totalement inadapté pour Take It Easy** (incertitude résolue avant chaque décision)
- 90% du calcul gaspillé sur incertitude non-pertinente

**Status:** ❌ ÉCHEC CATASTROPHIQUE

---

### 2. Copy-on-Write (CoW) Optimization

**Configuration:**
- Rc<RefCell<>> pour partage plateau/deck
- Élimination de 880,800 opérations clone

**Résultats:**
- Performance: **8.7-12% PLUS LENT** (régression)
- Allocations économisées: -97% ✅
- Qualité score: Pas de changement

**Causes de l'Échec:**
1. Overhead Rc/RefCell > économies clone pour petits structs
2. Problèmes de localité cache (indirection détruit prefetching)
3. Coût clone surestimé: seulement 57 bytes (Tile struct)

**Status:** ⚠️ ÉCHEC PERFORMANCE - Qualité code améliorée

---

## Bugs et Investigations

### 1. Bug GroupNorm Initialization (Janvier 2026)

**Découverte:**
- `initialize_weights()` mettait TOUS les tenseurs 1D à zéro
- Y compris GroupNorm.weight qui doit être 1.0
- Bloquait tout apprentissage

**Impact:**
- Policy network bloqué distribution uniforme (loss = 2.9444 = ln(19))
- 38 itérations AlphaGo Zero = ZÉRO apprentissage (8h45 perdues)
- Tout entraînement supervisé bloqué

**Code Problématique:**
```rust
} else if size.len() == 1 {
    param.f_zero_()  // ← Zeroes GroupNorm.weight aussi!
}
```

**Solution:**
```rust
if name.ends_with(".bias") {
    param.f_zero_()  // Seulement les vrais biais
}
// GroupNorm.weight préservé à défaut PyTorch (1.0)
```

**Validation:**
- Test gradient: Loss 2.83 → 0.04 (amélioration 98.6%)
- Training supervisé: policy_loss 2.23 → 1.08 (-51% en 15 époques)

**Status:** ✅ RÉSOLU

---

### 2. Bug Générateur Données Expert

**Découverte:**
- Données générées uniformément distribuées
- Chaque position ~10 fois (supposé être "expert")
- En fait: garbage

**Impact:**
- Training sur données uniformes → aucun signal
- 20h de génération perdues
- Preuve du problème d'apprentissage circulaire

**Status:** ✅ IDENTIFIÉ

---

### 3. Baseline Irreproductible (159.95 pts)

**Investigation:**
- Documentation mentionnait 159.95 pts
- Code admettait "NOT reproducible"
- Aucun poids fonctionnel trouvé

**Résultats Investigation:**
- Branch master: 12.75 pts (test CNN-only)
- Branch feat: 12.75 pts (identique)
- Poids non-entraînés/cassés
- Performance = 100% rollouts MCTS, 0% NN

**Conclusion:**
- La cible 140+ pts était aspirationnelle, pas historiquement atteinte
- Baseline reproductible actuelle: ~139 pts (Pattern Rollouts V2)
- Les poids CNN n'ont jamais été correctement entraînés

**Status:** ✅ CLARIFIÉ

---

### 4. Investigation CNN vs MCTS (Janvier 2026)

**Découverte:**
3 problèmes fondamentaux identifiés:

1. **Géométrie Cassée**
   - 10/15 lignes de scoring en zigzag dans le tenseur 5×5
   - CNN aveugle à 67% de la géométrie du jeu

2. **CNN Polluant Value Estimates**
   - Chemin Neural utilisait prédictions CNN (mauvaises)
   - Chemin Pure utilisait rollouts (corrects)

3. **Filtrage/Tri Moves par CNN**
   - Bons moves éliminés avant évaluation rollouts
   - CNN contrôlait quels moves MCTS pouvait considérer

**Fix Appliqué:**
- Features lignes explicites (47 canaux)
- Rollouts pour value_estimates (pas CNN)
- Désactivation filtrage CNN
- w_cnn = 0.00 partout

**Résultat:**
- Avant: 12 pts
- Après: 99.4 pts (CNN contourné)

**Status:** ✅ CONTOURNÉ (pas résolu fondamentalement)

---

## Tableau Comparatif Final

| Approche | Score | Status | Notes |
|----------|-------|--------|-------|
| **Pattern Rollouts V2** | **139.40 pts** | ✅ OPTIMAL | Reproductible, stable |
| GNN Bronze | 144 pts | ⚠️ Instable | Gains marginaux |
| Pure MCTS (150 sims) | ~105 pts | ✅ Baseline | Fiable |
| Pure MCTS (100 sims) | 103.3 pts | ✅ Baseline | Fiable |
| MCTS+CNN (contourné) | 99.4 pts | ⚠️ Dégradé | CNN désactivé |
| Pure MCTS | 84-88 pts | ✅ Baseline | Base de comparaison |
| GNN Supervisé | 60.97 pts | ❌ Échec | Haute entropie |
| Random | ~50 pts | — | Minimum |
| CNN (avant fix) | 12 pts | ❌ Catastrophe | Détruisait MCTS |
| Expectimax MCTS | 1.33 pts | ❌ Catastrophe | -99% régression |

---

## Leçons Apprises

### Ce Qui Fonctionne ✅

1. **MCTS avec Rollouts Heuristiques**
   - Pattern Rollouts V2 = 139.40 pts
   - Heuristiques de domaine capturent connaissance
   - Stable et reproductible

2. **Détection de Conflits**
   - Évite placement de tuiles incompatibles
   - Améliore qualité rollouts

3. **Progressive Widening**
   - Balance exploration/exploitation
   - Paramètres actuels optimaux

### Ce Qui Ne Fonctionne Pas ❌

1. **CNN pour Géométrie Hexagonale**
   - Convolutions 2D ne captent pas lignes diagonales
   - Architecture fondamentalement inadaptée

2. **GNN pour ce Problème**
   - Moins adapté que CNN (ironiquement)
   - Instabilité, haute entropie
   - Gains marginaux vs complexité

3. **Apprentissage Circulaire**
   - Auto-génération de données = plafond
   - Impossible de bootstrapper qualité
   - Besoin source externe de connaissance

4. **Expectimax pour Take It Easy**
   - Incertitude résolue avant décision
   - Modèle d'information erroné
   - Explosion combinatoire

5. **Optimisation Prématurée**
   - CoW: -10% performance
   - Clone pas le goulot d'étranglement
   - Profilage avant optimisation

### Insights Stratégiques

1. **Simplicité > Élégance**
   - Pattern Rollouts simples: 139 pts
   - GNN complexe: 60 pts
   - Expectimax élégant: 1.33 pts

2. **Contribution Réseau Actuelle: 0 pts**
   - 100% performance vient des rollouts/heuristiques
   - Réseau ne contribue rien positivement

3. **Problème de Bootstrap**
   - Self-play atteint plafond ~85 pts
   - Besoin algorithme différent ou données externes

---

## Recommandations

### Immédiat (Haut ROI)

1. **Adopter Pattern Rollouts V2 comme Production**
   - Score: 139.40 pts (prouvé reproductible)
   - Risque: Faible
   - Code: Propre, documenté

2. **Garder CNN Contourné**
   - Actuellement contribue 0 pts
   - Réactiver seulement avec architecture alternative

### Exploration Future (Si Ressources)

| Priorité | Option | Score Attendu | Effort | Risque |
|----------|--------|---------------|--------|--------|
| 1 | Tuning paramètres MCTS | +5-10 pts | Faible | Faible |
| 2 | MCTS Parallèle | 6-8× speedup | Moyen | Moyen |
| 3 | Architecture Attention/Transformer | +20-40 pts | Élevé | Élevé |
| 4 | Données externes (humains experts) | +30-50 pts | Très élevé | Moyen |

### Ne Pas Poursuivre

- ❌ GNN (gains marginaux, instabilité)
- ❌ Expectimax MCTS (structurellement inadapté)
- ❌ Optimisations CoW (régression performance)
- ❌ CNN standard (géométrie cassée)

---

## Fichiers de Référence

### Documentation Principale
| Fichier | Description |
|---------|-------------|
| `RESUME_RAPIDE.md` | Résumé état actuel |
| `INVESTIGATION_CNN_MCTS_2026-01-18.md` | Analyse technique CNN |
| `docs/pattern_rollouts_final_results.md` | Résultats Pattern Rollouts V2 |
| `docs/EXPECTIMAX_4_LEVELS_OF_FAILURE.md` | Analyse échec Expectimax |

### Code Clé
| Fichier | Rôle |
|---------|------|
| `src/mcts/algorithm.rs` | MCTS principal avec fixes |
| `src/mcts/hyperparameters.rs` | Paramètres (w_cnn=0) |
| `src/neural/tensor_conversion.rs` | Encodage 47 canaux |
| `src/bin/debug_geometry.rs` | Outil diagnostic géométrie |

---

## Conclusion

Après 2 mois d'exploration intensive:

1. **Le CNN/GNN n'apporte aucune valeur** actuellement
2. **Pattern Rollouts V2 (139.40 pts)** est le meilleur résultat reproductible
3. **Le problème de géométrie hexagonale** est fondamental pour les architectures convolutionnelles
4. **L'apprentissage circulaire** empêche le bootstrap de qualité

Pour progresser au-delà de 140 pts, il faudrait:
- Une architecture qui respecte la topologie hexagonale (Transformer? Attention guidée?)
- Ou des données externes de qualité (parties humaines expertes)
- Ou un algorithme d'apprentissage différent (pas self-play pur)

Le code actuel est stable à 99-103 pts avec CNN contourné, ou 139 pts avec Pattern Rollouts V2.
