#!/usr/bin/env python3
"""
Analyze expert training data quality
"""

import json
import sys
from collections import Counter
from pathlib import Path

def analyze_expert_data(json_path):
    """Analyze expert training data and print statistics"""
    print(f"📊 Analyzing: {json_path}")
    print("=" * 60)

    # Load data
    with open(json_path, 'r') as f:
        games = json.load(f)

    # Basic stats
    num_games = len(games)
    total_moves = sum(len(g['moves']) for g in games)
    scores = [g['final_score'] for g in games]

    print(f"\n📈 Basic Statistics:")
    print(f"  Games: {num_games}")
    print(f"  Total training examples: {total_moves}")
    print(f"  Examples per game: {total_moves / num_games:.1f}")

    print(f"\n🎯 Score Distribution:")
    print(f"  Average: {sum(scores) / len(scores):.2f} pts")
    print(f"  Min: {min(scores)} pts")
    print(f"  Max: {max(scores)} pts")
    print(f"  Median: {sorted(scores)[len(scores)//2]} pts")
    print(f"  Std dev: {(sum((s - sum(scores)/len(scores))**2 for s in scores) / len(scores))**0.5:.2f}")

    # Score ranges
    ranges = {
        "0-50": 0,
        "51-100": 0,
        "101-130": 0,
        "131-150": 0,
        "151-170": 0,
        "171+": 0
    }
    for score in scores:
        if score <= 50:
            ranges["0-50"] += 1
        elif score <= 100:
            ranges["51-100"] += 1
        elif score <= 130:
            ranges["101-130"] += 1
        elif score <= 150:
            ranges["131-150"] += 1
        elif score <= 170:
            ranges["151-170"] += 1
        else:
            ranges["171+"] += 1

    print(f"\n  Score ranges:")
    for range_name, count in ranges.items():
        pct = 100 * count / num_games
        bar = "█" * int(pct / 2)
        print(f"    {range_name:>10}: {count:3d} games ({pct:5.1f}%) {bar}")

    # Position distribution (where expert chooses to place tiles)
    all_positions = []
    for game in games:
        for move in game['moves']:
            all_positions.append(move['best_position'])

    position_counts = Counter(all_positions)
    print(f"\n📍 Position Usage Distribution:")
    print(f"  Most used positions (top 5):")
    for pos, count in position_counts.most_common(5):
        pct = 100 * count / len(all_positions)
        print(f"    Position {pos:2d}: {count:4d} times ({pct:5.2f}%)")

    # Check if distribution is too skewed
    max_pct = 100 * position_counts.most_common(1)[0][1] / len(all_positions)
    if max_pct > 15:
        print(f"  ⚠️ Position {position_counts.most_common(1)[0][0]} is overused ({max_pct:.1f}%)")
    else:
        print(f"  ✅ Position distribution looks balanced (max: {max_pct:.1f}%)")

    # Value distribution analysis
    all_values = []
    for game in games:
        for move in game['moves']:
            all_values.append(move['expected_value'])

    print(f"\n💰 Expected Value Distribution:")
    print(f"  Average: {sum(all_values) / len(all_values):.2f}")
    print(f"  Min: {min(all_values):.2f}")
    print(f"  Max: {max(all_values):.2f}")
    print(f"  Range: {max(all_values) - min(all_values):.2f}")

    # Check for diversity in values
    value_std = (sum((v - sum(all_values)/len(all_values))**2 for v in all_values) / len(all_values))**0.5
    print(f"  Std dev: {value_std:.2f}")
    if value_std < 5:
        print(f"  ⚠️ Low value diversity (std dev < 5)")
    else:
        print(f"  ✅ Good value diversity")

    # Turn-by-turn analysis
    print(f"\n🔄 Turn-by-Turn Analysis:")
    turn_scores = {}
    for game in games:
        for move in game['moves']:
            turn = move['turn']
            if turn not in turn_scores:
                turn_scores[turn] = []
            turn_scores[turn].append(move['expected_value'])

    print(f"  Average expected value by turn (first 5, last 5):")
    for turn in list(range(5)) + list(range(14, 19)):
        if turn in turn_scores:
            avg = sum(turn_scores[turn]) / len(turn_scores[turn])
            print(f"    Turn {turn:2d}: {avg:6.2f} pts")

    # Data quality checks
    print(f"\n✅ Data Quality Checks:")

    # Check all games have 19 moves
    invalid_games = [g['game_id'] for g in games if len(g['moves']) != 19]
    if invalid_games:
        print(f"  ❌ {len(invalid_games)} games don't have 19 moves: {invalid_games}")
    else:
        print(f"  ✅ All games have exactly 19 moves")

    # Check plateau encoding
    sample_move = games[0]['moves'][0]
    if len(sample_move['plateau_before']) == 19:
        print(f"  ✅ Plateau encoding is correct (19 cells)")
    else:
        print(f"  ❌ Plateau encoding is wrong ({len(sample_move['plateau_before'])} cells)")

    # Check tile encoding
    if all(k in sample_move['tile'] for k in ['value1', 'value2', 'value3']):
        print(f"  ✅ Tile encoding is correct")
    else:
        print(f"  ❌ Tile encoding is missing fields")

    # Check position bounds
    out_of_bounds = [pos for pos in all_positions if pos < 0 or pos >= 19]
    if out_of_bounds:
        print(f"  ❌ {len(out_of_bounds)} positions out of bounds [0-18]")
    else:
        print(f"  ✅ All positions in valid range [0-18]")

    print(f"\n{'=' * 60}")
    print(f"✅ Analysis complete!")

    # Return summary stats
    return {
        'num_games': num_games,
        'avg_score': sum(scores) / len(scores),
        'min_score': min(scores),
        'max_score': max(scores),
        'total_examples': total_moves
    }

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python analyze_expert_data.py <path_to_json>")
        sys.exit(1)

    json_path = sys.argv[1]
    if not Path(json_path).exists():
        print(f"Error: File not found: {json_path}")
        sys.exit(1)

    analyze_expert_data(json_path)
