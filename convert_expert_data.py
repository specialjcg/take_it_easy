#!/usr/bin/env python3
"""Convert game-based expert data to flat training format."""

import json
import sys

def tile_to_int(tile):
    """Convert tile dict to integer representation."""
    return tile['value1'] * 100 + tile['value2'] * 10 + tile['value3']

def plateau_to_tensor(plateau_before, tile, turn, total_turns=19):
    """Convert plateau state to 8-channel tensor (flattened)."""
    # This is a simplified version - in reality we'd need to match
    # the exact tensor_conversion logic from Rust
    # For now, create a basic representation
    
    # Channel 0-2: Board state (3 values per tile)
    tensor = []
    for pos in plateau_before:
        if pos == -1:
            tensor.extend([0.0, 0.0, 0.0])
        else:
            v1 = (pos // 100) / 10.0
            v2 = ((pos % 100) // 10) / 10.0
            v3 = (pos % 10) / 10.0
            tensor.extend([v1, v2, v3])
    
    # Channel 3: Current tile
    for _ in range(19):
        tensor.extend([
            tile['value1'] / 10.0,
            tile['value2'] / 10.0,
            tile['value3'] / 10.0
        ])
    
    # Channel 4: Available positions
    for pos in plateau_before:
        tensor.extend([1.0 if pos == -1 else 0.0] * 3)
    
    # Channel 5-7: Turn info (simplified)
    turn_val = turn / total_turns
    for _ in range(19):
        tensor.extend([turn_val, turn_val, turn_val])
    
    return tensor

def convert_games_to_flat_format(input_file, output_file):
    """Convert game-based format to flat training examples."""
    print(f"Loading {input_file}...")
    with open(input_file, 'r') as f:
        games = json.load(f)
    
    print(f"Converting {len(games)} games...")
    examples = []
    
    for game in games:
        final_score = game['final_score']
        # Normalize score to [-1, 1]
        normalized_score = ((final_score / 200.0) * 2.0) - 1.0
        normalized_score = max(-1.0, min(1.0, normalized_score))
        
        for move in game['moves']:
            # Create training example for this move
            state_tensor = plateau_to_tensor(
                move['plateau_before'],
                move['tile'],
                move['turn']
            )
            
            example = {
                'state': state_tensor,
                'policy_target': move['best_position'],
                'value_target': normalized_score
            }
            examples.append(example)
    
    print(f"Generated {len(examples)} training examples")
    print(f"Saving to {output_file}...")
    
    with open(output_file, 'w') as f:
        json.dump(examples, f)
    
    print(f"✅ Saved {len(examples)} examples to {output_file}")
    print(f"   Examples per game: {len(examples) / len(games):.1f}")

if __name__ == '__main__':
    if len(sys.argv) != 3:
        print("Usage: python3 convert_expert_data.py <input.json> <output.json>")
        sys.exit(1)
    
    convert_games_to_flat_format(sys.argv[1], sys.argv[2])
