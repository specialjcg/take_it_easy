# Roadmap: 107 pts → 130-150+ pts

## Stratégie Optimale (Ordre d'Implémentation)

### Phase 1: Augmentation de Données (PRIORITÉ 1) ⚡
**Temps: 4-6h | Impact: +++**

✅ **Pourquoi en premier:**
- Gain immédiat: 95k → 760k exemples (8x)
- Pas de temps de génération
- Amélioration attendue: +10-20 pts

**Actions:**
1. Créer `src/data/augmentation.rs`
2. Implémenter transformations (rotations + flips)
3. Modifier `supervised_trainer_csv.rs` pour augmenter à la volée
4. Réentraîner avec 760k exemples
5. **Test: objectif 115-125 pts**

**Estimation score:** 115-125 pts

---

### Phase 2: Architecture v2 avec Attention (PRIORITÉ 2) 🏗️
**Temps: 8-12h | Impact: ++++**

✅ **Pourquoi après Phase 1:**
- Les 760k exemples permettent d'entraîner un modèle plus gros
- Architecture attention a besoin de beaucoup de données
- Synergie: données × architecture

**Actions:**
1. Implémenter `BagAttentionModule` dans `policy_value_net.rs`
2. Créer `DualPathwayPolicyNet` (base + bag séparés)
3. Augmenter ResBlocks: 3 → 5
4. Entraîner sur 760k exemples augmentés
5. **Test: objectif 130-145 pts**

**Estimation score:** 130-145 pts

---

### Phase 3: Dataset Massif (PRIORITÉ 3) 📊
**Temps: 7-14h génération + 2h entraînement | Impact: ++**

✅ **Si besoin de plus:**
- Générer 10k-20k jeux supplémentaires
- Sans augmentation: 190k-380k
- Avec augmentation: 1.5M-3M exemples!

**Actions:**
1. Générer 10k jeux (190k exemples) en background
2. Augmenter → 1.5M exemples
3. Fine-tuner architecture v2
4. **Test: objectif 140-155+ pts**

**Estimation score:** 140-155+ pts

---

## Timeline Complète

| Phase | Durée | Score Attendu | Cumul Temps |
|-------|-------|---------------|-------------|
| Baseline actuel | - | **107 pts** | 0h |
| **Phase 1: Augmentation** | 6h | **115-125 pts** | 6h |
| **Phase 2: Architecture v2** | 10h | **130-145 pts** | 16h |
| **Phase 3: Dataset massif** | 16h | **140-155+ pts** | 32h |

**Estimation réaliste:** 130-145 pts en **16h de travail**

---

## Décision: Par Quoi Commencer?

### Option A: Phase 1 Seule (RAPIDE) ⚡
**Avantages:**
- 6h seulement
- Gain garanti (+10-20 pts)
- Validation rapide de l'approche

**Commande:**
```bash
# Implémenter augmentation
# Entraîner avec 760k exemples
# Tester → si >120 pts, continuer Phase 2
```

### Option B: Phase 1 + 2 en Parallèle (OPTIMAL) 🚀
**Avantages:**
- Pendant l'entraînement Phase 1, coder Phase 2
- Gain de temps total
- Synergie maximale

**Commandes parallèles:**
```bash
# Terminal 1: Entraîner avec augmentation (6h)
# Terminal 2: Coder architecture v2 (8h)
# Puis: Entraîner v2 avec augmentation (2h)
```

### Option C: Tout en Une Fois (COMPLET) 📊
**Avantages:**
- Solution complète
- Score maximal

**Inconvénients:**
- 32h de travail
- Pas de validation intermédiaire

---

## Recommandation

**Commencer par Phase 1 (Augmentation)**

**Justification:**
1. Rapide (6h)
2. Gain garanti
3. Valide l'approche avant gros investissement Phase 2/3
4. Si score >120 pts → continuer Phase 2
5. Si score plateau <115 pts → revoir stratégie

**Commande de démarrage:**
```bash
# Créer augmentation.rs
# Modifier supervised_trainer_csv.rs
# Entraîner avec 760k exemples
cargo run --release --bin supervised_trainer_csv -- \
  --data supervised_dataset_massive.csv \
  --augmentation 8x \
  --epochs 200 \
  --batch-size 32
```

---

## Questions pour Validation

1. **Confirmer priorités:** Phase 1 → Phase 2 → Phase 3 ?
2. **Timeline acceptable:** 16h pour 130-145 pts ?
3. **Commencer maintenant:** Implémenter Phase 1 (augmentation) ?
