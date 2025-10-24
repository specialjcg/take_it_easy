# Silver GNN Architecture - Technical Documentation

## Executive Summary

**Goal**: Achieve >140 pts average score, surpassing both Bronze GNN (125 pts) and CNN baseline (127 pts)

**Architecture**: Graph Neural Network with enhanced capacity, residual connections, and batch normalization

**Status**: Training in progress (1000 games from scratch)

---

## Architecture Evolution

### Bronze GNN (Baseline)
```
Layer dims: [64, 64, 64]
Features: Basic graph convolutions
Score: 125.41 pts (benchmark)
Limitation: Insufficient capacity
```

### Silver GNN (Current)
```
Layer dims: [128, 128, 64]
Features:
  - 3x parameter capacity
  - Residual connections (skip connections)
  - Batch normalization per layer
  - Adaptive dropout
Target: >140 pts
```

---

## Key Improvements

### 1. Increased Capacity

**Rationale**: Bronze GNN (64 dims) was bottlenecked in representation learning

**Implementation**:
```rust
// Bronze: const DEFAULT_HIDDEN: &[i64] = &[64, 64, 64];
// Silver:
const DEFAULT_HIDDEN: &[i64] = &[128, 128, 64];
```

**Impact**:
- Layer 1: 64→128 (+100% params)
- Layer 2: 64→128 (+100% params, **with residual**)
- Layer 3: 128→64 (output bottleneck preserved)
- **Total**: ~3x more learnable parameters

**Expected gain**: +5-7 pts

---

### 2. Residual Connections

**Rationale**: Enable deeper networks and prevent vanishing gradients (inspired by ResNet)

**Implementation**:
```rust
pub struct GraphLayer {
    w_self: nn::Linear,
    w_neigh: nn::Linear,
    bn: nn::BatchNorm,
    use_residual: bool,  // NEW: auto-enabled when in_dim == out_dim
}

// In forward():
let residual = if self.use_residual {
    Some(x.shallow_clone())
} else {
    None
};

// ... graph convolution ...

if let Some(res) = residual {
    out = out + res;  // Skip connection
}
```

