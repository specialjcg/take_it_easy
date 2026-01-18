# Architecture CNN v2 - Plan d'Amélioration

## Objectif: Passer de 107 pts à 130-150+ pts

### Architecture Actuelle (v1)
```
Input: 17×5×5
Conv1: 17→128 + GN + LeakyReLU
ResBlocks: 128→128→128→96 (3 blocks)
PolicyConv: 96→1
Output: 19 logits
Score: 107 pts
```

### Architecture Proposée (v2)

#### 1. Séparation Base / Bag avec Attention

```rust
Input: 17×5×5
  ↓
Split:
  - base_features: 8×5×5 (plateau + tile + turn)
  - bag_features: 9×5×5 (tiles restantes)

// Base pathway
base = Conv1(8→64) + GN + LeakyReLU
     ResBlock(64→64) × 2

// Bag pathway avec Attention
bag = Conv1(9→64) + GN + LeakyReLU
    BagAttention(64) → weighted features
    ResBlock(64→64) × 2

// Fusion
merged = Concat[base, bag]  // 128 channels
       ResBlock(128→128) × 3
       PolicyConv(128→1)

Output: 19 logits
```

#### 2. Bag Attention Module

**Principe:** Apprendre quelles tiles restantes sont les plus importantes

```rust
struct BagAttentionModule {
    query: Conv2D,   // 64→16
    key: Conv2D,     // 64→16
    value: Conv2D,   // 64→64
}

fn forward(bag_features):
    Q = query(bag_features)      // [B, 16, 5, 5]
    K = key(bag_features)        // [B, 16, 5, 5]
    V = value(bag_features)      // [B, 64, 5, 5]

    // Spatial attention
    attention = softmax(Q ⊗ K^T / sqrt(16))
    output = attention ⊗ V

    return output + bag_features  // Residual
```

#### 3. Architecture Complète

```
Parameters: ~5-6M (vs 3M)
Depth: 7 ResBlocks (vs 3)
Attention: 1 module sur bag
Skip connections: Multi-scale features
```

### Changements dans policy_value_net.rs

**Nouveaux modules à ajouter:**

1. `BagAttentionModule` (lignes ~290-350)
2. `DualPathwayPolicyNet` (lignes ~350-500)
3. Modifier `PolicyNetCNN::new()` pour utiliser dual pathway

### Paramètres d'Entraînement Ajustés

```rust
Epochs: 200
Batch size: 32 (plus petit pour 5M params)
Policy LR: 0.0003 (réduit)
Value LR: 0.00003
Patience: 20 (plus élevée)
```

### Attentes

**Avec 190k exemples + architecture v2:**
- Ratio: 190k / 5M = 1:26 (excellent)
- Score attendu: **125-145 pts**
- Convergence: epoch 40-80
