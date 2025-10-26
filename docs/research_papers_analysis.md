# Analyse des Pistes de Recherche pour Take It Easy

## Contexte du Jeu

**Take It Easy** est un jeu avec :
- ✅ Information parfaite (tous les joueurs voient les tuiles jouées)
- ✅ Stochasticité importante (tirage aléatoire de 19 tuiles parmi 27)
- ❌ PAS d'information cachée (pas de rôles secrets, pas de cartes cachées)
- ❌ PAS multi-agents adversaires (chaque joueur optimise son propre plateau)

**Baseline actuel** : Pattern Rollouts V2 = 139.40 pts (79.5% de l'optimal théorique)

## Analyse des 10 Références

### 🔴 NON APPLICABLES (Information Cachée / Rôles Secrets)

#### 1. Re-determinizing ISMCTS in Hanabi (Goodman, 2019)
**Raison de rejet** : Hanabi a des cartes cachées et information imparfaite. Take It Easy n'a PAS d'information cachée.

#### 3. Learning in Games with Progressive Hiding (Heymann et al., 2024)
**Raison de rejet** : Conçu pour jeux avec information cachée progressive. Non applicable à Take It Easy.

#### 5. ReBeL - Imperfect-Information Games (Brown et al., 2020)
**Raison de rejet** : Pour jeux à information imparfaite (poker, etc.). Take It Easy a information parfaite.

#### 6. Hidden-Role Stochastic Games (Han et al., 2023)
**Raison de rejet** : Jeux avec rôles cachés. Take It Easy n'a pas de rôles ni d'adversaires.

#### 7. ISMCTS in Secret Hitler (Reinhardt, 2020)
**Raison de rejet** : Jeu de rôle caché. Non applicable.

---

### 🟡 PARTIELLEMENT APPLICABLES (Contexte différent mais techniques utiles)

#### 9. Evolutionary Algorithm for Hearthstone (García-Sánchez et al., 2024)
**Applicable** : ⚠️ Limité
**Raison** : Hearthstone est multi-joueurs avec adversaires. Approche évolutionnaire pourrait optimiser les hyperparamètres MCTS mais ne change pas fondamentalement l'algorithme.

**Potentiel** : Utiliser algorithme évolutionnaire pour tuner :
- Paramètres UCB (exploration/exploitation)
- Poids des heuristiques Pattern Rollouts
- Profondeur de simulation

**Gain estimé** : +1-2 pts → 140-141 pts
**Effort** : 1 semaine

---

### 🟢 TRÈS APPLICABLES (Stochasticité + Information Parfaite)

#### 2. Monte Carlo Tree Search: A Review (Świechowski et al., 2021) ⭐⭐⭐
**Applicable** : ✅ Oui
**Pourquoi** : Revue complète des variantes MCTS pour jeux stochastiques.

**Techniques recommandées du papier** :
1. **MCTS avec déterminisation multiple** : Faire plusieurs arbres MCTS avec différents tirages possibles
2. **Progressive Widening** : Limiter le nombre de branches enfants dans l'arbre (utile pour l'aléa)
3. **RAVE amélioré** : All-Moves-As-First (amélioration de RAVE que nous avons testé)

**Gain estimé** : +2-4 pts → 141-143 pts
**Effort** : 2 semaines

---

#### 4. Learning to Play Stochastic Perfect-Information Games (Cohen-Solal et al., 2023) ⭐⭐⭐⭐⭐
**Applicable** : ✅✅✅ PARFAIT MATCH
**Pourquoi** : **EXACTEMENT le contexte de Take It Easy** : jeu stochastique à information parfaite.

**Techniques du papier applicables** :
1. **Expectimax MCTS** : Au lieu de UCB classique, utiliser l'espérance sur les tirages aléatoires possibles
2. **Chance Nodes** : Noeuds représentant l'aléa (tirage tuile) séparés des noeuds de décision
3. **Variance Reduction** : Techniques pour réduire la variance des estimations dues à l'aléa

**Architecture proposée** :
```
État plateau
    ↓
Chance Node (tirage tuile)
    ↓ (27 possibilités)
Decision Node (19 positions)
    ↓ (répéter)
Évaluation finale
```

**Gain estimé** : +4-7 pts → 143-146 pts ⭐ MEILLEURE PISTE
**Effort** : 3 semaines

---

#### 8. Q-Learning for Stochastic Control (2024)
**Applicable** : ✅ Oui
**Pourquoi** : Base théorique solide pour RL dans contexte stochastique.

**Apport** : Garanties de convergence pour Q-Learning dans jeux stochastiques. Pourrait remplacer MCTS par Deep Q-Learning avec :
- État = plateau + tuiles disponibles
- Action = (tuile, position)
- Reward = score final

