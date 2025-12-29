# Stratégie de Rollback - MCTS Performance Improvements

## 🛡️ Points de Sauvegarde

### Tag Baseline Créé
```bash
git tag -l | grep mcts
# Output: mcts-baseline-159pts
```

**État sauvegardé** :
- Score MCTS : 159.95 pts
- Toutes les optimisations hyperparamètres (+8.8%)
- Code stable : 207/207 tests passing
- Aucune modification structurelle

---

## 🌿 Stratégie de Branches

```
master (protected)
  │
  ├─ mcts-baseline-159pts [TAG] ← Point de retour sûr
  │
  └─ feat/mcts-performance-boost [BRANCH] ← Expérimentations
       │
       ├─ sprint-1-progressive-widening [COMMIT] ← Checkpoint 1
       ├─ sprint-2-zero-copy [COMMIT] ← Checkpoint 2
       └─ sprint-3-parallel [COMMIT] ← Checkpoint 3
```

---

## 🔄 Procédures de Rollback

### Rollback Complet (retour à baseline)

```bash
# Option 1 : Abandonner la branche feature
git checkout master
git branch -D feat/mcts-performance-boost

# Option 2 : Revenir au tag baseline
git checkout master
git reset --hard mcts-baseline-159pts

# Vérifier le retour
git log --oneline -1
# Output: 7459292 docs: add comprehensive MCTS performance improvement plan (Mikado method)

cargo test --release
# Devrait passer 207/207 tests
```

---

### Rollback Partiel (garder certains sprints)

```bash
# Exemple : Garder Sprint 1, annuler Sprint 2 et 3
git checkout feat/mcts-performance-boost

# Trouver le commit du Sprint 1
git log --oneline --grep="sprint-1"

# Reset à ce commit (remplacer <hash> par le SHA)
git reset --hard <hash-sprint-1>

# Force push si déjà poussé (ATTENTION)
git push origin feat/mcts-performance-boost --force
```

---

### Rollback d'un Fichier Spécifique

```bash
# Revenir version baseline d'un seul fichier
git checkout mcts-baseline-159pts -- src/mcts/algorithm.rs

# Ou depuis master
git checkout master -- src/mcts/algorithm.rs

# Commit la restauration
git commit -m "revert: restore algorithm.rs to baseline"
```

---

## ✅ Checkpoints de Validation Avant Merge

### Avant de merger feat/mcts-performance-boost → master

**1. Tests de Non-Régression**
```bash
cargo test --release
# REQUIS : 207/207 tests passing

cargo clippy -- -D warnings
# REQUIS : 0 warnings

cargo build --release
# REQUIS : successful build
```

**2. Benchmarks de Performance**
```bash
# Comparer avec baseline
cargo bench mcts_benchmark

# Devrait montrer :
# - Réduction allocations : 36,750 → <1,000
# - Amélioration score : 159.95 pts → ≥240 pts
# - Speedup parallèle : ≥6× sur 8 cores
```

**3. Tests de Stabilité**
```bash
# Lancer 100 parties AI vs Random
for i in {1..100}; do
    cargo run --release --bin test_ai_strength
done

# Analyser la variance des scores
```

---

## 🚨 Critères d'Abandon (Quand faire un rollback ?)

### Rollback Immédiat Si :
- ❌ `cargo test` échoue (régression fonctionnelle)
- ❌ Score MCTS < 155 pts (pire que baseline)
- ❌ Temps de compilation > 2× baseline (debt technique)
- ❌ Nouveaux warnings Clippy non justifiés

### Rollback Partiel Si :
- ⚠️ Score < 175 pts après Sprint 1 (Progressive Widening inefficace)
- ⚠️ Score < 210 pts après Sprint 2 (Zero-Copy overhead trop élevé)
- ⚠️ Speedup < 4× après Sprint 3 (Contention thread excessive)

### Continuer Si :
- ✅ Chaque sprint apporte +10% minimum
- ✅ Tests passent à chaque commit
- ✅ Code reste maintenable (complexité cyclomatique raisonnable)

---

## 📊 Tableau de Bord de Décision

