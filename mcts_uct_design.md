# MCTS UCT Implementation Design

## Problem with Current Batch MCTS
```rust
for _ in 0..simulations {
    for &position in &subset_moves {  // ← Explores ALL positions!
        run_rollout(position);
    }
}
```
Result: All positions explored equally → uniform data → policy can't learn

## Solution: Flat UCT (Simplified)
Instead of exploring all positions, **sample ONE position per simulation** according to policy:

```rust
for _ in 0..simulations {
    // 1. Sample ONE position using policy as probability distribution
    let position = sample_from_policy(&policy, &legal_moves);
    
    // 2. Explore ONLY that position
    run_rollout(position);
    
    // 3. Update statistics for that position
    update_stats(position, score);
}
```

## Key Changes
1. **Selection:** Use policy network probabilities to sample positions
2. **Exploration:** Only one position per simulation (not all)
3. **Statistics:** Track visit counts and values per position
4. **UCB Formula:** Incorporate visit counts for exploration/exploitation

## Expected Behavior
- Policy network influences which positions are explored more
- High-probability positions get more simulations  
- Creates non-uniform data
- Policy can learn from this data
- Breaks the circular learning problem

## Implementation Steps
1. Create `mcts_find_best_position_uct` function
2. Sample position using policy as weights
3. Run single-position rollout
4. Track visit counts and UCB scores
5. Return distribution based on visit counts

## File: `src/mcts/algorithm_uct.rs` (new file)
