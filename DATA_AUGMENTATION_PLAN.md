# Data Augmentation - Plan d'Implémentation

## Objectif: Multiplier 95k exemples par 4-6x → 380k-570k exemples

### Transformations Valides pour Take It Easy

#### 1. Rotations (×4)

**Plateau hexagonal 5×5:**
```
Positions originales (19):
     0   1
   2   3   4
 5   6   7   8
   9  10  11
    12  13

   14  15
 16  17  18
```

**Rotations possibles:**
- 0°   (original)
- 90°  (rotation horaire)
- 180° (demi-tour)
- 270° (rotation anti-horaire)

**Mapping de positions:**
```rust
// Rotation 90° horaire
let rot90: [usize; 19] = [
    12, 9, 5, 2, 0,     // Top row rotated
    13, 10, 6, 3, 1,    // Second row
    14, 11, 7, 4,       // Third row
    15, 8,              // Fourth row
    16, 17, 18          // Bottom row
];

// Rotation 180°
let rot180: [usize; 19] = [
    18, 17, 16, 15, 14,
    13, 12, 11, 10, 9,
    8, 7, 6, 5,
    4, 3,
    2, 1, 0
];

// Rotation 270° = rot90^3
```

**Tile orientation reste identique** (tuiles sont orientées, pas rotatives)

#### 2. Symétrie Horizontale (×2)

**Axe vertical central:**
```
     0   1              1   0
   2   3   4    →     4   3   2
 5   6   7   8      8   7   6   5
   9  10  11         11  10   9
    12  13            13  12

   14  15            15  14
 16  17  18        18  17  16
```

**Mapping:**
```rust
let flip_h: [usize; 19] = [
    1, 0,           // Row 1 flipped
    4, 3, 2,        // Row 2
    8, 7, 6, 5,     // Row 3
    11, 10, 9,      // Row 4
    13, 12,         // Row 5
    15, 14,         // Row 6
    18, 17, 16      // Row 7
];
```

#### 3. Symétrie Verticale (×2)

**Axe horizontal central:**
```
Flip vertical le long de l'axe central
```

### Combinaisons Totales

**Sans doublons:**
- Rotations: 4
- + Flip horizontal: 4
- **Total: 8 transformations uniques**

**95k exemples × 8 = 760k exemples!**

### Implémentation

#### Nouveau fichier: `src/data/augmentation.rs`

```rust
pub fn augment_example(
    plateau_state: &[i32],
    tile: (i32, i32, i32),
    position: usize,
    final_score: i32,
    transform: AugmentTransform,
) -> (Vec<i32>, (i32, i32, i32), usize, i32) {
    match transform {
        AugmentTransform::Original => {
            (plateau_state.to_vec(), tile, position, final_score)
        }
        AugmentTransform::Rot90 => {
            let new_plateau = apply_rotation_90(plateau_state);
            let new_position = ROTATION_90_MAP[position];
            (new_plateau, tile, new_position, final_score)
        }
        // ... autres transformations
    }
}

fn apply_rotation_90(plateau: &[i32]) -> Vec<i32> {
    let mut result = vec![0; 19];
    for (old_pos, &new_pos) in ROTATION_90_MAP.iter().enumerate() {
        result[new_pos] = plateau[old_pos];
    }
    result
}
```

#### Modifier `supervised_trainer_csv.rs`

```rust
// Lors du chargement des données
fn load_csv_data_with_augmentation(path: &str) -> Vec<TrainingExample> {
    let base_examples = load_csv_data(path);
    let mut augmented = Vec::new();

    for example in base_examples {
        // Original
        augmented.push(example.clone());

        // 7 transformations
        for transform in [Rot90, Rot180, Rot270, FlipH, ...] {
            augmented.push(augment_example(&example, transform));
        }
    }

    augmented  // 8x more examples!
}
```

### Avantages

1. **Volume:** 95k → 760k exemples (8x)
2. **Généralisation:** Le réseau apprend l'invariance spatiale
3. **Pas de temps CPU:** Augmentation à la volée pendant l'entraînement
4. **Diversité:** Réduit l'overfitting

### Précautions

- **Tiles ne changent PAS:** Les valeurs (1-9, 2-7, 3-8) restent identiques
- **Positions mappées correctement:** Chaque transformation a sa table de mapping
- **Score final inchangé:** Le score ne dépend pas de l'orientation

### Priorité

**Implémenter D'ABORD** avant de générer plus de données:
- 95k × 8 = 760k (équivalent 40k jeux)
- Gain immédiat sans temps de génération