| Sprint | Score Minimum | Action si Échec |
|--------|---------------|-----------------|
| Sprint 1 (PW) | ≥175 pts | Rollback Sprint 1, skip to Sprint 2 |
| Sprint 2 (CoW) | ≥210 pts | Rollback Sprint 2, keep Sprint 1 |
| Sprint 3 (Parallel) | ≥240 pts | Rollback Sprint 3, keep Sprint 1+2 |

**Philosophie** : Garder uniquement les optimisations **mesurables** et **reproductibles**.

---

## 🔍 Commandes de Diagnostic

### Comparer Branches
```bash
# Différences de code
git diff master..feat/mcts-performance-boost

# Statistiques
git diff --stat master..feat/mcts-performance-boost

# Fichiers modifiés
git diff --name-only master..feat/mcts-performance-boost
```

### Historique des Performances
```bash
# Liste des commits avec benchmarks
git log --grep="benchmark" --oneline

# Voir un commit spécifique
git show <commit-hash>
```

### Vérifier l'Intégrité
```bash
# Vérifier que le tag existe
git tag -v mcts-baseline-159pts

# Comparer HEAD avec baseline
git diff mcts-baseline-159pts..HEAD --stat
```

---

## 🎯 Stratégie de Merge Finale

### Si Tous les Sprints Réussissent (Score ≥240 pts)

```bash
git checkout master
git merge --no-ff feat/mcts-performance-boost -m "feat(mcts): performance improvements +150-300%

Sprints completed:
- Sprint 1: Progressive Widening (+15-25%)
- Sprint 2: Zero-Copy CoW + RAVE (+60-90%)
- Sprint 3: Parallel MCTS (+600-800%)

Performance:
- Before: 159.95 pts
- After: XXX pts (+YY%)
- Allocations: 36,750 → <1,000 (-97%)
- Speedup: Z× on 8 cores

Tests: 207/207 passing
Benchmarks: [link to results]"

git push origin master
git push origin --tags
```

### Si Succès Partiel (Garder Sprint 1 + 2 seulement)

```bash
# Cherry-pick les bons commits
git checkout master
git cherry-pick <hash-sprint-1>
git cherry-pick <hash-sprint-2>

# Tag la nouvelle baseline
git tag -a mcts-baseline-210pts -m "After Progressive Widening + Zero-Copy"

git push origin master
git push origin --tags
```

---

## 📝 Checklist Avant Merge

- [ ] Tous les tests passent : `cargo test --release`
- [ ] Aucun warning : `cargo clippy -- -D warnings`
- [ ] Benchmarks validés : score ≥240 pts ou justification
- [ ] Documentation mise à jour : README, CHANGELOG
- [ ] Code review : au moins 1 reviewer (ou self-review approfondi)
- [ ] Commit message clair avec métriques
- [ ] Tag de version créé : `mcts-vX.Y.Z`

---

## 🆘 Recovery en Cas de Désastre

### Si master est cassé par accident

```bash
# Voir l'historique des refs
git reflog

# Trouver le dernier bon commit
git log --oneline -10

# Reset master au bon état
git reset --hard <good-commit-hash>

# Force push (DANGER - seulement si seul sur le repo)
git push origin master --force

# Ou créer une branche de recovery
git checkout -b recovery-master
git reset --hard mcts-baseline-159pts
```

### Si le tag est perdu

```bash
# Lister tous les tags (même deleted)
git fsck --lost-found

# Recréer le tag depuis le commit connu
git tag -a mcts-baseline-159pts 7459292 -m "Recreated baseline tag"
```

---

## 📚 Références Git

- **Git Tags** : https://git-scm.com/book/en/v2/Git-Basics-Tagging
- **Git Branching** : https://git-scm.com/book/en/v2/Git-Branching-Branching-Workflows
- **Git Reset** : https://git-scm.com/docs/git-reset
- **Git Reflog** : https://git-scm.com/docs/git-reflog

---

**Résumé** : Avec cette stratégie, vous avez **4 niveaux de sécurité** :
1. 🏷️ Tag `mcts-baseline-159pts` (rollback complet)
2. 🌿 Branche `feat/mcts-performance-boost` (isolation)
3. 💾 Commits intermédiaires (rollback partiel)
4. 🔄 Git reflog (recovery d'urgence)

**Principe** : *"Fail fast, rollback faster"* ✅
