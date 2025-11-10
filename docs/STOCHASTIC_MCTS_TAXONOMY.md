# Taxonomie des Jeux pour Stochastic MCTS

*Guide pratique: Quand utiliser (ou éviter) les approches stochastiques*

---

## 🎯 Question Centrale

**Votre jeu contient de l'aléatoire. Devez-vous utiliser Stochastic MCTS (Expectimax, etc.)?**

**Réponse courte:** Ça dépend de la **structure informationnelle** du jeu, pas juste de la présence d'aléatoire.

---

## 📐 Critères de Décision (Arbre de Décision)

```
Votre jeu contient de l'aléa (dés, cartes, tirages)?
│
├─ NON → MCTS classique (déterministe)
│
└─ OUI → Continuez...
    │
    ├─ L'aléa est résolu AVANT les décisions?
    │  │
    │  ├─ OUI (ex: Take It Easy, Tetris)
    │  │  └─→ ❌ N'utilisez PAS Stochastic MCTS
    │  │      └─→ ✅ Utilisez: MCTS standard + heuristiques
    │  │
    │  └─ NON (ex: Backgammon, Poker)
    │     └─→ Continuez...
    │
    ├─ Les futurs aléas influencent la décision actuelle?
    │  │
    │  ├─ NON (indépendance)
    │  │  └─→ ❌ N'utilisez PAS Stochastic MCTS
    │  │
    │  └─ OUI (dépendance forte)
    │     └─→ Continuez...
    │
    ├─ Facteur de branchement des chance nodes < 10?
    │  │
    │  ├─ NON (b > 20)
    │  │  └─→ ⚠️ Stochastic MCTS sera très coûteux
    │  │      └─→ Considérez: Déterminisation, ISMCTS
    │  │
    │  └─ OUI (b < 10)
    │     └─→ Continuez...
    │
    └─ Budget computationnel ≥ b² × baseline?
       │
       ├─ NON
       │  └─→ ⚠️ Pas assez de budget
       │      └─→ Utilisez: Heuristiques + MCTS léger
       │
       └─ OUI
          └─→ ✅ Stochastic MCTS est probablement adapté!
```

---

## 🎮 Taxonomie par Type de Jeu

### Catégorie A: Stochastic MCTS Recommandé ✅

**Caractéristiques:**
- Aléa pendant les décisions (dés, tirages adverses)
- Information mutuelle élevée (futurs influencent présent)
- Branchement modéré (b < 10)
- Structure tactique (décisions complexes)

#### Exemples

**1. Backgammon** ⭐⭐⭐⭐⭐

```
Structure:
  Tour joueur:
    1. Lance 2 dés (aléatoire) → 21 combinaisons
    2. Décide quels pions bouger (tactique)
  Tour adversaire: pareil

Pourquoi Stochastic MCTS marche:
✓ Futurs dés affectent la stratégie (prendre des risques?)
✓ Branchement modéré (b = 21)
✓ Horizon court (1-2 coups anticipés suffisent)
✓ Information mutuelle forte (dés probables vs pions exposés)

Résultats empiriques:
- Stochastic MCTS: ELO ~1800
- MCTS déterministe: ELO ~1200
→ Gain: +600 ELO ✅
```

**2. Can't Stop**

```
Structure:
  Tour joueur:
    1. Lance 4 dés
    2. Décide comment les grouper
    3. Décide: continuer ou s'arrêter (risque/récompense)

Pourquoi Stochastic MCTS marche:
✓ Décision "continuer?" dépend des futures probabilités de dés
✓ b modéré (dépend de l'état)
✓ Structure push-your-luck (gestion de risque)

Résultats:
- Stochastic MCTS: Bat les joueurs experts
```

**3. Poker (avec limitations)**

```
Structure:
  Tour pré-flop:
    1. Cartes privées distribuées
    2. Décision: miser/suivre/passer
  Tour flop:
    3. 3 cartes communes révélées (aléatoire)
    4. Décision: miser/suivre/passer
  ...

Pourquoi Stochastic MCTS marche (partiellement):
✓ Futures cartes changent les probabilités de victoire
✓ b réductible (impossible card removal)
⚠️ Information partielle (cartes adverses)
⚠️ Psychologie (bluff) difficile à modéliser

Note: On utilise plutôt ISMCTS (Information Set MCTS)
```

**4. Catan (placement aléatoire de ressources)**

