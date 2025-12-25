# Sprint 2 - Zero-Copy Refactor Plan

## Objectif
Éliminer 36,750+ clones dans algorithm.rs en utilisant PlateauCoW et DeckCoW

## Analyse du Bottleneck

### Code Actuel (lignes 416-456)
```rust
for &position in &subset_moves {                      // ~7 iterations
    let mut temp_plateau = plateau.clone();           // ❌ Clone #1
    let mut temp_deck = deck.clone();                 // ❌ Clone #2

    temp_plateau.tiles[position] = chosen_tile;
    temp_deck = replace_tile_in_deck(&temp_deck, &chosen_tile);

    for _ in 0..rollout_count {                       // ~7 iterations
        let lookahead_plateau = temp_plateau.clone(); // ❌ Clone #3
        let lookahead_deck = temp_deck.clone();       // ❌ Clone #4

        let tile2 = ...;
        let second_moves = get_legal_moves(&lookahead_plateau);

        for &pos2 in &second_moves {                  // ~15 iterations
            let mut plateau2 = lookahead_plateau.clone(); // ❌ Clone #5
            let mut deck2 = lookahead_deck.clone();       // ❌ Clone #6

            plateau2.tiles[pos2] = tile2;
            deck2 = replace_tile_in_deck(&deck2, &tile2);

            let score = simulate_games_smart(
                plateau2.clone(),  // ❌ Clone #7
                deck2.clone(),     // ❌ Clone #8
                None
            ) as f64;
        }
    }
}
```

**Calcul** : 7 × 7 × 15 × 8 clones × 150 simulations = **880,800 clone operations**

---

## Plan de Refactor

### Phase 1 : Modifier Signatures (SAFE)
Changer les fonctions pour accepter CoW au lieu de &mut:

```rust
// AVANT
fn mcts_core(
    plateau: &mut Plateau,
    deck: &mut Deck,
    ...
) -> MCTSResult

// APRÈS
fn mcts_core_cow(
    plateau: &PlateauCoW,
    deck: &DeckCoW,
    ...
) -> MCTSResult
```

### Phase 2 : Refactor Boucle Critique (COMPLEX)

#### Transformation Ligne 417-418
```rust
// AVANT
let mut temp_plateau = plateau.clone();       // Expensive Vec<Tile> clone
let mut temp_deck = deck.clone();

// APRÈS
let temp_plateau = plateau.clone();           // Cheap Rc clone (pointer copy)
let temp_deck = deck.clone();
```

#### Transformation Ligne 420-421 (Mutation)
```rust
// AVANT
temp_plateau.tiles[position] = chosen_tile;

// APRÈS
let temp_plateau_modified = temp_plateau.clone_for_modification();
temp_plateau_modified.set_tile(position, chosen_tile);
```

**MAIS** - Problème : on perd le bénéfice CoW si on clone_for_modification() immédiatement.

**Solution** : Utiliser write() directement car temp_plateau n'est utilisé que dans ce scope :
```rust
let temp_plateau = plateau.clone();  // Cheap (Rc increment)
temp_plateau.set_tile(position, chosen_tile);  // Direct mutation via RefCell
```

#### Transformation Ligne 431-432
```rust
// AVANT
let lookahead_plateau = temp_plateau.clone();  // Expensive
let lookahead_deck = temp_deck.clone();

// APRÈS
let lookahead_plateau = temp_plateau.clone();  // Cheap (Rc clone)
let lookahead_deck = temp_deck.clone();
```

#### Transformation Ligne 442 (Read-only)
```rust
// AVANT
let second_moves = get_legal_moves(&lookahead_plateau);

// APRÈS - Option 1 : Modifier get_legal_moves signature
let second_moves = lookahead_plateau.read(|p| get_legal_moves(p));

// APRÈS - Option 2 : Garder compatibilité, créer ref temporaire
// (Nécessite cloner la référence, moins optimal)
```

#### Transformation Ligne 454 (simulate_games_smart)
```rust
// AVANT
simulate_games_smart(plateau2.clone(), deck2.clone(), None)

// APRÈS - simulate_games_smart doit accepter CoW
simulate_games_smart_cow(&plateau2, &deck2, None)
```

---

### Phase 3 : Fonctions Auxiliaires

Fonctions à adapter pour CoW :
1. ✅ `get_legal_moves` : accepter `&Plateau` (pas besoin CoW, read-only)
2. ❌ `replace_tile_in_deck` : retourne nouveau Deck (à adapter)
3. ❌ `simulate_games_smart` : accepter CoW
4. ❌ `convert_plateau_to_tensor` : read-only, mais signature actuelle
5. ❌ `enhanced_position_evaluation` : read-only

**Stratégie** :
- Créer versions `_cow` des fonctions qui mutent
- Garder anciennes pour compatibilité temporaire
- Migrer progressivement

---

## Risques et Mitigations

| Risque | Impact | Mitigation |
|--------|--------|------------|
| RefCell borrow_mut() panic | HIGH | Tests exhaustifs, éviter emprunts simultanés |
| Performance régression si mal utilisé | MEDIUM | Profiler après implémentation |
| Complexité code augmentée | LOW | Documentation inline |
| Bugs subtils avec shared state | HIGH | Tests unitaires ++, assertions ref_count |

---

## Checklist Implémentation

### ✅ Phase 1 : Infrastructure (DONE)
- [x] PlateauCoW créé (fb499f1)
- [x] DeckCoW créé (fb499f1)
- [x] Tests unitaires CoW (8 tests)
- [x] 94/94 tests passing

### 🔄 Phase 2 : Core Algorithm
- [ ] Créer `mcts_core_cow()`
- [ ] Refactor boucle lignes 416-456
- [ ] Adapter `replace_tile_in_deck` pour CoW
- [ ] Tests de non-régression
- [ ] Profiler allocations (valider -97%)

### 📋 Phase 3 : Migration Callers
- [ ] `mcts_find_best_position_for_tile_with_nn` → use `mcts_core_cow`
- [ ] `mcts_find_best_position_for_tile_pure` → use `mcts_core_cow`
- [ ] `simulate_games_smart` → accepter CoW
- [ ] Benchmark final (espéré: +20-40 pts)

### 🧹 Phase 4 : Cleanup
- [ ] Supprimer ancien `mcts_core` si inutilisé
- [ ] Renommer `_cow` → nom standard
- [ ] Documentation
- [ ] Commit Sprint 2 complet

---

## Expected Impact

**Avant** :
- Allocations/call : ~36,750
- CPU time : baseline
- Score : ~86 pts

**Après** :
- Allocations/call : <1,000 (-97%)
- CPU time : -30% (from profiling)
- Score : **106-126 pts** (+20-40 pts attendu)

---

## Next Steps

1. Implémenter `mcts_core_cow()` avec boucle refactorée
2. Créer `replace_tile_in_deck_cow()` helper
3. Tests unitaires + benchmark comparatif
4. Itérer jusqu'à gains mesurables
