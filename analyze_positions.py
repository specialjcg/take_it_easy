#!/usr/bin/env python3
"""
Analyse la distribution des positions dans les données d'entraînement
pour comprendre pourquoi le policy network ne bouge pas.
"""

import sys
from collections import Counter
import math

def analyze_self_play_logs(log_file):
    """
    Parse les logs de self-play pour extraire quelles positions
    sont sélectionnées par MCTS.
    """
    positions_selected = []

    with open(log_file, 'r') as f:
        for line in f:
            # Cette ligne est un placeholder - il faudrait parser les vrais logs
            # pour extraire les positions sélectionnées par MCTS
            pass

    return positions_selected

def main():
    print("🔍 Analyse de la Distribution des Positions")
    print("=" * 60)

    # Pour l'instant, simulons une vérification simple
    # En pratique, il faudrait parser les données de self-play

    print("\n❓ Question clé: Pourquoi policy_loss = 2.9444 constant?")
    print("\nPolicy loss = 2.9444 = ln(19) signifie:")
    print("  → Le réseau prédit une distribution UNIFORME (1/19 pour chaque position)")
    print("  → Cross-entropy entre prédiction uniforme et target = ln(19)")
    print()
    print("Cela peut arriver si:")
    print()
    print("1. ❌ Les DONNÉES sont uniformes:")
    print("   - MCTS sélectionne chaque position ~également")
    print("   - Réseau uniforme → UCT uniforme → sélection uniforme → données uniformes")
    print("   - PROBLÈME CIRCULAIRE")
    print()
    print("2. ❌ Les GRADIENTS ne passent pas:")
    print("   - Architecture policy network bloque gradients")
    print("   - Optimizer mal configuré")
    print()
    print("3. ❌ Learning rate TROP BAS:")
    print("   - LR=0.01 insuffisant pour policy network")
    print("   - Value network bouge (LR ok) mais policy non")
    print()
    print("=" * 60)
    print("\n🔬 Tests Recommandés:")
    print()
    print("1. Test Distribution Self-Play:")
    print("   → Jouer 100 games et compter combien de fois chaque position est choisie")
    print("   → Si uniforme → problème circulaire")
    print()
    print("2. Test Gradients Policy Network:")
    print("   → Forward + backward sur batch synthétique")
    print("   → Vérifier que les poids changent")
    print()
    print("3. Test Learning Rate Policy:")
    print("   → Augmenter LR policy à 0.05 ou 0.1")
    print("   → Garder value_lr = 0.01")
    print()
    print("4. Test Données Synthétiques:")
    print("   → Créer données où position 9 (centre) apparaît 80% du temps")
    print("   → Entraîner policy network dessus")
    print("   → Si loss descend → réseau OK, problème = données")
    print()

if __name__ == "__main__":
    main()