**Where active**:
- Layer 1: ❌ (input_dim → 128, dims don't match)
- Layer 2: ✅ (128 → 128, **residual active!**)
- Layer 3: ❌ (128 → 64, dims don't match)

**Expected gain**: +2-3 pts (better gradient flow)

---

### 3. Batch Normalization

**Rationale**: Stabilize training, enable higher learning rates, reduce internal covariate shift

**Implementation**:
```rust
let bn = nn::batch_norm1d(
    path / "bn",
    out_dim,
    nn::BatchNormConfig {
        ws_init: nn::Init::Const(1.0),
        bs_init: nn::Init::Const(0.0),
        ..Default::default()
    },
);

// In forward() - reshape for BatchNorm1d:
// Input: [batch, nodes=19, features]
// Transpose to: [batch, features, nodes] for BatchNorm1d
let out_transposed = out.transpose(1, 2);
let out_normalized = out_transposed.apply_t(&self.bn, train);
out = out_normalized.transpose(1, 2);  // Back to [batch, nodes, features]
```

**Expected gain**: +1-2 pts (training stability)

---

## Complete Forward Pass

```
Input: Plateau → Node features [batch, 19, 6]
                                    ↓
┌─────────────────────────────────────────────────┐
│ GraphLayer 1: 6 → 128                           │
│  ├─ w_self:  6 → 128  (linear transform)        │
│  ├─ w_neigh: 6 → 128  (neighbor aggregation)    │
│  ├─ out = w_self(x) + adj @ w_neigh(x)          │
│  ├─ out = BatchNorm1d(out)                      │
│  ├─ out = out (no residual, 6≠128)              │
│  └─ out = ReLU(out) + Dropout(0.3)              │
└─────────────────────────────────────────────────┘
                    ↓ [batch, 19, 128]
┌─────────────────────────────────────────────────┐
│ GraphLayer 2: 128 → 128                         │
│  ├─ w_self:  128 → 128                          │
│  ├─ w_neigh: 128 → 128                          │
│  ├─ out = w_self(x) + adj @ w_neigh(x)          │
│  ├─ out = BatchNorm1d(out)                      │
│  ├─ out = out + x  ✅ RESIDUAL (128==128)       │
│  └─ out = ReLU(out) + Dropout(0.3)              │
└─────────────────────────────────────────────────┘
                    ↓ [batch, 19, 128]
┌─────────────────────────────────────────────────┐
│ GraphLayer 3: 128 → 64                          │
│  ├─ w_self:  128 → 64                           │
│  ├─ w_neigh: 128 → 64                           │
│  ├─ out = w_self(x) + adj @ w_neigh(x)          │
│  ├─ out = BatchNorm1d(out)                      │
│  ├─ out = out (no residual, 128≠64)             │
│  └─ out = ReLU(out) + Dropout(0.3)              │
└─────────────────────────────────────────────────┘
                    ↓ [batch, 19, 64]
                    ↓
        ┌───────────┴───────────┐
        ↓                       ↓
┌─────────────────┐   ┌─────────────────┐
│ PolicyHead      │   │ ValueHead       │
│ 64 → 1 per node │   │ 64 → 1 pooled   │
│ → Softmax(19)   │   │ → Tanh()        │
└─────────────────┘   └─────────────────┘
```

---

## Training Configuration

### From Scratch Strategy

**Why from scratch?**
- Bronze GNN weights incompatible (64 vs 128 dims)
- Clean dataset aligned with new architecture
- Avoid feature distribution mismatch

**Parameters**:
```bash
--num-games 1000            # 2x Bronze (more data for bigger network)
--num-simulations 200       # +33% exploration vs Bronze (150)
--evaluation-interval 100   # Checkpoints every 100 games
--min-score-high 145        # Strict quality threshold
--min-score-medium 125      # Higher baseline than Bronze (120)
--medium-mix-ratio 0.15     # Less medium-quality data (was 0.20)
--dynamic-sim-boost 50      # Adaptive simulation count
```

**Learning dynamics**:
- More parameters → slower convergence
- Higher quality data → better final performance
- Longer training → opportunity to surpass CNN

**Estimated time**: 60-90 minutes

---

## Expected Performance

### Baseline Comparison

| Model          | Score  | vs Pure MCTS | Architecture               |
|----------------|--------|--------------|----------------------------|
| Pure MCTS      | 102 pts| -            | No neural guidance         |
| Bronze GNN     | 125 pts| +23 pts      | [64, 64, 64]               |
| CNN Baseline   | 128 pts| +26 pts      | ResNet-style conv          |
| **Silver GNN** | **?**  | **?**        | **[128, 128, 64] + BN + Residual** |

### Success Criteria

- **Minimum**: 130 pts (+5 pts vs Bronze, matches CNN)
- **Target**: 140 pts (+10 pts vs CNN baseline)
- **Stretch**: 145+ pts (gold tier)

### Confidence Intervals

Based on architectural improvements:

- **Pessimistic (30%)**: 128-132 pts
  - Capacity helps but not enough
  - Matches CNN, small improvement over Bronze

- **Expected (50%)**: 135-142 pts
  - All improvements synergize
  - Clear win over CNN
  - Approaching target

- **Optimistic (20%)**: 145+ pts
  - Architecture is well-suited to hexagonal structure
  - Graph representation captures spatial relationships optimally
  - Achieves gold tier

---

## Risk Factors

### 1. Training Instability (Low risk)

**Symptom**: Scores oscillate or plateau early

**Mitigation**:
- BatchNorm stabilizes gradients
- Residual connections prevent vanishing gradients
- Adaptive dropout (0.3) prevents overfitting

### 2. Dataset Quality (Medium risk)

**Symptom**: Model learns suboptimal strategies

**Mitigation**:
- High quality thresholds (145/125 pts)
- Low medium mix ratio (15%)
- 200 simulations for better exploration

### 3. Architecture Mismatch (Low risk)

**Symptom**: GNN underperforms CNN despite larger capacity

**Root cause**: Hexagonal board may not benefit from graph structure

**Mitigation**: N/A (fundamental hypothesis test)

---

## Files Modified

### Core Architecture
- `src/neural/gnn.rs`: GraphLayer, GraphEncoder, GraphPolicyNet, GraphValueNet

### Key Changes
```rust
// Line 6
const DEFAULT_HIDDEN: &[i64] = &[128, 128, 64];

// Line 12-13
bn: nn::BatchNorm,
use_residual: bool,

// Line 38-73
pub fn forward(&self, x: &Tensor, adj: &Tensor, train: bool, dropout: f64) -> Tensor {
    // ... residual + batchnorm logic ...
}
```

---

## Next Steps

1. **Monitor Training** (current)
   - Check progress every 100 games
   - Watch for convergence trends
   - Validate score improvement

2. **Benchmark vs CNN** (after training)
   ```bash
   cargo run --release --bin compare_mcts -- \
     -g 200 -s 200 --nn-architecture gnn
   ```

3. **Analyze Results**
   - If >140 pts: Document success, commit as Silver GNN
   - If 130-140 pts: Iterate with hyperparameter tuning
   - If <130 pts: Investigate failure modes

4. **Potential Gold GNN** (if Silver succeeds)
   - Graph Attention Networks (GAT)
   - Multi-head attention
   - Curriculum learning
   - Ensemble methods (CNN + GNN)

---

## References

- Bronze GNN benchmark: 125.41 pts (200 games, 150 sims)
- CNN baseline benchmark: 127.72 pts (200 games, 150 sims)
- Training log: `training_silver_gnn_from_scratch.log`
- Monitor script: `./monitor_training.sh`

---

**Last updated**: 2025-10-24
**Status**: Training in progress (shell ID: 043591)
**ETA**: ~60-90 minutes from 22:07 UTC
