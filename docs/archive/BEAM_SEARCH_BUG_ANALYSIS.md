# Beam Search Bug Analysis

## Problème Découvert

Le `expert_data_generator.rs` génère des scores très faibles (92 pts en moyenne) comparé à `optimal_solver.rs` (175 pts).

### Tests Effectués

1. **Beam 100** :
   - optimal_solver : N/A (non testé)
   - expert_data_generator : 92.94 pts

2. **Beam 1000** :
   - optimal_solver : 174.8 pts ✅
   - expert_data_generator : 114 pts ❌

### Cause Identifiée

Le `PartialSolution` dans `expert_data_generator.rs` contient un champ `history: Vec<(Tile, usize)>` qui est **cloné à chaque itération** du beam search :

```rust
#[derive(Clone)]
struct PartialSolution {
    plateau: Plateau,
    remaining_tiles: Vec<Tile>,
    score_estimate: f64,
    history: Vec<(Tile, usize)>,  // ⚠️ PROBLÈME ICI
}
```

À l'étape N, chaque solution dans le beam a un vecteur `history` de taille N. Quand on fait :
```rust
let mut new_solution = solution.clone();  // Clone history de taille N
new_solution.history.push((tile, position));  // +1
```

**Impact sur performance** :
- Beam width = 1000
- Étape 10 : clone 1000 × 10 éléments = 10,000 allocations
- Étape 19 : clone 1000 × 19 éléments = 19,000 allocations
- **Total : ~190,000 allocations de vecteurs par partie !**

Cela ralentit massivement le beam search et probablement cause des timeouts internes qui font que l'algorithme n'explore pas correctement l'espace de recherche.

### Preuve

`optimal_solver.rs` **n'a PAS de champ `history`** et fonctionne parfaitement avec 174.8 pts.

```bash
$ grep -n "history" src/bin/optimal_solver.rs
# (aucun résultat)
```

## Solutions Proposées

### Solution 1 : Supprimer history et reconstruire après (COMPLEXE)

Retirer `history` de `PartialSolution` et reconstruire l'historique APRÈS avoir trouvé la meilleure solution en rejouant les coups.

**Problème** : Difficile de retrouver l'ordre exact des coups à partir du plateau final.

### Solution 2 : Modifier optimal_solver.rs pour retourner l'historique (RECOMMANDÉ)

Créer un nouveau binaire `optimal_solver_with_history.rs` basé sur `optimal_solver.rs` mais qui :
1. Stocke l'historique UNIQUEMENT pour la meilleure solution (pas pendant le beam search)
2. Sauvegarde au format JSON pour training

**Avantages** :
- Réutilise le code éprouvé d'optimal_solver.rs
- Pas de perte de performance pendant beam search
- Historique garanti correct

### Solution 3 : Utiliser un index au lieu de clone (TECHNIQUE)

Au lieu de cloner l'historique, utiliser un index parent-enfant :

```rust
struct PartialSolution {
    plateau: Plateau,
    remaining_tiles: Vec<Tile>,
    score_estimate: f64,
    parent_id: Option<usize>,
    move_played: Option<(Tile, usize)>,
}
```

Puis reconstruire l'historique en remontant les parents.

**Avantages** :
- Pas de clones de vecteurs
- Performance similaire à optimal_solver.rs

**Inconvénients** :
- Plus complexe à implémenter
- Nécessite de garder TOUTES les solutions dans un Vec global

## Recommandation Immédiate

**ABANDONNER `expert_data_generator.rs` actuel et utiliser Solution 2.**

Créer `src/bin/optimal_data_generator.rs` basé sur `optimal_solver.rs` avec modifications minimales pour sauvegarder l'historique au format JSON.

## Impact sur Curriculum Learning

Ce bug invalide **Phase 1 data** générée (`expert_data/phase1_beam100.json`) :
- Scores : 92.94 pts (inutilisable)
- Fichier à supprimer : `expert_data/phase1_beam100.json`

**Action requise** :
1. Implémenter Solution 2 (optimal_data_generator.rs)
2. Régénérer Phase 1 avec beam search correct
3. Vérifier scores ≥ 150 pts avant de continuer

## Timeline Mise à Jour

| Tâche | Durée Estimée | Statut |
|-------|---------------|--------|
| Créer optimal_data_generator.rs | 2h | ⏳ À FAIRE |
| Régénérer Phase 1 (Beam 100, 50 parties) | 30min | ⏳ À FAIRE |
| Vérifier scores Phase 1 ≥ 150 pts | 5min | ⏳ À FAIRE |

**Total délai supplémentaire : +2.5h**
