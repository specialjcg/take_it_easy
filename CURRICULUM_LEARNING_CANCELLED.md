# Curriculum Learning - Projet Annulé

## Découverte Critique

Après investigation approfondie, le projet Curriculum Learning est **ANNULÉ** car les "données expertes" générées par beam search sont **PIRES** que le baseline MCTS actuel.

## Problème des Tuiles

L'`optimal_solver.rs` qui montrait des scores de 175 pts utilisait des **tuiles factices** :

```rust
// ❌ TUILES FACTICES dans optimal_solver.rs
let mut all_tiles: Vec<Tile> = vec![
    Tile(1, 2, 3), Tile(1, 6, 8), Tile(1, 7, 3), ...
];
```

Alors que les **VRAIES tuiles du jeu** Take It Easy sont :

```rust
// ✅ VRAIES TUILES du jeu
const ALL_TILES: [Tile; 27] = [
    Tile(1, 5, 9), Tile(2, 6, 7), Tile(3, 4, 8),
    Tile(1, 6, 8), Tile(2, 4, 9), Tile(3, 5, 7),
    Tile(1, 4, 7), Tile(2, 5, 8), Tile(3, 6, 9),
    // × 3 exemplaires
];
```

## Résultats du Beam Search avec VRAIES Tuiles

Tests effectués avec `optimal_data_generator.rs` (code CORRECT) :

| Beam Width | Score Moyen | vs Baseline (139 pts) |
|------------|-------------|-----------------------|
| 100        | 110 pts     | **-29 pts** ❌        |
| 1000       | 114 pts     | **-25 pts** ❌        |

**Conclusion** : Même avec Beam Width 1000, le beam search donne **114 pts**, soit **25 pts de MOINS** que Pattern Rollouts V2 (139 pts).

## Pourquoi les Données "Expertes" sont Mauvaises

1. **Tuiles du jeu difficiles** : Les vraies tuiles ont des valeurs qui rendent l'optimisation difficile
2. **Heuristiques inadaptées** : Les heuristiques du beam search ne capturent pas bien les synergies des tuiles
3. **Espace de recherche restreint** : Même avec Beam 1000, on n'explore qu'une fraction des possibilités

## Impact sur le Projet

### Fichiers Créés (à Conserver pour Référence)

- `src/bin/optimal_data_generator.rs` ✅ Code correct, mais inutilisable
- `src/bin/expert_data_generator.rs` ❌ Code buggué (historique)
- `curriculum_learning.sh` ⏸️ Script non utilisé
- `docs/curriculum_learning_implementation_plan.md` 📚 Documentation
- `BEAM_SEARCH_BUG_ANALYSIS.md` 📚 Analyse du premier bug
- `expert_data/phase1_beam100.json` ❌ Données invalides (92 pts)

### Fichiers à Nettoyer

```bash
rm src/bin/expert_data_generator.rs  # Buggué
rm -rf expert_data/                   # Données invalides
```

## Leçons Apprises

1. **Toujours vérifier les tuiles** : L'optimal_solver utilisait des tuiles factices
2. **Benchmarker avant d'investir** : Tester beam search AVANT de coder tout le curriculum
3. **MCTS > Beam Search** : Pour ce jeu, MCTS Pattern Rollouts (139 pts) bat beam search (114 pts)

## Alternatives Recommandées

### Option 1 : Rester sur Pattern Rollouts V2 (139 pts) ✅ ACTUEL

**Status quo** - La solution actuelle est déjà performante.

### Option 2 : Hybrid Training (MCTS + Règles Expertes)

Au lieu d'utiliser beam search, mixer MCTS (70%) avec **règles heuristiques hand-crafted** (30%) :
- Compléter les lignes proches de la fin
- Prioritiser les grandes valeurs (9, 8, 7)
- Éviter les conflits

**Gain estimé** : +2-5 pts → 141-144 pts
**Durée** : 3-4 jours

### Option 3 : Augmenter MCTS Simulations

Passer de 150 à 300 simulations/coup :
- Plus de temps de calcul
- Meilleure exploration

**Gain estimé** : +1-3 pts → 140-142 pts
**Durée** : 1 jour (juste un paramètre)

### Option 4 : Améliorer les Heuristiques Pattern Rollouts

Affiner les pattern rollouts V2 avec des patterns plus sophistiqués :
- Patterns de fin de partie
- Patterns anti-conflits
- Patterns de synergie

**Gain estimé** : +3-6 pts → 142-145 pts
**Durée** : 1 semaine

## Décision Finale

**ABANDONNER Curriculum Learning** et se concentrer sur :
1. Documenter Pattern Rollouts V2 (139 pts) comme solution de production
2. Éventuellement explorer Option 4 (améliorer heuristiques) si temps disponible

## Statistiques Finales

| Approche | Score | Gain vs Baseline | Statut |
|----------|-------|------------------|--------|
| Pattern Rollouts V2 (baseline) | 139.40 pts | - | ✅ PRODUCTION |
| Gold GNN | 127.74 pts | -11.66 pts | ❌ ÉCHEC |
| Beam Search (Beam 1000) | 114 pts | -25 pts | ❌ PIRE |
| Curriculum Learning | N/A | N/A | 🚫 ANNULÉ |

**Pattern Rollouts V2 reste le champion incontesté.**
