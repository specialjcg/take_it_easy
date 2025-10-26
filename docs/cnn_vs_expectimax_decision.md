# CNN vs Expectimax MCTS - Analyse et Décision

## Situation Actuelle

**Pattern Rollouts V2 (139.40 pts)** utilise déjà un **CNN (Convolutional Neural Network)** :
- Architecture : CNN avec residual blocks
- Rôle : Guide MCTS en prédisant la valeur des positions
- Performance : 139.40 pts (79.5% de l'optimal)

## Question : Que faire avec le CNN ?

### Option A : Améliorer le CNN (Architecture Deep Learning)

**Approches possibles** :

#### A1. CNN plus profond (plus de layers)
- Ajouter plus de residual blocks
- Gain estimé : +1-2 pts
- Effort : 1 semaine

#### A2. CNN avec Attention Mechanism
- Ajouter Squeeze-and-Excitation blocks
- Ajouter Self-Attention layers
- Gain estimé : +2-3 pts
- Effort : 2 semaines

#### A3. MCTS-Guided CNN (Papier #10 Hearthstone)
- **Policy Network** : Prédit top-3 positions pour chaque tuile
- **Value Network** : Évalue la valeur du plateau
- MCTS explore seulement top-3 positions (6× plus rapide)
- Gain estimé : +3-5 pts → 142-144 pts
- Effort : 2-3 semaines

**Avantage** : Réutilise infrastructure CNN existante

**Inconvénient** : N'adresse pas le problème fondamental de modélisation de l'aléa

---

### Option B : Expectimax MCTS (Algorithme de Recherche)

**Concept** : Modifier MCTS pour modéliser explicitement l'aléa du tirage de tuiles

**Différence fondamentale** :
```
MCTS actuel (avec CNN) :
État → Sélection UCB → Simulation → Backpropagation
         ↑
    Guidé par CNN

Expectimax MCTS :
État → Chance Node (tirage tuile) → Expectimax → Decision Node (position) → Backpropagation
                                                        ↑
                                                   Peut AUSSI utiliser CNN !
```

**Point clé** : Expectimax MCTS peut **AUSSI utiliser le CNN** pour guider les décisions !

**Gain estimé** : +4-7 pts → 143-146 pts
**Effort** : 3 semaines

**Avantage** : Fondamentalement meilleur pour jeux stochastiques
**Inconvénient** : Plus complexe à implémenter

---

## Pourquoi MCTS Actuel N'est Pas Optimal

Le MCTS actuel (même avec CNN) a un **problème de modélisation** :

### Exemple Concret

**Situation** : Il reste 5 tuiles à jouer, 3 positions libres.

**MCTS actuel** :
1. Simule un tirage aléatoire de tuile
2. Explore les 3 positions possibles avec UCB
3. Backpropage le résultat

**Problème** : Chaque simulation tire UNE SEULE tuile aléatoire. Si on a de la malchance dans les tirages, l'estimation est biaisée.

**Expectimax MCTS** :
1. Crée un **Chance Node** représentant TOUS les tirages possibles
2. Calcule **l'espérance** sur tous les tirages (pondéré par probabilité)
3. Pour chaque tirage, explore les positions
4. Donne une estimation **non biaisée**

### Illustration

```
Situation : 19 tuiles tirées parmi 27 → il reste 8 tuiles possibles

MCTS actuel :
Simulation 1 : Tire Tile(1,5,9) → explore positions → score 145
Simulation 2 : Tire Tile(3,6,9) → explore positions → score 138
Simulation 3 : Tire Tile(1,5,9) → explore positions → score 145
Moyenne : 142.7 pts (mais biaisé vers Tile(1,5,9) qui a été tiré 2 fois)

Expectimax MCTS :
Calcule directement l'espérance sur LES 8 TUILES :
E[score] = (1/8 × score_tile1) + (1/8 × score_tile2) + ... + (1/8 × score_tile8)
         = Estimation non biaisée
```

---

## Recommandation : Les Deux !

### 🎯 Plan Optimal : Expectimax MCTS + CNN

**Phase 1 : Implémenter Expectimax MCTS (3 semaines)**
1. Modifier `src/mcts/algorithm.rs` pour ajouter Chance Nodes
2. Implémenter Expectimax selection au lieu de UCB
3. **Garder le CNN existant** pour guider les Decision Nodes

**Résultat attendu** : 143-146 pts (gain +4-7 pts grâce à meilleure modélisation de l'aléa)

**Phase 2 : Améliorer le CNN (2 semaines) - OPTIONNEL**
1. Si Phase 1 donne 143-145 pts → améliorer CNN avec Policy Network
2. Policy Network prédit top-3 positions
3. Expectimax MCTS explore seulement top-3

**Résultat attendu** : 145-148 pts (gain supplémentaire +2-3 pts grâce à meilleure sélection)

---

## Comparaison avec Approches Testées

| Approche | Architecture | Algorithme | Score | Statut |
|----------|--------------|------------|-------|--------|
| Pattern Rollouts V2 (baseline) | CNN | MCTS + Heuristiques | 139.40 | ✅ PRODUCTION |
| Gold GNN | GNN | MCTS + Heuristiques | 127.74 | ❌ ÉCHEC |
| **Expectimax MCTS + CNN** | CNN (existant) | **Expectimax MCTS** | **143-146** | 🎯 RECOMMANDÉ |
| MCTS-Guided CNN | CNN (amélioré) | MCTS + Heuristiques | 142-144 | ⭐ ALTERNATIF |
| Expectimax + Policy CNN | CNN (amélioré) | **Expectimax MCTS** | **145-148** | 🚀 OPTIMAL |

---

## Pourquoi Pas Juste Améliorer le CNN ?

**Réponse** : Le CNN actuel fait **déjà bien son travail** (139.40 pts).

**Problème** : Le goulot d'étranglement n'est PAS le CNN, c'est **l'algorithme MCTS** qui ne modélise pas correctement l'aléa.

### Preuve

Si le problème était le CNN :
- Gold GNN (architecture plus sophistiquée) aurait dû donner de meilleurs résultats
- Résultat : 127.74 pts (PIRE que CNN 139.40 pts)

**Conclusion** : L'architecture réseau n'est pas le problème. C'est l'algorithme de recherche qui doit être amélioré.

---

## Décision Finale

### ✅ Implémenter Expectimax MCTS en GARDANT le CNN

**Justification** :
1. **Fondamentalement meilleur** : Expectimax modélise correctement l'aléa
2. **Réutilise le CNN** : Pas besoin de réentraîner ou modifier le réseau
3. **Gain maximal** : +4-7 pts → 143-146 pts (atteindrait objectif 145 pts)
4. **Synergie** : Expectimax + CNN = combinaison optimale

**Architecture finale** :
```
Expectimax MCTS :
  ├── Chance Nodes : Modélise tirage aléatoire tuiles
  ├── Decision Nodes : Choix de position
  │     ↓
  │   Guidé par CNN (value estimation)
  └── Expectimax : Calcule espérance sur tirages
```

**Prochaines étapes** :
1. Créer `docs/expectimax_mcts_implementation_plan.md`
2. Implémenter Chance Nodes dans `src/mcts/algorithm.rs`
3. Modifier sélection UCB → Expectimax
4. Benchmarker avec CNN existant

**Timeline** : 3 semaines pour Expectimax MCTS
**Gain attendu** : +4-7 pts → 143-146 pts ✅ Objectif 145 pts atteint !
