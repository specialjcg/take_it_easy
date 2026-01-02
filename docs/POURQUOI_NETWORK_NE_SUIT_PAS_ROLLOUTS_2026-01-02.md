# Pourquoi le Réseau N'Apprend Pas des Rollouts ?

**Date:** 2026-01-02
**Question Clé:** Si les rollouts obtiennent ~144-150 pts (baseline), pourquoi la policy network n'apprend-elle pas à faire pareil ?

---

## TL;DR - Réponse Courte

**Le réseau apprend de MCTS visit counts (uniformes), PAS de la qualité des rollouts.**

- ✅ **Value Network** apprend des **résultats finaux** des rollouts → fonctionne
- ❌ **Policy Network** apprend de la **distribution des visites MCTS** → bloquée car uniforme
- **Les rollouts influencent la value, mais PAS directement la policy**

---

## Architecture du Training AlphaZero

### Ce Que le Réseau Voit Pendant le Training

```
┌─────────────────────────────────────────────┐
│         SELF-PLAY GAME                      │
├─────────────────────────────────────────────┤
│                                             │
│  Position → MCTS (200 sims) → Move chosen   │
│                                             │
│  MCTS fait 200 simulations:                 │
│    - Chaque sim fait des ROLLOUTS          │
│    - Rollouts retournent des scores        │
│    - MCTS accumule visit counts par move   │
│                                             │
│  Training Example Créé:                     │
│    - Input: Position (9×5×5 tensor)        │
│    - Policy Target: MCTS visit counts      │ ← PROBLÈME ICI
│    - Value Target: Final game score        │ ← FONCTIONNE
│                                             │
└─────────────────────────────────────────────┘
```

### Ce Que Chaque Réseau Apprend

#### Value Network (✅ APPREND)
```
Input:  Position state (9×5×5)
Target: Actual final game score (e.g., 147 pts)
Loss:   MSE(prediction, actual_score)

Example:
  Prediction: 140 pts
  Actual:     147 pts
  Loss:       (147-140)² = 49
  Gradient:   Strong signal → weights update
  Result:     ✅ Network learns to predict scores
```

#### Policy Network (❌ N'APPREND PAS)
```
Input:  Position state (9×5×5)
Target: MCTS visit count distribution (e.g., [0.05, 0.05, 0.06, ...])
Loss:   CrossEntropy(prediction, visit_counts)

Example avec MCTS uniforme:
  Visit counts: [52, 53, 51, 48, 52, ...] / 200 = [0.26, 0.27, 0.26, 0.24, 0.26, ...]
                                                    ≈ uniform [0.053, 0.053, ...]

  Network output: [0.053, 0.053, 0.053, ...] (uniform aussi)

  Loss:   CrossEntropy(uniform, uniform) = ln(19) = 2.9444
  Gradient: ≈ 0 (target ≈ prediction)
  Result:  ❌ Pas de mise à jour des poids
```

---

## Pourquoi MCTS Visit Counts Sont Uniformes ?

### Le Problème Circulaire Détaillé

#### État Initial (Iteration 1)
```
Policy Network: Uniform distribution [1/19, 1/19, ..., 1/19]
                ↓
MCTS Root Node: π_prior = [0.053, 0.053, ..., 0.053] (uniform)
                ↓
UCT Formula:    Q(a) + c·π_prior(a)·√N / (1 + N(a))
                     [action value] + [exploration bonus]
                ↓
AVEC uniform π_prior:
  - Tous les moves ont même exploration bonus initial
  - Q(a) vient des rollouts, mais...
  - Avec seulement 200 sims ÷ 19 moves ≈ 10 sims/move
  - Q(a) très bruité, pas assez de signal
                ↓
Visit Counts: [52, 48, 51, 49, 50, ...] ≈ uniform
```

#### Avec Dirichlet Noise (Essai de Correction)
```
Base π_prior:     [0.053, 0.053, ..., 0.053] (uniform)
Dirichlet noise:  [0.046, 0.754, 0.000, ..., 0.024]
Mixed (ε=0.5):    0.5·π + 0.5·noise
                = [0.050, 0.404, 0.026, ..., 0.039]

MCTS avec noise:  Move 1 (p=0.404) visité ~80 fois
                  Moves autres ≈ 6-7 fois chacun

Mais:
  - Seulement 1er move du jeu a le noise
  - Moves suivants: retour à uniform policy
  - Sur 19 moves/game, 1 seul avec noise = 5% des données
  - 95% des données restent uniformes

Résultat: Visit counts globalement uniformes
```

