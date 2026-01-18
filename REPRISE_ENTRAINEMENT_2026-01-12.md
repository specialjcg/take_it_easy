# Reprise des essais d'entraînement - Take It Easy CNN
**Date:** 12 janvier 2026
**Objectif:** Améliorer le modèle CNN au-delà de 88 pts

---

## 🎯 État actuel

**Meilleur modèle obtenu:**
- Dataset: 130+ (2,356 exemples, avg 143.9 pts)
- Augmentation: AUCUNE
- Performance: **88 points** (niveau MCTS pur)
- Fichier modèle: **À restaurer depuis l'entraînement 130+**

**Objectif visé:** 110-150 pts (niveau baseline CNN)

---

## 📊 Historique complet des essais

| # | Dataset | Exemples | Avg Score | Augmentation | Résultat | Notes |
|---|---------|----------|-----------|--------------|----------|-------|
| 1 | Massive | 95,000 | 100.7 pts | ❌ | **52 pts** | Trop de mauvais exemples |
| 2 | 130+ | 2,356 | 143.9 pts | ❌ | **88 pts** | ✅ **MEILLEUR** - Bonne qualité |
| 3 | 140+ | 1,178 | 153.7 pts | On-the-fly invalide (90°) | **81 pts** | Rotations rectangulaires invalides |
| 4 | 140+ | 1,178 | 153.7 pts | Hexagonale buggée | **24 pts** | Bug d'encodage plateau |
| 5 | 140+ | 1,178 | 153.7 pts | Hexagonale fixée | **39 pts** | Dataset trop petit → overfitting |

---

## 🔬 Apprentissages clés

### 1. Qualité des données > Quantité
- 95k exemples moyens (avg 100) → 52 pts ❌
- 2.3k exemples excellents (avg 144) → 88 pts ✅

### 2. Augmentation géométrique: pièges critiques

**❌ CE QUI NE MARCHE PAS:**
- Rotations 90°/180°/270° sur plateau hexagonal
- Raison: incompatible avec structure hexagonale (3 directions à 120°)
- Résultat: modèle apprend des patterns impossibles

**✅ CE QUI EST MATHÉMATIQUEMENT CORRECT:**
- Permutations cycliques des 3 directions de tuiles
- Tile(a,b,c) → Tile(b,c,a) → Tile(c,a,b)
- Implémentation corrigée dans `src/data/augmentation.rs`

**⚠️ MAIS:**
- Avec dataset trop petit (1,178 ex), même l'augmentation correcte cause overfitting sévère
- L'augmentation on-the-fly avec petit dataset = variabilité excessive = convergence difficile

### 3. Taille critique du dataset
- < 2,000 exemples: risque d'overfitting élevé
- 2,000-3,000: zone idéale pour ce type de CNN
- > 5,000: nécessite plus d'epochs mais évite overfitting

---

## 🎯 Recommandation finale: OPTION B

### Combiner 130+ et 140+ SANS augmentation

**Configuration proposée:**
```bash
# Créer dataset combiné
cat filtered_datasets/supervised_130plus.csv > combined_130_140.csv
tail -n +2 filtered_datasets/supervised_140plus.csv >> combined_130_140.csv

# Entraîner
cargo run --release --bin supervised_trainer_csv -- \
  --data combined_130_140.csv \
  --epochs 100 \
  --batch-size 64 \
  --policy-lr 0.0003 \
  --value-lr 0.00003 \
  --nn-architecture cnn \
  --patience 12 \
  --seed 42
```

**Avantages:**
- **3,534 exemples** (2,356 + 1,178)
- Qualité homogène: tous ≥130 pts
- Score moyen: ~147 pts
- Pas d'augmentation = pas de complications
- Temps: ~10-15 minutes

**Performance attendue: 95-110 pts**

---

## 📁 Fichiers importants

### Datasets disponibles
- `supervised_dataset_massive.csv` - 95k ex, avg 100.7 pts
- `filtered_datasets/supervised_130plus.csv` - 2,356 ex, avg 143.9 pts ✅
- `filtered_datasets/supervised_140plus.csv` - 1,178 ex, avg 153.7 pts ✅
- `filtered_datasets/supervised_150plus.csv` - 609 ex, avg 162.8 pts

