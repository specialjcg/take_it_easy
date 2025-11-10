#!/usr/bin/env python3
"""
Analyze hyperparameter tuning results from CSV log.

Usage:
    python3 scripts/analyze_hyperparameters.py hyperparameter_tuning_log.csv
"""

import sys
import pandas as pd
import numpy as np


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 analyze_hyperparameters.py <csv_file>")
        sys.exit(1)

    csv_file = sys.argv[1]

    try:
        df = pd.read_csv(csv_file)
    except FileNotFoundError:
        print(f"Error: File '{csv_file}' not found.")
        sys.exit(1)
    except Exception as e:
        print(f"Error reading CSV: {e}")
        sys.exit(1)

    print("=" * 80)
    print("📊 HYPERPARAMETER TUNING ANALYSIS")
    print("=" * 80)
    print(f"Total configurations tested: {len(df)}")
    print(f"Games per configuration: {df['games'].iloc[0]}")
    print(f"Simulations per move: {df['simulations'].iloc[0]}")
    print()

    # Sort by average score (descending)
    df_sorted = df.sort_values('avg_score', ascending=False)

    print("=" * 80)
    print("🏆 TOP 10 CONFIGURATIONS")
    print("=" * 80)

    for idx, (i, row) in enumerate(df_sorted.head(10).iterrows(), 1):
        print(f"\n#{idx} - Average Score: {row['avg_score']:.2f} ± {row['std_dev']:.2f}")
        print(f"    c_puct: {row['c_puct_early']:.2f}/{row['c_puct_mid']:.2f}/{row['c_puct_late']:.2f}")
        print(f"    prune: {row['prune_early']:.3f}/{row['prune_mid1']:.3f}/{row['prune_mid2']:.3f}/{row['prune_late']:.3f}")
        print(f"    rollout: {int(row['rollout_strong'])}/{int(row['rollout_medium'])}/{int(row['rollout_default'])}/{int(row['rollout_weak'])}")
        print(f"    weights: CNN={row['weight_cnn']:.2f}, Roll={row['weight_rollout']:.2f}, Heur={row['weight_heuristic']:.2f}, Ctx={row['weight_contextual']:.2f}")

    print("\n" + "=" * 80)
    print("📈 STATISTICS")
    print("=" * 80)
    print(f"Best score:  {df['avg_score'].max():.2f}")
    print(f"Worst score: {df['avg_score'].min():.2f}")
    print(f"Mean score:  {df['avg_score'].mean():.2f}")
    print(f"Median score: {df['avg_score'].median():.2f}")
    print(f"Std dev:     {df['avg_score'].std():.2f}")

    # Find best configuration
    best_row = df_sorted.iloc[0]

    print("\n" + "=" * 80)
    print("⭐ BEST CONFIGURATION")
    print("=" * 80)
    print(f"Average Score: {best_row['avg_score']:.2f} ± {best_row['std_dev']:.2f}")
    print(f"Range: {int(best_row['min_score'])}-{int(best_row['max_score'])}")
    print()
    print("Command to reproduce:")
    print(f"cargo run --release --bin tune_hyperparameters -- \\")
    print(f"  --games {int(best_row['games'])} \\")
    print(f"  --seed {int(best_row['seed'])} \\")
    print(f"  --c-puct-early {best_row['c_puct_early']:.3f} \\")
    print(f"  --c-puct-mid {best_row['c_puct_mid']:.3f} \\")
    print(f"  --c-puct-late {best_row['c_puct_late']:.3f} \\")
    print(f"  --variance-mult-high {best_row['variance_mult_high']:.3f} \\")
    print(f"  --variance-mult-low {best_row['variance_mult_low']:.3f} \\")
    print(f"  --prune-early {best_row['prune_early']:.3f} \\")
    print(f"  --prune-mid1 {best_row['prune_mid1']:.3f} \\")
    print(f"  --prune-mid2 {best_row['prune_mid2']:.3f} \\")
    print(f"  --prune-late {best_row['prune_late']:.3f} \\")
    print(f"  --rollout-strong {int(best_row['rollout_strong'])} \\")
    print(f"  --rollout-medium {int(best_row['rollout_medium'])} \\")
    print(f"  --rollout-default {int(best_row['rollout_default'])} \\")
    print(f"  --rollout-weak {int(best_row['rollout_weak'])} \\")
    print(f"  --weight-cnn {best_row['weight_cnn']:.3f} \\")
    print(f"  --weight-rollout {best_row['weight_rollout']:.3f} \\")
    print(f"  --weight-heuristic {best_row['weight_heuristic']:.3f} \\")
    print(f"  --weight-contextual {best_row['weight_contextual']:.3f}")

    # Analyze impact of individual parameters (if enough variance)
    print("\n" + "=" * 80)
    print("🔍 PARAMETER IMPACT ANALYSIS")
    print("=" * 80)

    # Weight parameters analysis
    if 'weight_cnn' in df.columns:
        weight_params = ['weight_cnn', 'weight_rollout', 'weight_heuristic', 'weight_contextual']
        print("\nEvaluation Weights Impact:")
        for param in weight_params:
            if df[param].nunique() > 1:
                correlation = df[[param, 'avg_score']].corr().iloc[0, 1]
                print(f"  {param:20s}: correlation = {correlation:+.3f}")

    # c_puct analysis
    if 'c_puct_early' in df.columns:
        cpuct_params = ['c_puct_early', 'c_puct_mid', 'c_puct_late']
        if any(df[p].nunique() > 1 for p in cpuct_params):
            print("\nc_puct Impact:")
            for param in cpuct_params:
                if df[param].nunique() > 1:
                    correlation = df[[param, 'avg_score']].corr().iloc[0, 1]
                    print(f"  {param:20s}: correlation = {correlation:+.3f}")

    # Pruning analysis
    if 'prune_early' in df.columns:
        prune_params = ['prune_early', 'prune_mid1', 'prune_mid2', 'prune_late']
        if any(df[p].nunique() > 1 for p in prune_params):
            print("\nPruning Ratio Impact:")
            for param in prune_params:
                if df[param].nunique() > 1:
                    correlation = df[[param, 'avg_score']].corr().iloc[0, 1]
                    print(f"  {param:20s}: correlation = {correlation:+.3f}")

    # Rollout analysis
    if 'rollout_default' in df.columns:
        rollout_params = ['rollout_strong', 'rollout_medium', 'rollout_default', 'rollout_weak']
        if any(df[p].nunique() > 1 for p in rollout_params):
            print("\nRollout Count Impact:")
            for param in rollout_params:
                if df[param].nunique() > 1:
                    correlation = df[[param, 'avg_score']].corr().iloc[0, 1]
                    print(f"  {param:20s}: correlation = {correlation:+.3f}")

    print("\n" + "=" * 80)
    print("✅ Analysis complete!")
    print("=" * 80)


if __name__ == "__main__":
    main()