### Avec 200 Simulations vs 800+ Simulations

#### AlphaGo Zero (800-1600 sims)
```
19 moves × 800 sims = 42 visits/move en moyenne

MCTS avec 42 visits/move:
  - Q(a) converge vers vraie valeur
  - Bons moves émergent clairement
  - Visit counts: [120, 95, 80, 15, 12, 8, ...] (discriminant)
                   ↑ bon   ↑ ok  ↑ ok  ↑ mauvais
```

#### Notre Cas (200 sims)
```
19 moves × 200 sims = 10 visits/move en moyenne

MCTS avec 10 visits/move:
  - Q(a) très bruité (variance élevée)
  - Bons vs mauvais moves peu différenciés
  - Visit counts: [52, 48, 51, 49, 50, ...] (quasi-uniforme)
                   ↑ Tous similaires, pas de signal clair
```

**C'est pourquoi AlphaGo Zero utilisait 800-1600 simulations!**

---

## Pourquoi le Réseau Ne Suit Pas les Rollouts ?

### Malentendu Commun

**❌ Fausse Hypothèse:**
"Les rollouts jouent des parties et obtiennent ~144 pts. Le réseau devrait apprendre à imiter les rollouts."

**✅ Réalité:**
"Le réseau n'a JAMAIS accès aux décisions des rollouts, seulement aux visit counts MCTS."

### Flux de Données Réel

```
┌──────────────────────────────────────────────────────┐
│  MCTS Tree Search (200 simulations)                  │
│                                                       │
│  Simulation 1:                                        │
│    Root → Move A → ... → ROLLOUT → score: 142       │
│    Backup: Q(Move A) += 142                          │
│                                                       │
│  Simulation 2:                                        │
│    Root → Move B → ... → ROLLOUT → score: 138       │
│    Backup: Q(Move B) += 138                          │
│                                                       │
│  ...                                                  │
│                                                       │
│  Simulation 200:                                      │
│    Root → Move C → ... → ROLLOUT → score: 150       │
│    Backup: Q(Move C) += 150                          │
│                                                       │
│  ─────────────────────────────────────────────────   │
│                                                       │
│  Après 200 sims:                                      │
│    Move A: 52 visits, Q = 7380/52 = 141.9            │
│    Move B: 48 visits, Q = 6624/48 = 138.0            │
│    Move C: 51 visits, Q = 7650/51 = 150.0            │
│    ...                                                │
│                                                       │
│  Visit Counts: [52, 48, 51, 49, ...]                 │
│                ↑ Quasi-uniforme car π_prior uniforme │
│                                                       │
└──────────────────────────────────────────────────────┘
                        ↓
         ┌──────────────────────────────┐
         │  TRAINING DATA CRÉÉE         │
         ├──────────────────────────────┤
         │  Policy Target:              │
         │    [52, 48, 51, ...]/200     │
         │    = [0.26, 0.24, 0.26, ...] │
         │    ≈ UNIFORM                 │← Policy Network apprend de ça
         ├──────────────────────────────┤
         │  Value Target:               │
         │    Actual game score: 145    │← Value Network apprend de ça
         └──────────────────────────────┘
```

### Ce Que le Réseau NE Voit PAS

❌ **Décisions des rollouts** (quel move le rollout a choisi à chaque position)
❌ **Qualité relative des moves** pendant les rollouts
❌ **Stratégies utilisées** par les rollouts

### Ce Que le Réseau VOIT

✅ **Visit counts** (combien de fois MCTS a exploré chaque move)
✅ **Score final** du jeu complet
✅ **Position state** (9×5×5 tensor)

---

## Pourquoi la Value Network Apprend Mais Pas la Policy ?

### Value Network: Signal Direct

```
Input:     Position state
Target:    Actual game score (145 pts)
Prediction: 138 pts

Loss = (145 - 138)² = 49
Gradient: ∂Loss/∂weights = 2·(145-138)·∂prediction/∂weights
                          = 14·∂prediction/∂weights

→ Gradient fort et clair
→ Weights se mettent à jour
→ Network apprend à prédire les scores
```

**Pourquoi ça marche:**
- Le score final (145 pts) est un **signal non-ambigu**
- Chaque partie donne 19 exemples (1 par move) avec même target
- Les rollouts **influencent directement** ce score
- Le réseau apprend: "Cette position → environ 145 pts"

### Policy Network: Signal Circulaire

