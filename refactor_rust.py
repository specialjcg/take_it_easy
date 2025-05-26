#!/usr/bin/env python3
"""
Script de refactoring automatique pour le code Rust Take It Easy
Extrait les fonctions du main.rs vers des modules thématiques
"""

import os
import re
import shutil
from pathlib import Path

def extract_function(content, function_name):
    """Extrait une fonction complète avec sa documentation et ses attributs"""
    # Pattern pour capturer fonction avec doc comments et attributs
    pattern = rf'((?:\/\/\/.*\n)*(?:#\[.*\]\n)*(?:pub\s+)?(?:async\s+)?fn\s+{function_name}\s*(?:<[^>]*>)?\s*\([^)]*\)(?:\s*->\s*[^{{]+)?\s*\{{)'

    match = re.search(pattern, content, re.MULTILINE | re.DOTALL)
    if not match:
        return None, content

    start = match.start()

    # Trouver la fin de la fonction en comptant les accolades
    brace_count = 0
    i = content.find('{', start)
    if i == -1:
        return None, content

    brace_count = 1
    i += 1

    while i < len(content) and brace_count > 0:
        if content[i] == '{':
            brace_count += 1
        elif content[i] == '}':
            brace_count -= 1
        i += 1

    if brace_count == 0:
        function_code = content[start:i]
        remaining_content = content[:start] + content[i:]
        return function_code, remaining_content

    return None, content

def extract_struct_with_impl(content, struct_name):
    """Extrait une struct avec ses implémentations"""
    results = []

    # Extraire la struct
    struct_pattern = rf'((?:\/\/\/.*\n)*(?:#\[.*\]\n)*(?:pub\s+)?struct\s+{struct_name}\s*(?:<[^>]*>)?\s*\{{[^}}]*\}})'
    struct_match = re.search(struct_pattern, content, re.MULTILINE | re.DOTALL)
    if struct_match:
        results.append(struct_match.group(1))
        content = content.replace(struct_match.group(1), '', 1)

    # Extraire les implémentations
    impl_pattern = rf'(impl(?:<[^>]*>)?\s+{struct_name}(?:<[^>]*>)?\s*\{{.*?\n\}})'
    impl_matches = re.finditer(impl_pattern, content, re.MULTILINE | re.DOTALL)

    for match in reversed(list(impl_matches)):
        results.append(match.group(1))
        content = content[:match.start()] + content[match.end():]

    return results, content

def create_module_content(module_name, extracted_items, imports=None):
    """Crée le contenu d'un module avec les imports appropriés"""
    content = f"// {module_name}.rs - Module pour {module_name}\n\n"

    # Imports par défaut
    default_imports = [
        "use crate::test::{Deck, Plateau, Tile};",
    ]

    # Imports spécifiques par module
    module_imports = {
        'neural': [
            "use tch::{nn, Tensor};",
            "use tch::nn::{Optimizer, OptimizerConfig};",
            "use std::collections::HashMap;",
            "use crate::policy_value_net::{PolicyNet, ValueNet};",
        ],
        'mcts': [
            "use std::collections::HashMap;",
            "use tch::Tensor;",
            "use rand::Rng;",
            "use crate::policy_value_net::{PolicyNet, ValueNet};",
        ],
        'scoring': [
            "use std::collections::HashMap;",
        ],
        'game': [
            "use rand::Rng;",
        ],
        'utils': [
            "use rand::Rng;",
        ]
    }

    # Ajouter les imports
    all_imports = default_imports.copy()
    if module_name in module_imports:
        all_imports.extend(module_imports[module_name])

    if imports:
        all_imports.extend(imports)

    for imp in sorted(set(all_imports)):
        content += imp + "\n"

    content += "\n"

    # Ajouter les éléments extraits
    for item in extracted_items:
        content += item + "\n\n"

    return content