**Gain estimé** : +3-6 pts → 142-145 pts
**Effort** : 4 semaines (complexe, nécessite grosse infrastructure d'entraînement)

---

#### 10. MCTS + Supervised Learning for Hearthstone (Świechowski et al., 2018) ⭐⭐⭐⭐
**Applicable** : ✅✅ Oui
**Pourquoi** : Jeu de cartes stochastique, approche hybride MCTS + réseau.

**Techniques applicables** :
1. **Policy Network pour guider MCTS** : Réseau prédit la "meilleure position" pour chaque tuile → réduit espace de recherche
2. **Value Network pour estimation** : Remplace rollouts aléatoires par évaluation directe du plateau
3. **Training hybride** : MCTS génère données → entraîne réseau → réseau guide MCTS (boucle)

**Différence avec notre Gold GNN** : Eux utilisent le réseau PENDANT la recherche MCTS (pas après). Le réseau réduit l'espace de recherche au lieu de remplacer MCTS.

**Gain estimé** : +3-5 pts → 142-144 pts
**Effort** : 2-3 semaines

---

## Recommandations Priorisées

### 🥇 Option 1 : Expectimax MCTS (Papier #4) - RECOMMANDÉ

**Pourquoi** :
- Contexte EXACT de Take It Easy (stochastique + information parfaite)
- Fondamentalement meilleur que MCTS classique pour ce type de jeu
- MCTS classique ne modélise pas correctement l'aléa du tirage

**Plan d'implémentation** :
1. Ajouter **Chance Nodes** dans l'arbre MCTS
2. Remplacer UCB par **Expectimax** (moyenne pondérée sur tirages possibles)
3. Implémenter **variance reduction** pour stabiliser estimations

**Fichiers à modifier** :
- `src/mcts/algorithm.rs` : Ajouter type de noeud `ChanceNode`
- `src/mcts/selection.rs` : Remplacer UCB par Expectimax
- `src/mcts/expansion.rs` : Gérer expansion chance nodes

**Timeline** :
- Semaine 1 : Implémentation Chance Nodes
- Semaine 2 : Expectimax selection
- Semaine 3 : Variance reduction + tuning

**Gain attendu** : +4-7 pts → **143-146 pts** ✅ Atteindrait l'objectif 145 pts !

---

### 🥈 Option 2 : MCTS-Guided Neural Network (Papier #10)

**Pourquoi** :
- Combine forces de MCTS (recherche exhaustive) et NN (reconnaissance patterns)
- Retour d'expérience positif sur Hearthstone (jeu similaire)
- Peut réutiliser infrastructure CNN existante

**Approche** :
1. **Policy Network** : Prédit P(position | tuile, plateau) → top-3 positions
2. **MCTS explore seulement top-3** au lieu de 19 positions → 6× plus rapide
3. **Value Network** : Remplace pattern rollouts par évaluation directe

**Différence clé avec Gold GNN échoué** :
- Gold GNN : Réseau REMPLACE MCTS (échec)
- Cette approche : Réseau GUIDE MCTS (succès attendu)

**Gain attendu** : +3-5 pts → 142-144 pts
**Effort** : 2-3 semaines

---

### 🥉 Option 3 : Evolutionary Hyperparameter Tuning (Papier #9)

**Pourquoi** : Plus simple, quick win possible.

**Hyperparamètres à optimiser** :
1. UCB exploration constant (actuellement empirique)
2. Poids des heuristiques Pattern Rollouts V2
3. Nombre de simulations par coup
4. Profondeur de rollout

**Algorithme** : CMA-ES (Covariance Matrix Adaptation Evolution Strategy)

**Gain attendu** : +1-2 pts → 140-141 pts
**Effort** : 1 semaine

---

## Comparaison avec Approches Précédentes

| Approche | Score | Gain vs Baseline | Statut | Raison |
|----------|-------|------------------|--------|---------|
| Pattern Rollouts V2 (baseline) | 139.40 | - | ✅ PRODUCTION | - |
| Gold GNN (testé) | 127.74 | -11.66 | ❌ ÉCHEC | Réseau remplace MCTS au lieu de guider |
| Curriculum Learning (testé) | N/A | N/A | ❌ ANNULÉ | Beam search pire que MCTS |
| **Expectimax MCTS (Papier #4)** | **143-146** | **+4-7** | 🎯 RECOMMANDÉ | Fondamentalement adapté à l'aléa |
| MCTS-Guided NN (Papier #10) | 142-144 | +3-5 | ⭐ ALTERNATIF | Réseau guide MCTS, pas remplace |
| Evolutionary Tuning (Papier #9) | 140-141 | +1-2 | 💡 QUICK WIN | Simple à implémenter |

---

## Décision Recommandée

### ✅ Implémenter Expectimax MCTS (Papier #4)

**Justification** :
1. **Adapté au jeu** : Stochastique + information parfaite = contexte exact
2. **Gain maximal** : +4-7 pts → atteindrait objectif 145 pts
3. **Fondamentalement meilleur** : MCTS classique n'est pas optimal pour jeux avec aléa
4. **Effort raisonnable** : 3 semaines, réutilise code MCTS existant

**Prochaines étapes** :
1. Lire papier complet Cohen-Solal et al. (2023)
2. Créer `docs/expectimax_mcts_implementation_plan.md`
3. Implémenter Chance Nodes
4. Benchmarker avec 50 games × 150 sims
5. Si gain ≥ +3 pts → continuer avec variance reduction

---

## Références Bibliographiques

1. ❌ Goodman (2019) - Re-determinizing ISMCTS (Hanabi) - Information cachée
2. ⭐⭐⭐ Świechowski et al. (2021) - MCTS Review - Techniques générales
3. ❌ Heymann et al. (2024) - Progressive Hiding - Information cachée
4. ⭐⭐⭐⭐⭐ **Cohen-Solal et al. (2023) - Stochastic Perfect-Information Games - MATCH PARFAIT**
5. ❌ Brown et al. (2020) - ReBeL - Information imparfaite
6. ❌ Han et al. (2023) - Hidden-Role Games - Rôles cachés
7. ❌ Reinhardt (2020) - ISMCTS Secret Hitler - Rôles cachés
8. ⭐⭐⭐ Q-Learning Stochastic (2024) - Base théorique RL
9. ⭐⭐ García-Sánchez et al. (2024) - Evolutionary Hearthstone - Tuning hyperparamètres
10. ⭐⭐⭐⭐ **Świechowski et al. (2018) - MCTS + Supervised Hearthstone - Réseau guide MCTS**

**Top 3 à implémenter** :
1. 🥇 Papier #4 : Expectimax MCTS
2. 🥈 Papier #10 : MCTS-Guided Neural Network
3. 🥉 Papier #9 : Evolutionary Hyperparameter Tuning