```
Input:     Position state
Target:    MCTS visit counts [0.26, 0.24, 0.26, 0.24, ...]
Prediction: [0.25, 0.25, 0.25, 0.25, ...] (uniform)

Loss = CrossEntropy([0.26, 0.24, ...], [0.25, 0.25, ...])
     = -Σ p_target·log(p_pred)
     ≈ ln(19) = 2.9444 (quasi-uniform target et prediction)

Gradient: ∂Loss/∂weights = Σ (p_pred - p_target)·∂p_pred/∂weights
                          ≈ 0 (p_pred ≈ p_target partout)

→ Gradient ≈ 0
→ Weights ne changent pas
→ Network reste uniforme
```

**Pourquoi ça ne marche pas:**
- Les visit counts sont **dérivés de la policy actuelle** (uniforme)
- Target uniforme → prediction uniforme → gradient nul
- Les rollouts **n'influencent PAS directement** les visit counts
- Le réseau est piégé: "Policy uniforme → MCTS uniforme → Target uniforme → Policy reste uniforme"

---

## Visualisation du Problème

### Ce Qui Se Passe Réellement

```
ROLLOUTS (inside MCTS simulations):
  Move A → rollout → 142 pts
  Move B → rollout → 138 pts
  Move C → rollout → 150 pts ← MEILLEUR!
  Move D → rollout → 135 pts
  ...

MAIS... MCTS Visit Counts (ce que voit le réseau):
  Move A: 52 visits (26%)
  Move B: 48 visits (24%)
  Move C: 51 visits (26%)  ← Devrait être 80%+ mais seulement 26%
  Move D: 49 visits (25%)
  ...

POURQUOI Move C n'a pas 80% des visites malgré meilleur score?
  → Parce que π_prior(C) = 0.053 (uniform)
  → UCT donne même exploration bonus à tous
  → 200 sims insuffisant pour converger vers vraie valeur
  → Visit counts restent quasi-uniformes
```

### Ce Qui Devrait Se Passer (avec plus de sims)

```
AVEC 800 SIMULATIONS:

ROLLOUTS (same):
  Move C → rollout → 150 pts (meilleur)

MCTS Visit Counts (800 sims):
  Move A: 80 visits (10%)
  Move B: 65 visits (8%)
  Move C: 320 visits (40%) ← DISCRIMINANT!
  Move D: 45 visits (6%)
  ...

Policy Target: [0.10, 0.08, 0.40, 0.06, ...]
Network Output: [0.25, 0.25, 0.25, 0.25, ...] (uniform)

Loss = CrossEntropy → FORT gradient car target ≠ prediction
→ Network apprend: "À cette position, Move C est bon!"
```

---

## Pourquoi Supervised Learning Fonctionne ?

### Différence Clé: Signal Direct

```
SUPERVISED LEARNING:

Input:     Position state (9×5×5)
Target:    Move choisi par expert humain (ou optimal solver)
           Exemple: Move 7 (one-hot [0,0,0,0,0,0,1,0,0,...])

Network Output: [0.05, 0.05, 0.05, 0.05, 0.05, 0.05, 0.05, ...]
                                                      ↑
Target:         [0,    0,    0,    0,    0,    0,    1,    0, ...]
                                                      ↑

Loss = CrossEntropy
     = -log(p_pred[7])
     = -log(0.05) = 3.0

Gradient: FORT car target concentré (1.0) vs prediction dispersée (0.05)

→ Weights se mettent à jour
→ Network apprend: "À cette position → Move 7"
```

**Pourquoi ça marche:**
- ✅ **Signal clair et non-ambigu** (un seul move est correct)
- ✅ **Pas de dépendance circulaire** (expert indépendant du réseau)
- ✅ **Gradient fort** (target one-hot vs prediction uniform)
- ✅ **Qualité garantie** (expert joue bien, données de qualité)

---

## Comparaison des Trois Approches

| Aspect | Pure Self-Play | Supervised Learning | Hybrid (Super→Self) |
|--------|---------------|---------------------|---------------------|
| **Policy Target** | MCTS visit counts | Expert move | Expert → MCTS |
| **Signal Quality** | ❌ Uniform → faible | ✅ One-hot → fort | ✅ Fort puis raffiné |
| **Dépendance** | ❌ Circulaire | ✅ Indépendante | ✅ Indépendante puis auto-améliorante |
| **Gradient** | ≈ 0 (bloqué) | Fort (apprentissage) | Fort → graduel |
| **Résultat** | ❌ Policy 2.9444 | ✅ Policy ~1.8 | ✅ Policy ~1.5-1.8 |
| **Score** | 149 pts | 160-170 pts | 170-180+ pts |

