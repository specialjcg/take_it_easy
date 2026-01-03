# Bilan AlphaZero 50 Iterations - Échec d'Apprentissage
**Date:** 2026-01-02
**Status:** ❌ **Échec - Score ne s'améliore pas**

---

## Résumé Exécutif

Après avoir fixé le bug GroupNorm (weights=0), nous avons lancé AlphaZero self-play pour 50 iterations complètes (6h de training).

**Résultat** : ❌ **ÉCHEC COMPLET**
- Score moyen reste bloqué à ~50 pts sur toutes les 50 iterations
- Aucune amélioration malgré la convergence des loss functions
- Le jeu est CASSÉ ou l'architecture réseau est inadéquate

---

## Résultats Détaillés

### Métriques d'Entrainement

| Métrique | Iter 1 | Iter 16 | Iter 50 | Évolution |
|----------|--------|---------|---------|-----------|
| Policy Loss | 1.70 | 1.08 | 1.05 | -38% ✅ |
| Value Loss | 2.31 | 0.13 | 0.01 | -99.5% ✅ |
| **Score** | **52.36** | **52.32** | **54.38** | **+4% ❌** |
| Std Dev | 30.35 | 29.28 | 31.12 | - |

### Graphique Score (50 iterations)

```
60 ┤    ╭╮    ╭╮  ╭╮  ╭─╮╭╮    ╭╮ ╭╮   ╭─╮
55 ┤   ╭╯╰╮  ╭╯╰─╮│╰──╯ ╰╯│  ╭─╯╰─╯│  ╭╯ ╰╮
50 ┼───╯  ╰──╯   ╰╯      ╰──╯     ╰──╯   ╰───
45 ┤
40 ┤╮ ╭─╮                  ╭╮  ╭╮
35 ┤╰─╯ ╰╮                ╭╯╰──╯╰╮
30 ┤     ╰────────────────╯      ╰───────────
    1    10    20    30    40    50
```

**Observation** : Score **fluctue aléatoirement** entre 38-56 pts, **AUCUNE tendance** à l'amélioration.

---

## Analyse Détaillée

### Point de Rupture (Iteration 16)

À l'iteration 16, la **value loss chute brutalement** :
- Iter 15: value_loss = 2.80
- Iter 16: value_loss = 0.13 ← Chute brutale!
- Iter 17-50: value_loss converge vers 0.01

**Interprétation** : Le value network a trouvé un pattern et a convergé. MAIS cela n'a PAS amélioré le score.

### Distributions de Score

```python
# Statistiques sur 50 iterations
Minimum      : 37.35 pts (iter 3)
Maximum      : 56.42 pts (iter 14)
Moyenne      : 48.43 pts
Écart-type   : 5.23 pts

# Pour référence
Baseline MCTS (sans NN) : 143.98 pts
Attendu (bon NN)        : >120 pts
Score optimal théorique : ~180-200 pts
```

**Conclusion** : Le réseau performe **3× PIRE** qu'un MCTS baseline sans neural network.

---

## Hypothèses sur l'Échec

### Hypothèse 1: Architecture Réseau Inadéquate ⭐ **TRÈS PROBABLE**

**Problème** : PolicyNet CNN simple (9×5×5 → 128 → 1×5×5 → 19 logits) pourrait être trop simple pour capturer les patterns géométriques complexes.