def main():
    """Fonction principale de refactoring"""
    src_dir = Path("src")
    main_file = src_dir / "main.rs"

    if not main_file.exists():
        print("❌ Fichier main.rs non trouvé dans src/")
        return

    # Sauvegarde
    backup_file = src_dir / "main.rs.backup"
    shutil.copy2(main_file, backup_file)
    print(f"✅ Sauvegarde créée: {backup_file}")

    # Lire le contenu
    with open(main_file, 'r', encoding='utf-8') as f:
        content = f.read()

    # Définir les fonctions à extraire par module
    modules = {
        'game': {
            'functions': [
                'create_plateau_empty',
                'create_shuffle_deck',
                'is_plateau_full',
                'get_legal_moves',
                'generate_tile_image_names',
                'is_game_over',
                'apply_move',
                'placer_tile',
            ],
            'structs': []
        },
        'scoring': {
            'functions': [
                'result',
                'calculate_line_completion_bonus',
                'enhanced_position_evaluation',
                'compute_alignment_score',
                'compute_potential_scores',
            ],
            'structs': []
        },
        'mcts': {
            'functions': [
                'mcts_find_best_position_for_tile_with_nn',
                'simulate_games',
                'local_lookahead',
            ],
            'structs': ['MCTSResult']
        },
        'neural': {
            'functions': [
                'convert_plateau_to_tensor',
                'enhanced_gradient_clipping',
                'robust_state_normalization',
                'train_network_with_game_data',
                'compute_global_stats',
                'normalize_input',
                'calculate_n_step_returns',
                'huber_loss',
                'calculate_prediction_accuracy',
            ],
            'structs': []
        },
        'utils': {
            'functions': [
                'random_index',
                'load_game_data',
                'save_game_data',
                'serialize_tensor',
                'tensor_to_vec',
                'deserialize_game_data',
                'append_to_results_file',
            ],
            'structs': []
        }
    }

    # Extraire les éléments pour chaque module
    for module_name, items in modules.items():
        extracted_items = []

        # Extraire les fonctions
        for func_name in items['functions']:
            func_code, content = extract_function(content, func_name)
            if func_code:
                extracted_items.append(func_code)
                print(f"✅ Fonction {func_name} extraite pour {module_name}")
            else:
                print(f"⚠️  Fonction {func_name} non trouvée")

        # Extraire les structs
        for struct_name in items['structs']:
            struct_items, content = extract_struct_with_impl(content, struct_name)
            extracted_items.extend(struct_items)
            if struct_items:
                print(f"✅ Struct {struct_name} extraite pour {module_name}")

        # Créer le fichier module
        if extracted_items:
            module_file = src_dir / f"{module_name}.rs"
            module_content = create_module_content(module_name, extracted_items)

            with open(module_file, 'w', encoding='utf-8') as f:
                f.write(module_content)

            print(f"✅ Module {module_name}.rs créé avec {len(extracted_items)} éléments")

    # Mettre à jour main.rs avec les déclarations de modules
    module_declarations = "\n".join([
        f"mod {module};" for module in modules.keys()
    ])

    module_uses = "\n".join([
        f"use {module}::*;" for module in modules.keys()
    ])

    # Insérer les déclarations après les imports existants
    import_section_end = content.find("mod test;")
    if import_section_end != -1:
        # Trouver la fin de la ligne
        line_end = content.find('\n', import_section_end)
        if line_end != -1:
            content = (content[:line_end + 1] +
                      f"\n{module_declarations}\n\n{module_uses}\n" +
                      content[line_end + 1:])
    else:
        # Ajouter au début après les uses existants
        content = f"{module_declarations}\n\n{module_uses}\n\n" + content

    # Sauvegarder le main.rs modifié
    with open(main_file, 'w', encoding='utf-8') as f:
        f.write(content)

    print(f"✅ main.rs mis à jour avec les déclarations de modules")

    # Vérification finale
    print("\n🎯 Refactoring terminé!")
    print("📋 Fichiers créés:")
    for module in modules.keys():
        module_file = src_dir / f"{module}.rs"
        if module_file.exists():
            print(f"   ✅ {module_file}")

    print(f"\n🔧 Pour vérifier: cargo check")
    print(f"📁 Sauvegarde: {backup_file}")

if __name__ == "__main__":
    main()