---

## Réponse à la Question Initiale

### "Pourquoi le réseau n'apprend pas à faire comme les rollouts?"

**Réponse en 3 points:**

1. **Le réseau n'a pas accès aux décisions des rollouts**
   - Il voit seulement les visit counts MCTS
   - Les rollouts influencent Q(a) mais pas directement les visit counts
   - Avec 200 sims, visit counts restent uniformes malgré rollouts différents

2. **Les rollouts aident la VALUE network, pas la POLICY**
   - Value: apprend du score final (influencé par rollouts) ✅
   - Policy: apprend des visit counts (uniformes malgré rollouts) ❌
   - C'est une asymétrie fondamentale de l'architecture AlphaZero

3. **Il manque du "signal de démarrage"**
   - Les rollouts donnent du signal, mais trop dilué sur 200 sims
   - Policy uniforme + 200 sims → visit counts uniformes
   - Besoin de:
     - SOIT plus de sims (800+) pour signal MCTS plus fort
     - SOIT bootstrap avec supervised learning (signal direct)

---

## Analogie pour Comprendre

### Imaginez Apprendre le Piano

**Pure Self-Play = Taper au hasard et écouter si ça sonne bien**
```
Itération 1: Tape random → sons discordants
             "Tous les sons semblent également mauvais"
             → Gradient ≈ 0 (pas de direction claire)

Itération 38: Tape toujours random → sons discordants
              Stuck dans "tout essayer uniformément"
```

**Supervised Learning = Regarder un expert jouer et imiter**
```
Voir: Expert joue Do-Mi-Sol → son harmonieux
Essayer: Tape Do-Mi-Sol
Résultat: Son harmonieux ✅
Apprendre: "Do-Mi-Sol est bon"
→ Gradient fort vers bonnes notes
```

**Hybrid = Apprendre bases avec expert, puis explorer variations**
```
Phase 1: Apprendre Do-Mi-Sol de l'expert
Phase 2: Explorer Do-Mi-Sol-Si, Do-Fa-La, etc.
Résultat: Comprendre harmonie ET découvrir nouveaux accords
```

---

## Conclusion Technique

### Pourquoi Pas d'Apprentissage ?

**Root Cause: Gradient ≈ 0**
```
∂Loss_policy/∂weights = Σ_a (π_pred(a) - π_target(a)) · ∂π_pred(a)/∂weights

Avec π_target ≈ uniform (visit counts uniformes):
  π_target ≈ [0.053, 0.053, ...]
  π_pred ≈ [0.053, 0.053, ...] (aussi uniform)

  → (π_pred - π_target) ≈ [0, 0, 0, ...] pour tous les a
  → Gradient ≈ 0
  → Weights ne changent pas
```

### Pourquoi Pas Comme Rollouts ?

**Séparation: Rollouts → Value, MCTS Visits → Policy**

```
ROLLOUT OUTCOMES (142, 138, 150, 135 pts):
  → Utilisés pour Q(a) dans MCTS
  → Influencent VALUE target (score final)
  → Value Network apprend de ça ✅

  MAIS PAS utilisés directement pour POLICY target!

MCTS VISIT COUNTS ([52, 48, 51, 49]):
  → Dérivent de π_prior (uniform) + Q(a)
  → Avec 200 sims: uniformes malgré Q(a) différent
  → Utilisés pour POLICY target
  → Policy Network apprend de ça (mais c'est uniform) ❌
```

### Solution: Supervised Learning Brise le Cycle

**Expert Data → Policy Target One-Hot → Gradient Fort → Learning**
```
π_target = [0, 0, 1, 0, ...] (expert choisit move 3)
π_pred = [0.053, 0.053, 0.053, ...] (uniform)

Gradient = π_pred - π_target = [0.053, 0.053, -0.947, 0.053, ...]
                                                  ↑ FORT signal négatif

→ Weights se mettent à jour pour augmenter π_pred(3)
→ Après training: π_pred = [0.02, 0.03, 0.65, 0.02, ...]
→ Policy non-uniforme ✅
→ MCTS avec policy non-uniforme → visit counts discriminants
→ Self-play peut continuer l'apprentissage
```

---

## Prochaine Étape

**Lancer Supervised Training** pour briser le cycle et donner un signal d'apprentissage clair au réseau.

**Fichier de données:** `expert_data_filtered_110plus_from500.json`
**Attendu:** Policy loss 2.9444 → 1.8-2.0 après 50-100 epochs