### Code modifié
- `src/data/augmentation.rs` - ✅ Augmentation hexagonale correcte implémentée
  - Permutations cycliques (3×)
  - Encoding/decoding correct (a*100 + b*10 + c)
  - Tests passés ✅

### Logs d'entraînement
- `/tmp/training_130plus.log` - Meilleur résultat (88 pts)
- `/tmp/training_140plus_hexagonal_FIXED.log` - Dernier essai (39 pts)

---

## 🚀 Prochaines étapes

### Étape 1: Créer le dataset combiné
```bash
cd /home/jcgouleau/IdeaProjects/RustProject/take_it_easy
cat filtered_datasets/supervised_130plus.csv > combined_130_140.csv
tail -n +2 filtered_datasets/supervised_140plus.csv >> combined_130_140.csv
wc -l combined_130_140.csv  # Devrait afficher 3535 (3534 + header)
```

### Étape 2: Nettoyer les poids actuels
```bash
rm -f model_weights/cnn/policy/policy.params
rm -f model_weights/cnn/value/value.params
```

### Étape 3: Lancer l'entraînement final
```bash
nohup cargo run --release --bin supervised_trainer_csv -- \
  --data combined_130_140.csv \
  --epochs 100 \
  --batch-size 64 \
  --policy-lr 0.0003 \
  --value-lr 0.00003 \
  --nn-architecture cnn \
  --patience 12 \
  --seed 42 \
  > /tmp/training_combined_130_140.log 2>&1 &
```

### Étape 4: Surveiller l'entraînement
```bash
# Vérifier les premiers epochs
tail -f /tmp/training_combined_130_140.log

# Ou afficher les epochs complétés
grep -E "Epoch.*[0-9]+/100" /tmp/training_combined_130_140.log | tail -10
```

### Étape 5: Tester le résultat final
```bash
cargo run --release --bin test_pure_cnn_policy
```

---

## 📈 Critères de succès

- ✅ **Succès complet:** > 100 pts (dépasse MCTS pur significativement)
- ✅ **Succès partiel:** 90-100 pts (amélioration sur 88 pts)
- ⚠️ **Résultat moyen:** 88-90 pts (stagnation)
- ❌ **Échec:** < 88 pts (régression)

---

## 💡 Si Option B ne donne pas >100 pts

### Plan alternatif: Augmentation matérialisée

Si le résultat est < 95 pts, essayer:

```bash
# Créer dataset 130+ avec augmentation matérialisée (3x = 7,068 ex)
# TODO: Implémenter script de matérialisation qui applique les 3 permutations
# puis entraîner sur ce dataset augmenté
```

Avantages:
- Chaque variante vue plusieurs fois → meilleure convergence
- Pas de variabilité excessive comme avec on-the-fly
- Dataset final: 7,068 exemples de haute qualité

---

## 📝 Notes techniques

### Encodage des tuiles
- Format: `Tile(a,b,c)` → entier `a*100 + b*10 + c`
- Exemple: Tile(1,6,3) → 163
- Plateau: 19 entiers (positions 0-18)

### Permutations cycliques valides
```
Original:      Tile(a, b, c)
CyclicPerm1:   Tile(b, c, a)  # Rotation 120°
CyclicPerm2:   Tile(c, a, b)  # Rotation 240°
```

### Structure du plateau (19 positions)
```
    0   1
   2  3  4
  5  6  7  8
   9 10 11
  12 13 14 15
   16 17 18
```

---

## ✅ Checklist avant de commencer

- [ ] Vérifier que les datasets 130+ et 140+ existent
- [ ] Créer le dataset combiné `combined_130_140.csv`
- [ ] Nettoyer les anciens poids du modèle
- [ ] Lancer l'entraînement avec les paramètres recommandés
- [ ] Surveiller les premiers epochs (bonnes losses)
- [ ] Attendre la fin (early stopping ou 100 epochs)
- [ ] Tester avec `test_pure_cnn_policy`
- [ ] Comparer le résultat avec 88 pts (baseline)

---

**Auteur:** Claude Sonnet 4.5
**Session:** 2026-01-11 → 2026-01-12
**Statut:** Prêt pour Option B - Entraînement combiné 130+140+