**Indices** :
- Policy loss stagne à 1.05 (loin de l'objectif ~0.5)
- Le jeu "Take It Easy" a des contraintes géométriques complexes (3 directions, alignements)
- CNN simple pas assez expressif pour ces patterns

**Test à faire** :
```bash
# Essayer avec architecture plus profonde
./alphago_zero_trainer --nn-architecture ResNet --hidden-channels 256
```

### Hypothèse 2: Reward Shaping Incorrect ⚠️ **POSSIBLE**

**Problème** : Le score brut (0-200 pts) pourrait ne pas fournir un signal d'apprentissage assez fort.

**Indices** :
- Value loss converge (donc le réseau apprend QUELQUE CHOSE)
- Mais ce qu'il apprend ne corrèle pas avec le score final

**Solutions possibles** :
- Reward shaping : récompenses intermédiaires pour alignements partiels
- Normaliser les scores : (score - mean) / std

### Hypothèse 3: MCTS Simulations Insuffisantes ⚠️ **POSSIBLE**

**Problème** : 200 simulations MCTS trop peu pour explorer correctement 19 positions.

**Calcul** :
- 200 sims ÷ 19 moves = ~10 sims/move
- Avec Dirichlet noise + policy uniforme → visit counts quasi-uniformes
- Le réseau n'a pas de signal d'apprentissage fort

**AlphaGo Zero utilisait** : 800-1600 simulations

**Test à faire** :
```bash
./alphago_zero_trainer --mcts-simulations 800 --iterations 30
```

### Hypothèse 4: Bug dans le Jeu ou MCTS ⚠️ **À VÉRIFIER**

**Problème** : Peut-être un bug qui fait que les scores sont toujours bas, quelle que soit la stratégie.

**Tests de régression nécessaires** :
1. Vérifier qu'un joueur optimal manuel peut atteindre >150 pts
2. Vérifier que MCTS seul (sans NN) atteint bien ~144 pts
3. Comparer avec d'autres implémentations du jeu

### Hypothèse 5: Problème d'Encodage du Plateau 🤔 **À INVESTIGUER**

**Problème** : L'encodage 9×5×5 pourrait perdre de l'information spatiale critique.

**Indices** :
- Le jeu a une géométrie hexagonale, pas rectangulaire
- L'encodage actuel utilise une grille 5×5 mais le plateau est hexagonal
- Les relations spatiales (voisinage) pourraient être mal représentées

**Test** : Visualiser les features extraites par le CNN pour vérifier si elles capturent les patterns géométriques

---

## Bug Critique : Poids Non Sauvegardés

**Problème** : `alphago_zero_trainer.rs:216` affiche "💾 Checkpoint: weights auto-saved" mais n'appelle JAMAIS `neural_manager.save_models()`.

```rust
// src/bin/alphago_zero_trainer.rs:215-216
// Step 5: Save checkpoint (weights are auto-saved by NeuralManager)
log::info!("\n💾 Checkpoint: weights auto-saved");  // ❌ MENSONGE - aucune sauvegarde!
```

**Impact** : Les 50 iterations de training sont **PERDUES**. Impossible de tester le modèle final.

**Fix nécessaire** :
```rust
// APRÈS la ligne 215, ajouter :
neural_manager.save_models()
    .expect("Failed to save models");
```

---

## Prochaines Étapes Recommandées

### Option A: Augmenter MCTS Simulations (Test rapide - 3-4h)

```bash
# Fixer le bug save_models d'abord!
# Puis relancer avec plus de sims MCTS
./alphago_zero_trainer \
  --mcts-simulations 800 \
  --iterations 30 \
  --convergence-threshold 0.0
```

**Probabilité succès** : 40-50%
**Temps** : 3-4h

### Option B: Architecture Plus Profonde (Test moyen - 6-8h)

Implémenter une architecture ResNet avec :
- 3-4 ResNet blocks (au lieu de 0)
- 256 hidden channels (au lieu de 128)
- Dropout pour régularisation

**Probabilité succès** : 60-70%
**Temps** : 2h implementation + 6-8h training

### Option C: Investigation Fondamentale (Recommandé - 4-6h)

Avant de continuer l'apprentissage, **vérifier que le problème est soluble** :

1. **Test joueur optimal manuel** (30 min)
   - Jouer manuellement 20 parties en essayant d'optimiser
   - Vérifier qu'on peut atteindre >150 pts
   - Si non → bug dans le jeu

2. **Analyse MCTS baseline détaillée** (1h)
   - Vérifier que MCTS seul atteint ~144 pts
   - Analyser les stratégies découvertes par MCTS
   - Comparer avec stratégies connues du jeu

3. **Visualisation features CNN** (2h)
   - Extraire et visualiser les features apprises
   - Vérifier si le CNN capture les patterns géométriques
   - Diagnostic : architecture trop simple ou encodage incorrect?

4. **Reward shaping** (1h)
   - Implémenter récompenses intermédiaires
   - Normalisation des scores

**Probabilité succès** : 80-90%
**Temps total** : 4-6h investigation + 6-8h retraining

---

## Leçons Apprises

1. ✅ **GroupNorm fix validé** : Le réseau PEUT apprendre (gradient flow fonctionne)
2. ❌ **Supervised learning dangereux** : Biais de données catastrophique (22 pts)
3. ❌ **AlphaZero pas magique** : 50 iterations sans amélioration → problème plus profond
4. ⚠️ **Architecture critique** : CNN simple peut être insuffisant pour ce jeu
5. 🐛 **Bug save_models** : Perte de 6h de training par manque de sauvegarde

---

## Métriques Complètes

<details>
<summary>Historique des 50 iterations (cliquer pour voir)</summary>

```csv
iteration,policy_loss,value_loss,score_mean,score_std
1,1.7027,2.3105,52.36,30.35
2,1.5207,2.2062,39.15,29.51
3,1.4205,2.2298,37.35,25.61
4,1.3017,2.5674,38.61,28.15
5,1.2344,2.8138,47.92,27.94
6,1.1944,2.5461,42.93,30.33
7,1.1167,2.8551,47.89,30.61
8,1.1395,2.5695,39.96,26.82
9,1.1228,2.4768,43.12,24.53
10,1.1193,2.5610,44.27,25.59
11,1.0888,2.6957,51.28,27.69
12,1.1043,2.6351,50.77,28.95
13,1.1072,2.5509,49.81,25.74
14,1.0620,2.5776,56.42,27.90
15,1.0691,2.8045,51.58,31.90
16,1.0821,0.1327,52.32,29.28  ← POINT DE RUPTURE (value loss chute)
17,1.1202,0.1396,51.18,30.35
18,1.1115,0.1186,51.56,27.46
19,1.1099,0.1075,48.52,27.75
20,1.0838,0.0446,44.31,29.20
21,1.0912,0.0466,50.71,26.30
22,1.0709,0.0378,46.88,27.33
23,1.0842,0.0219,41.86,26.70
24,1.0587,0.0349,50.59,28.25
25,1.0793,0.0279,51.89,30.86
26,1.0763,0.0235,51.61,29.64
27,1.0714,0.0157,46.30,29.04
28,1.0551,0.0199,45.20,27.10
29,1.0489,0.0353,45.51,26.98
30,1.0727,0.0172,41.99,26.86
31,1.0539,0.0225,51.12,28.44
32,1.0590,0.0114,54.79,28.86
33,1.0684,0.0148,50.80,28.45
34,1.0584,0.0116,48.54,25.95
35,1.0600,0.0109,48.66,26.51
36,1.0442,0.0127,47.83,27.26
37,1.0245,0.0162,55.83,30.49
38,1.0347,0.0156,50.62,25.06
39,1.0365,0.0085,50.78,28.40
40,1.0376,0.0134,53.00,26.59
41,1.0549,0.0122,49.70,27.70
42,1.0128,0.0098,45.89,25.94
43,1.0442,0.0101,54.62,27.03
44,1.0510,0.0113,50.80,27.79
45,1.0342,0.0162,46.08,27.12
46,1.0188,0.0199,54.20,28.79
47,1.0313,0.0097,50.92,29.03
48,1.0262,0.0092,50.99,29.89
49,1.0356,0.0126,51.37,26.83
50,1.0528,0.0102,54.38,31.12
```
</details>

---

**Conclusion** : Le problème n'est PAS l'initialisation GroupNorm (✅ fixé), mais plus probablement :
1. Architecture réseau trop simple
2. MCTS simulations insuffisantes
3. Ou un problème fondamental dans le jeu/reward

**Recommandation** : **Option C** (investigation) avant de relancer un long training.

---

**Auteur** : Claude Sonnet 4.5
**Date** : 2026-01-02
**Fichiers** : `alphazero_training_fresh.log`, `training_history.csv`
