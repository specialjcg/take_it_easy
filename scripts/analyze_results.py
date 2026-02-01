#!/usr/bin/env python3
"""
Analyse des résultats d'entraînement Take It Easy
Usage: python3 scripts/analyze_results.py [data_file.csv]
"""

import csv
import sys
import statistics
from collections import defaultdict
from pathlib import Path

def load_arena_results(path):
    """Load arena results CSV."""
    scores_a, scores_b = [], []
    with open(path, 'r') as f:
        reader = csv.reader(f)
        next(reader)  # skip header
        for row in reader:
            if len(row) >= 3:
                try:
                    scores_a.append(int(row[1]))
                    scores_b.append(int(row[2]))
                except ValueError:
                    pass
    return scores_a, scores_b

def load_training_data(path):
    """Load training data CSV."""
    games = defaultdict(lambda: {"moves": [], "score": 0})
    with open(path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            gid = row.get('game_id', '0')
            games[gid]["score"] = int(row.get('final_score', 0))
            games[gid]["moves"].append({
                "turn": int(row.get('turn', 0)),
                "pos": int(row.get('position', 0)),
                "tile": (
                    int(row.get('tile_0', 0)),
                    int(row.get('tile_1', 0)),
                    int(row.get('tile_2', 0))
                )
            })
    return games

def analyze_score_distribution(scores):
    """Analyze score distribution."""
    print("\n📊 DISTRIBUTION DES SCORES")
    print("-" * 50)

    mean = statistics.mean(scores)
    std = statistics.stdev(scores) if len(scores) > 1 else 0
    print(f"Moyenne: {mean:.1f} ± {std:.1f} pts")
    print(f"Min/Max: [{min(scores)}, {max(scores)}]")

    scores_sorted = sorted(scores)
    n = len(scores_sorted)
    q1, q2, q3 = scores_sorted[n//4], scores_sorted[n//2], scores_sorted[3*n//4]
    print(f"Quartiles: Q1={q1}, Q2={q2}, Q3={q3}")

    print("\nDistribution par seuil:")
    for threshold in [150, 130, 120, 110, 100]:
        count = len([s for s in scores if s >= threshold])
        pct = 100 * count / len(scores)
        bar = "█" * int(pct / 5)
        print(f"  >= {threshold}: {count:4d} ({pct:5.1f}%) {bar}")

    return mean, std

def analyze_positions(games):
    """Analyze position preferences."""
    print("\n📊 POSITIONS PRÉFÉRÉES")
    print("-" * 50)

    hex_names = {
        0: "top-L", 1: "top-M", 2: "top-R",
        3: "r2-1", 4: "r2-2", 5: "r2-3", 6: "r2-4",
        7: "r3-1", 8: "r3-2", 9: "CENTER", 10: "r3-4", 11: "r3-5",
        12: "r4-1", 13: "r4-2", 14: "r4-3", 15: "r4-4",
        16: "bot-L", 17: "bot-M", 18: "bot-R"
    }

    early_positions = defaultdict(int)
    for gid, data in games.items():
        for m in data['moves']:
            if m['turn'] <= 3:
                early_positions[m['pos']] += 1

    total = sum(early_positions.values())
    print("Positions early-game (turns 0-3):")
    sorted_pos = sorted(early_positions.items(), key=lambda x: -x[1])
    for pos, count in sorted_pos[:8]:
        pct = 100 * count / total
        bar = "█" * int(pct)
        print(f"  {pos:2d} ({hex_names.get(pos, '?'):7s}): {count:4d} ({pct:4.1f}%) {bar}")

def analyze_top_vs_bottom(games):
    """Compare top vs bottom games."""
    print("\n📊 TOP 50 vs BOTTOM 50")
    print("-" * 50)

    sorted_games = sorted(games.items(), key=lambda x: -x[1]['score'])

    if len(sorted_games) < 100:
        print("Pas assez de données pour cette analyse")
        return

    top_50 = sorted_games[:50]
    bottom_50 = sorted_games[-50:]

    top_avg = sum(d['score'] for _, d in top_50) / 50
    bottom_avg = sum(d['score'] for _, d in bottom_50) / 50

    print(f"Score moyen TOP 50:    {top_avg:.1f} pts")
    print(f"Score moyen BOTTOM 50: {bottom_avg:.1f} pts")
    print(f"Écart:                 {top_avg - bottom_avg:.1f} pts")

    # First move analysis
    def get_first_positions(games_list):
        pos = defaultdict(int)
        for gid, data in games_list:
            first = [m for m in data['moves'] if m['turn'] == 0]
            for m in first:
                pos[m['pos']] += 1
        return pos

    top_pos = get_first_positions(top_50)
    bottom_pos = get_first_positions(bottom_50)

    print("\nPremier coup - TOP 50:")
    for pos, count in sorted(top_pos.items(), key=lambda x: -x[1])[:3]:
        print(f"  Position {pos}: {count}")

    print("\nPremier coup - BOTTOM 50:")
    for pos, count in sorted(bottom_pos.items(), key=lambda x: -x[1])[:3]:
        print(f"  Position {pos}: {count}")

def suggest_improvements(mean, std, max_score):
    """Suggest improvements based on analysis."""
    print("\n" + "=" * 60)
    print("🎯 AXES D'AMÉLIORATION")
    print("=" * 60)

    theoretical_max = 342
    efficiency = max_score / theoretical_max * 100

    suggestions = []

    if std > 25:
        suggestions.append(f"• VARIANCE ÉLEVÉE ({std:.1f} pts)")
        suggestions.append("  → Augmenter simulations MCTS early-game")
        suggestions.append("  → Stabiliser les décisions tours 0-5")

    if efficiency < 70:
        suggestions.append(f"• PLAFOND BAS (max={max_score}, {efficiency:.0f}% du théorique)")
        suggestions.append("  → Améliorer détection patterns de complétion")
        suggestions.append("  → Corriger normalisation scores dans MCTS")

    if mean < 130:
        suggestions.append(f"• MOYENNE INSUFFISANTE ({mean:.1f} pts)")
        suggestions.append("  → Plus de données d'entraînement (min_score=110)")
        suggestions.append("  → Augmenter epochs d'entraînement")

    for s in suggestions:
        print(s)

    print("\n" + "=" * 60)
    print("📋 COMMANDE RECOMMANDÉE")
    print("=" * 60)

    # Calculer paramètres optimaux
    min_score = 110 if mean > 120 else 100
    games = 1000 if mean < 140 else 500
    sims = 300 if std > 25 else 200
    epochs = 100 if mean < 140 else 50

    print(f"""
./scripts/improve_production.sh {games} {sims} {min_score} {epochs}

Paramètres suggérés:
  - Games: {games} (plus de données)
  - Simulations: {sims} (meilleure qualité)
  - Min score: {min_score} ({min_score/mean*100:.0f}% de la moyenne)
  - Epochs: {epochs}
""")

def main():
    print("=" * 60)
    print("      ANALYSE DES RÉSULTATS - TAKE IT EASY AI")
    print("=" * 60)

    data_dir = Path("data")

    # Find latest training data
    training_files = list(data_dir.glob("selfplay_*.csv"))
    if not training_files:
        training_files = list(data_dir.glob("*training*.csv"))

    if training_files:
        latest = max(training_files, key=lambda p: p.stat().st_mtime)
        print(f"\nAnalyse de: {latest}")
        games = load_training_data(latest)
        all_scores = [d['score'] for d in games.values()]

        mean, std = analyze_score_distribution(all_scores)
        analyze_positions(games)
        analyze_top_vs_bottom(games)
        suggest_improvements(mean, std, max(all_scores))

    # Arena results
    arena_file = data_dir / "arena_results.csv"
    if arena_file.exists():
        print("\n" + "=" * 60)
        print("RÉSULTATS ARENA (comparaison modèles)")
        print("=" * 60)
        scores_a, scores_b = load_arena_results(arena_file)
        if scores_a and scores_b:
            print(f"\nModel A: {statistics.mean(scores_a):.1f} ± {statistics.stdev(scores_a):.1f}")
            print(f"Model B: {statistics.mean(scores_b):.1f} ± {statistics.stdev(scores_b):.1f}")

if __name__ == "__main__":
    main()