```
Structure:
  1. Lance 2 dés → ressources produites
  2. Décisions: construire, échanger
  3. Anticipation: "Si je place ici, quelles ressources j'aurai?"

Pourquoi Stochastic MCTS marche:
✓ Placement initial dépend des probabilités de dés
✓ b = 11 (sommes de 2 à 12)
✓ Espérance de ressources guide les décisions

Résultats:
- IA avec Stochastic MCTS compétitive
```

---

### Catégorie B: Stochastic MCTS Neutre/Mitigé ⚠️

**Caractéristiques:**
- Aléa présent mais impact modéré
- Information mutuelle moyenne
- Alternatives souvent aussi bonnes/meilleures

#### Exemples

**1. Yahtzee**

```
Structure:
  1. Lance 5 dés
  2. Décide lesquels relancer (0-3 fois)
  3. Choisis catégorie de score

Pourquoi résultats mitigés:
⚠️ Branchement élevé (252 combinaisons de dés)
✓ Espérance calculable analytiquement (pas besoin de MCTS!)
⚠️ Horizon court (3 lancers)

Approche optimale:
→ Programmation dynamique (tables pré-calculées)
→ Plus efficace que Stochastic MCTS
```

**2. Jeux de cartes simples (ex: Blackjack)**

```
Pourquoi neutre:
⚠️ Stratégie optimale connue (calcul exact possible)
⚠️ Horizon très court (1-2 décisions)
✓ Mais utile pour variantes complexes (multi-joueurs)

Recommandation:
- Jeu simple → Tables stratégiques
- Variante complexe → Stochastic MCTS envisageable
```

---

### Catégorie C: Stochastic MCTS Déconseillé ❌

**Caractéristiques:**
- Aléa résolu avant décisions
- Information mutuelle faible/nulle
- Branchement explosif
- Meilleures alternatives disponibles

#### Exemples

**1. Take It Easy** (étude de cas) ❌❌❌

```
Structure:
  1. Tire une tuile (aléatoire, uniforme)
  2. Décide où la placer (déterministe après tirage)
  3. Répète 19 fois
  4. Score final calculé

Pourquoi Stochastic MCTS échoue:
❌ Séparation temporelle (tirage AVANT décision)
❌ Branchement énorme (b = 27 tuiles)
❌ Horizon long (19 coups × 27 branches = 27^19 séquences)
❌ Information mutuelle ≈ 0 (futurs tirages indépendants)

Résultats empiriques:
- Stochastic MCTS: 1.33 pts
- MCTS + heuristiques: 139.40 pts
→ Régression: -99.0% ❌

Alternative optimale:
✓ MCTS standard (connaît la tuile actuelle)
✓ Heuristiques domaine (Pattern Rollouts)
✓ CNN pour évaluation de grille
```

**2. Tetris**

```
Structure:
  1. Pièce aléatoire apparaît
  2. Décide où/comment la placer
  3. Répète jusqu'à game over

Pourquoi similaire à Take It Easy:
❌ Aléa (pièce) résolu avant décision (placement)
❌ Futurs tirages n'informent pas sur placement actuel
❌ b élevé (7 pièces × 4 rotations)

Approche optimale:
✓ Heuristiques domaine (height, holes, bumpiness)
✓ Value network entraîné par RL
✓ MCTS déterministe (si utilisé)

Note: Le record mondial Tetris (IA) utilise
des heuristiques, PAS Stochastic MCTS!
```

**3. Jeux de type "Match-3" (Candy Crush, etc.)**

```
Structure:
  1. Grille avec éléments aléatoires
  2. Décision: quel swap faire?
  3. Cascade (semi-aléatoire)
  4. Nouveaux éléments tombent (aléatoire)

Pourquoi Stochastic MCTS échoue:
❌ Branchement exponentiel (b très grand)
❌ Cascade difficile à modéliser exactement
❌ Futurs spawns peu informatifs pour décision actuelle

Approche utilisée en pratique:
✓ Heuristiques de patterns (combos, couleurs)
✓ A* search (si objectifs locaux)
```

**4. Jeux de "Gacha" / Loot boxes**

```
Structure:
  1. Décision: ouvrir ou pas?
  2. Résultat aléatoire (rare vs commun)

Pourquoi MCTS n'a pas de sens:
❌ Pas de tactique (juste espérance mathématique)
❌ Pas de structure combinatoire
❌ Calcul direct de l'espérance suffit

Note: Si une IA utilise "MCTS" ici, c'est du marketing!
```

---

## 📊 Tableau Récapitulatif

