# 📋 Résumé rapide - Où on en est ?

**Date:** 18 janvier 2026 (mis à jour)

## 🎯 Situation actuelle

**Problème résolu:** Le CNN détruisait MCTS (12 pts vs 100 pts)

**Cause identifiée:** 3 problèmes fondamentaux (voir ci-dessous)

**État actuel:** CNN contourné, MCTS+NN = 99.4 pts (vs Pure MCTS = 103.3 pts)

---

## 🔍 Causes Racines Identifiées

### 1. Géométrie Cassée (CRITIQUE)
- 5/15 lignes de scoring sont droites dans le tenseur 5×5
- 10/15 lignes sont en zigzag → CNN aveugle à 2/3 du jeu
- **Fix:** Ajout 30 canaux de features de lignes explicites (47 total)

### 2. CNN Polluant les Value Estimates
- Chemin Neural utilisait CNN pour value_estimates
- Chemin Pure utilisait rollouts (correct)
- **Fix:** Utiliser rollouts dans les deux chemins

### 3. Filtrage/Tri des Moves par CNN
- Bons moves éliminés avant évaluation
- **Fix:** Désactiver filtrage, utiliser tous les moves

---

## 📊 Résultats (18 janvier 2026)

| Mode | Score Moyen | Notes |
|------|-------------|-------|
| Random | ~50 pts | Baseline |
| Pure MCTS (100 sims) | 103.3 pts | Référence |
| **MCTS + CNN (avant fix)** | **12 pts** | ❌ Catastrophique |
| **MCTS + CNN (après fix)** | **99.4 pts** | ✅ Quasi-égal Pure |

---

## ⚡ Commandes Utiles

### Tester performance actuelle
```bash
cargo run --release --bin compare_mcts -- \
  --games 30 --simulations 100 --nn-architecture cnn
```

### Analyser géométrie
```bash
cargo run --release --bin debug_geometry
```

### Entraîner CNN (47 canaux)
```bash
rm -rf model_weights/cnn
cargo run --release --bin supervised_trainer_csv -- \
  --data supervised_130plus_filtered.csv \
  --epochs 150 --batch-size 64 \
  --policy-lr 0.001 --value-lr 0.0001 \
  --nn-architecture cnn
```

---

## 🚀 Prochaines Étapes Recommandées

### Option A: GNN (Recommandée)
- Architecture qui respecte la topologie hexagonale
- Message passing le long des 15 lignes de scoring
- Plus adapté au problème

### Option B: Améliorer CNN
- Convolutions asymétriques (1×5)
- Skip connections vers features de lignes
- Attention guidée par géométrie

### Option C: Entraînement Progressif
- Commencer avec w_cnn=0.01
- Augmenter si CNN améliore les scores
- Validation sur performance MCTS réelle

---

## 📁 Fichiers Importants

| Fichier | Description |
|---------|-------------|
| `INVESTIGATION_CNN_MCTS_2026-01-18.md` | Rapport technique complet |
| `src/bin/debug_geometry.rs` | Outil de visualisation géométrie |
| `src/neural/tensor_conversion.rs` | Encodage 47 canaux |
| `src/mcts/algorithm.rs` | MCTS avec fixes appliqués |
| `src/mcts/hyperparameters.rs` | Poids CNN désactivés |

---

## ⚠️ État du Code

Le CNN est actuellement **entièrement contourné**:
- `weight_cnn = 0.00` partout
- `value_estimates` viennent des rollouts, pas du CNN
- Pas de filtrage/tri par policy CNN

Pour réactiver le CNN, il faut d'abord résoudre le problème de géométrie (GNN recommandé).

---

## 📖 Documentation

| Document | Description |
|----------|-------------|
| `HISTORIQUE_EXPLORATIONS_COMPLET.md` | **NOUVEAU** - Résumé consolidé de TOUTES les explorations |
| `INVESTIGATION_CNN_MCTS_2026-01-18.md` | Analyse technique CNN détaillée |
| `docs/pattern_rollouts_final_results.md` | Résultats Pattern Rollouts V2 (139.40 pts) |
| `docs/EXPECTIMAX_4_LEVELS_OF_FAILURE.md` | Analyse échec Expectimax |

---

## 📊 Historique des Explorations (Résumé)

### Architectures Testées
| Architecture | Score | Status |
|--------------|-------|--------|
| Pattern Rollouts V2 | **139.40 pts** | ✅ OPTIMAL |
| GNN Bronze | 144 pts | ⚠️ Instable |
| Pure MCTS | 103.3 pts | ✅ Baseline |
| GNN Supervisé | 60.97 pts | ❌ Échec |
| CNN (avant fix) | 12 pts | ❌ Catastrophe |
| Expectimax | 1.33 pts | ❌ Catastrophe |

### Ce Qui Ne Fonctionne Pas
- ❌ CNN standard (géométrie cassée - 10/15 lignes invisibles)
- ❌ GNN (instable, haute entropie)
- ❌ Expectimax (modèle d'information erroné)
- ❌ Apprentissage circulaire (self-play plafonne)

Voir `HISTORIQUE_EXPLORATIONS_COMPLET.md` pour détails complets.
