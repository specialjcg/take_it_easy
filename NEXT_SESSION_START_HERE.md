# 🚀 START HERE - Prochaine Session Claude

**Date:** 2025-12-29
**Status:** UCT MCTS implémenté, Self-Play convergé à 146.92 pts

---

## ⚡ Action Immédiate (5 min setup)

### 1. Vérifier l'état actuel
```bash
cd /home/jcgouleau/IdeaProjects/RustProject/take_it_easy
git status
git log -1 --oneline
# Devrait montrer: 1846218 feat(mcts): implement UCT algorithm...
```

### 2. Lire le plan complet
```bash
cat docs/ROADMAP_PERFORMANCE_2025-12-29.md
```

### 3. Performance actuelle
```
✅ Batch MCTS (baseline): 74.22 pts
✅ UCT MCTS: 149.36 pts (+101%)
✅ Self-Play: 146.92 pts (+97%)
🎯 Target: 180-200 pts
📊 Gap: +33-53 pts à combler
```

---

## 🎯 Prochaine Étape Recommandée

### Option A: Test Rapide (2-3h) - RECOMMANDÉ
Lancer training étendu SANS changement de code:

```bash
cargo build --release

./target/release/alphago_zero_trainer \
  --games-per-iter 100 \
  --convergence-threshold 10.0 \
  --iterations 50 \
  --mcts-simulations 200 \
  --epochs-per-iter 15 \
  --output training_history_extended.csv

# Laisser tourner, résultats attendus: 155-165 pts
```

### Option B: Implémenter Dirichlet Noise (4-6h) - MAX IMPACT
C'est la modification la plus impactante (+15-30 pts):

1. Ouvrir `src/bin/alphago_zero_trainer.rs`
2. Aller à la ligne ~248 (fonction `generate_self_play_games`)
3. Ajouter Dirichlet noise AVANT l'appel MCTS (voir ROADMAP section 1.1)
4. Rebuild et tester

Résultat attendu: policy_loss < 2.80 (vs 2.9444 actuel)

---

## 📋 Checklist Priorités

### Phase 1: Quick Wins (START HERE)
- [ ] 1.1 Dirichlet Noise (PRIORITÉ #1) → +15-30 pts
- [ ] 1.2 Temperature Sampling → +5-15 pts
- [ ] 1.3 Augmenter Volume Training → +10-20 pts

### Phase 2: Optimisation
- [ ] 2.1 Rollout Count (5→15)
- [ ] 2.2 Value Normalization
- [ ] 2.3 Learning Rate Schedule

### Phase 3: Architecture
- [ ] 3.1 Tester GNN
- [ ] 3.2 ResNet Blocks
- [ ] 3.3 Experience Replay

---

## 📊 Métriques à Surveiller

```bash
# Pendant training:
tail -f training_history_extended.csv

# Chercher:
policy_loss: doit diminuer de 2.9444 → 2.80 → 2.50 → 2.00
value_loss: doit diminuer de 1.50 → 1.00 → 0.70 → 0.50
score: doit augmenter de 147 → 160 → 170 → 180+
```

---

## 🔧 Fichiers Clés

```
docs/ROADMAP_PERFORMANCE_2025-12-29.md  ← Plan complet détaillé
src/bin/alphago_zero_trainer.rs         ← Main training loop
src/mcts/algorithm.rs                   ← UCT implementation
training_history.csv                    ← Résultats actuels
```

---

## 💡 Contexte Rapide

**Problème identifié:** Circular Learning
- Policy uniforme → UCT uniforme → Données uniformes → Policy uniforme

**Solution:** Dirichlet Noise (Phase 1.1)
- Force l'exploration même avec policy uniforme
- Technique clé d'AlphaGo Zero

**Commit actuel:** 1846218
- UCT MCTS: +101% performance
- Self-play infrastructure complète
- Prêt pour amélioration

---

## 🚨 Quick Commands

```bash
# Build
cargo build --release

# Training rapide (2h)
./target/release/alphago_zero_trainer --games-per-iter 50 --iterations 20

# Training long (overnight)
./target/release/alphago_zero_trainer --games-per-iter 100 --iterations 50 --convergence-threshold 10.0

# Benchmark
./target/release/compare_batch_vs_uct
```

---

**Recommandation:** Commencer par Option A (test rapide) pour établir nouvelle baseline, puis passer à Option B (Dirichlet) pour maximiser gains.