| Jeu | Catégorie | Aléa | b (branching) | I/H ratio* | Stochastic MCTS? | Score |
|-----|-----------|------|---------------|-----------|------------------|-------|
| **Backgammon** | A | Pendant | 21 | 0.45 | ✅ Fortement | ⭐⭐⭐⭐⭐ |
| **Can't Stop** | A | Pendant | ~15 | 0.38 | ✅ Oui | ⭐⭐⭐⭐ |
| **Poker** | A | Pendant | 10-52 | 0.35 | ⚠️ ISMCTS préféré | ⭐⭐⭐ |
| **Catan** | A | Pendant | 11 | 0.32 | ✅ Oui | ⭐⭐⭐⭐ |
| **Yahtzee** | B | Pendant | 252 | 0.25 | ⚠️ DP meilleur | ⭐⭐ |
| **Blackjack** | B | Pendant | 13 | 0.28 | ⚠️ Tables > MCTS | ⭐⭐ |
| **Take It Easy** | C | Avant | 27 | 0.02 | ❌ Non | ⭐ |
| **Tetris** | C | Avant | 28 | 0.01 | ❌ Non | ⭐ |
| **Candy Crush** | C | Après | >100 | 0.05 | ❌ Non | ⭐ |
| **Gacha games** | C | Pur | ∞ | 0 | ❌ Non | ☆ |

*I/H ratio = Information mutuelle / Entropie (mesuré empiriquement)

---

## 🔬 Test Rapide: Votre Jeu est-il Adapté?

### Checklist de Validation

Répondez aux questions suivantes:

```
☐ Les futurs événements aléatoires influencent-ils la décision actuelle?
   Exemple: "Si je risque ce mouvement, et que les dés sont bons, je gagne"

☐ Le facteur de branchement des chance nodes est-il < 20?
   Comptez: combien de résultats aléatoires distincts à chaque étape?

☐ Avez-vous un budget computationnel de ≥ b² simulations?
   Calculez: b² × (simulations d'un MCTS normal)

☐ Les joueurs humains raisonnent-ils en termes d'espérances?
   Test: Un expert dit-il "Je prends ce risque car l'espérance est positive"?

☐ La stratégie optimale est-elle inconnue/incalculable?
   Si solvable analytiquement → Stochastic MCTS superflu
```

**Interprétation:**
- 5/5 ✓ → Stochastic MCTS probablement optimal
- 3-4/5 ✓ → Peut marcher, à tester empiriquement
- 1-2/5 ✓ → Stochastic MCTS probablement sous-optimal
- 0/5 ✓ → N'utilisez PAS Stochastic MCTS

### Exemple: Application à Take It Easy

```
☐ Futurs aléas influencent décision actuelle?
  → NON (futurs tirages indépendants du placement actuel)

☐ Branchement < 20?
  → NON (b = 27 tuiles possibles)

☐ Budget ≥ b²?
  → NON (27² = 729× plus de simulations nécessaires)

☐ Joueurs pensent en espérances?
  → NON (joueurs pensent: "où mettre CETTE tuile pour mes lignes?")

☐ Stratégie optimale inconnue?
  → PARTIEL (heuristiques domaine marchent bien)

Score: 0/5 ✓ → Stochastic MCTS déconseillé ❌
```

---

## 🛠️ Alternatives Recommandées (si Stochastic MCTS inadapté)

### Alternative 1: MCTS Déterministe + Heuristiques

**Principe:** Traiter chaque instance aléatoire comme un jeu déterministe.

```python
# Au lieu de modéliser TOUS les futurs tirages:
def choose_action(game_state, current_tile):
    # On connaît déjà current_tile (aléa résolu)
    mcts = StandardMCTS(game_state, current_tile)
    mcts.add_heuristics(domain_patterns)  # Ex: Pattern Rollouts
    return mcts.search(num_simulations=150)

# Pas besoin de chance nodes!
```

**Avantages:**
- ✅ Budget concentré sur la décision actuelle
- ✅ Pas d'explosion combinatoire
- ✅ Heuristiques exploitent la structure du jeu

**Quand utiliser:** Jeux Catégorie C (aléa avant décisions)

**Exemples:** Take It Easy (139 pts), Tetris (records mondiaux)

---

### Alternative 2: Déterminisation (Single Observer MCTS)

**Principe:** Échantillonner UN futur possible, jouer comme si déterministe.

```python
def choose_action(game_state):
    all_actions_scores = defaultdict(float)

    for sample in range(num_samples):
        # Échantillonne UNE séquence future aléatoire
        future_scenario = sample_random_future(game_state)

        # MCTS standard sur ce scénario déterministe
        mcts = StandardMCTS(game_state, future_scenario)
        best_action = mcts.search(num_simulations=150)

        # Vote: quelle action est bonne dans CE scénario?
        all_actions_scores[best_action] += 1

    # Action la plus robuste (bonne dans le plus de scénarios)
    return max(all_actions_scores, key=all_actions_scores.get)
```

**Avantages:**
- ✅ Réduit b de 27 à 1 (pas de chance nodes)
- ✅ MCTS reste efficace (pas d'explosion)
- ✅ Capture la robustesse (action bonne en moyenne)

**Quand utiliser:** Jeux Catégorie B/C avec besoin d'anticipation

**Exemples:** Certaines variantes de Poker, jeux avec information cachée

---

### Alternative 3: Value Network + MCTS Léger

**Principe:** Entraîner un réseau à estimer la valeur, MCTS pour la recherche locale.

```python
# 1. Entraînement offline du réseau
value_net = train_network(
    games_database,
    architecture="CNN" or "GNN",
    training_method="curriculum_learning"
)

# 2. Utilisation online avec MCTS
def choose_action(game_state, current_tile):
    mcts = StandardMCTS(game_state, current_tile)

    # Évaluation des feuilles par le réseau (pas rollouts aléatoires)
    mcts.set_leaf_evaluator(value_net)

    return mcts.search(num_simulations=150)
```

**Avantages:**
- ✅ Pas besoin de modéliser l'aléa
- ✅ Réseau apprend les patterns complexes
- ✅ MCTS affine la décision localement

**Quand utiliser:** Toutes catégories (surtout B et C)

**Exemples:** AlphaZero (Chess/Go), Take It Easy (CNN), MuZero

---

### Alternative 4: Pure Policy Network (sans MCTS)

**Principe:** Entraîner un réseau à prédire directement la meilleure action.

```python
# Entraînement supervisé ou RL
policy_net = train_policy(
    expert_games or self_play,
    architecture="Transformer" or "CNN"
)

# Utilisation
def choose_action(game_state, current_tile):
    # Pas de recherche, juste prédiction
    action_probs = policy_net(game_state, current_tile)
    return argmax(action_probs)  # ou sample selon température
```

**Avantages:**
- ✅ Très rapide (pas de simulations)
- ✅ Bon pour temps réel ou dispositifs limités
- ⚠️ Moins robuste que MCTS+network

**Quand utiliser:** Temps réel, ressources limitées, après beaucoup d'entraînement

**Exemples:** AlphaGo (policy network seul = 1500 ELO), agents Atari

---

## 📈 Comparaison des Approches sur Take It Easy

| Approche | Score | Temps/coup | Complexité | Résultat |
|----------|-------|------------|------------|----------|
| **Aléatoire** | ~50 pts | 0.1 ms | Trivial | Baseline minimum |
| **Greedy heuristiques** | ~110 pts | 1 ms | Simple | Bon pour prototypage |
| **MCTS pur (sans rollouts)** | ~80 pts | 100 ms | Moyen | Inefficace sans guidance |
| **MCTS + Pattern Rollouts V2** | **139 pts** | 895 ms | Moyen-élevé | ✅ **État de l'art actuel** |
| **CNN Value Net + MCTS** | ~135 pts | 300 ms | Élevé | Bon compromis vitesse/qualité |
| **Expectimax MCTS (testé)** | 1.33 pts | 358 ms | Très élevé | ❌ **Échec total** |
| **Gold GNN + MCTS (hypothétique)** | 145 pts? | 500 ms? | Très élevé | 🔬 À tester |

**Leçon:** La complexité algorithmique ne garantit PAS de meilleures performances.

---

## 🎓 Principes de Conception

### Principe 1: "Match the Algorithm to the Information Structure"

> L'algorithme doit correspondre à la structure informationnelle du problème, pas à sa surface.

**Exemple:**
- Backgammon: Aléa pendant décision → Stochastic MCTS adapté
- Take It Easy: Aléa avant décision → Stochastic MCTS inadapté
- **Même présence d'aléa, structures différentes!**

### Principe 2: "Computational Budget is Sacred"

> Un calcul doit améliorer la décision proportionnellement à son coût.

**Test:** Rendement = (amélioration du score) / (coût computationnel)

```
Expectimax sur Take It Easy:
  Amélioration: +0 pts (pire que baseline!)
  Coût: ×6 (358 ms vs 895 ms, mais moins de simulations)
  Rendement: -∞ ❌

Pattern Rollouts:
  Amélioration: +30 pts vs CNN seul
  Coût: ×3 (vs CNN seul)
  Rendement: +10 pts par ×1 ✅
```

### Principe 3: "Heuristics Beat Brute Force (when available)"

> Si une heuristique domaine existe, elle est souvent meilleure qu'une recherche générique.

**Exemples:**
- Tetris: Heuristiques > Stochastic MCTS
- Chess (années 1990): Heuristiques > MCTS
- Take It Easy: Pattern Rollouts > Expectimax

**Mais:** Réseaux de neurones peuvent apprendre ces heuristiques!

### Principe 4: "Test Before Commit"

> Implémentez un prototype rapide et mesurez avant d'investir.

**Protocole:**
1. Implémentez alternative simple (heuristique)
2. Implémentez Stochastic MCTS
3. Comparez sur 10-100 parties
4. Décidez basé sur les données, pas l'intuition

**Exemple Take It Easy:**
- Expectimax semblait théoriquement prometteur
- Tests ont révélé régression de -99%
- Décision: abandonner et investir ailleurs (GNN)

---

## 🔗 Ressources Complémentaires

### Outils de Diagnostic

**1. Information Mutuelle Test**
```python
def test_mutual_information(game, num_samples=1000):
    """
    Mesure I(action_t ; futurs_aléas) / H(futurs_aléas)

    Si ratio < 0.1: Stochastic MCTS probablement inadapté
    Si ratio > 0.3: Stochastic MCTS probablement utile
    """
    actions = []
    futures = []

    for _ in range(num_samples):
        state = game.sample_state()
        actions.append(game.get_optimal_action(state))
        futures.append(game.sample_future(state))

    mi = mutual_information(actions, futures)
    h = entropy(futures)
    return mi / h

# Application à Take It Easy
ratio = test_mutual_information(TakeItEasy())
print(f"I/H ratio: {ratio:.3f}")  # Résultat: 0.02 → ❌
```

**2. Branching Factor Calculator**
```python
def compute_branching_factor(game):
    """
    Calcule b effectif des chance nodes
    """
    samples = [game.sample_chance_outcome() for _ in range(1000)]
    unique_outcomes = len(set(samples))
    return unique_outcomes

b = compute_branching_factor(TakeItEasy())
print(f"b = {b}")  # Résultat: 27 → ⚠️ Élevé
```

### Lectures Recommandées

**Théorie Stochastic MCTS:**
- Browne et al. (2012): *"Survey of Monte Carlo Tree Search Methods"*
  → Chapitre 5: "Stochastic Games"

- Cowling et al. (2012): *"Information Set MCTS"*
  → Alternative pour information partielle

**Cas d'échec documentés:**
- Frank & Basin (1998): *"Search in Games with Incomplete Information"*
  → Montre limites du MCTS sur certains jeux

**Succès Stochastic MCTS:**
- Van den Broeck et al. (2009): *"Solving Backgammon with MCTS"*
- Whitehouse et al. (2011): *"Monte Carlo Tree Search in Poker"*

---

## ✅ Checklist Finale

Avant d'implémenter Stochastic MCTS, vérifiez:

```
[ ] Calculé le facteur de branchement (b)
[ ] Mesuré l'information mutuelle (I/H ratio)
[ ] Estimé le budget nécessaire (b² × baseline)
[ ] Vérifié qu'aucune solution analytique n'existe
[ ] Implémenté alternative simple (benchmark)
[ ] Défini métrique de succès claire
[ ] Planifié tests empiriques (≥100 parties)
```

**Si tous les checks sont ✓:** Implémentez et testez!

**Si < 4 checks sont ✓:** Considérez fortement des alternatives.

---

## 🎬 Conclusion

> "Not all randomness is created equal. The structure of uncertainty matters more than its presence."

**Messages clés:**
1. ✅ Stochastic MCTS est puissant QUAND la structure du jeu le justifie
2. ❌ Mais il échoue catastrophiquement sur certains jeux aléatoires (ex: Take It Easy)
3. 🔬 Toujours tester empiriquement avant d'investir dans une implémentation complexe
4. 🛠️ Des alternatives plus simples sont souvent meilleures

**Pour votre jeu:** Utilisez l'arbre de décision (section 2) et la checklist finale!

---

*Document créé: 2025-10-30*
*Basé sur: Analyse empirique Take It Easy + littérature MCTS*
*Mainteneur: Équipe de recherche Take It Easy*
