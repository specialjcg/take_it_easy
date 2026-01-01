use std::collections::BTreeSet;
use std::env;
use std::path::Path;

use tch::{Kind, Tensor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let default_files = [
        "game_data_states.pt",
        "game_data_positions.pt",
        "game_data_subscores.pt",
        "game_data_policy.pt",
        "game_data_policy_raw.pt",
        "game_data_policy_boosted.pt",
        "game_data_boosts.pt",
    ];

    let args: Vec<String> = env::args().skip(1).collect();
    let targets: Vec<String> = if args.is_empty() {
        default_files
            .iter()
            .filter(|p| Path::new(*p).exists())
            .map(|s| s.to_string())
            .collect()
    } else {
        args
    };

    if targets.is_empty() {
        println!("Aucun fichier .pt trouvé (passez les chemins en argument si besoin).");
        return Ok(());
    }

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║           Inspection des données d'entraînement           ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    for path in targets {
        println!("📁 Fichier: {}", path);
        if !Path::new(&path).exists() {
            println!("  ❌ Fichier introuvable\n");
            continue;
        }

        // Taille du fichier
        if let Ok(metadata) = std::fs::metadata(&path) {
            let size_kb = metadata.len() as f64 / 1024.0;
            if size_kb < 1024.0 {
                println!("  📦 Taille: {:.2} KB", size_kb);
            } else {
                println!("  📦 Taille: {:.2} MB", size_kb / 1024.0);
            }
        }

        let tensor = match Tensor::load(&path) {
            Ok(t) => t,
            Err(err) => {
                println!("  ❌ Impossible de charger: {}\n", err);
                continue;
            }
        };

        let shape = tensor
            .size()
            .into_iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(" × ");
        let num_samples = tensor.size()[0];
        println!("  📊 Shape: [{}]", shape);
        println!("  🎯 Type: {:?}", tensor.kind());
        println!("  📈 Nombre d'exemples: {}", num_samples);

        // Calculer la taille totale
        let total_elements = tensor.numel();
        println!("  🔢 Éléments totaux: {}", total_elements);

        if tensor.numel() == 0 {
            println!("  ⚠️  Tenseur vide\n");
            continue;
        }

        // Statistiques globales
        println!("\n  📊 Statistiques globales:");
        let (min_val, max_val, mean_val) = sample_stats(&tensor);
        println!("     Min: {:.4}", min_val);
        println!("     Max: {:.4}", max_val);
        println!("     Mean: {:.4}", mean_val);

        // Échantillons
        let samples = tensor.size()[0].min(3);
        if samples > 0 {
            println!("\n  🔍 Aperçu des {} premiers exemples:", samples);
            for idx in 0..samples {
                let sample = tensor.get(idx);
                let (s_min, s_max, s_mean) = sample_stats(&sample);

                if idx == 0 {
                    let sample_shape = sample
                        .size()
                        .into_iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join(" × ");
                    println!("     Shape par exemple: [{}]", sample_shape);
                }

                println!(
                    "     Exemple {}: min={:.4}, max={:.4}, mean={:.4}",
                    idx + 1,
                    s_min,
                    s_max,
                    s_mean
                );

                if is_positions_tensor(&tensor) && idx == 0 {
                    let uniques = unique_positions(&sample);
                    println!("       Positions uniques: {:?}", uniques);
                }
            }
        }

        println!();
    }

    Ok(())
}

fn sample_stats(tensor: &Tensor) -> (f64, f64, f64) {
    let min_val = tensor.min().double_value(&[]);
    let max_val = tensor.max().double_value(&[]);
    let mean_val = tensor.mean(Kind::Float).double_value(&[]);
    (min_val, max_val, mean_val)
}

fn unique_positions(sample: &Tensor) -> Vec<i64> {
    let flattened = sample.to_kind(Kind::Int64).flatten(0, -1);
    let numel = flattened.numel();
    let mut buffer = vec![0i64; numel];
    flattened.copy_data(&mut buffer, numel);
    let mut set = BTreeSet::new();
    for value in buffer {
        set.insert(value);
        if set.len() >= 20 {
            break;
        }
    }
    set.into_iter().collect()
}

fn is_positions_tensor(tensor: &Tensor) -> bool {
    matches!(tensor.kind(), Kind::Int | Kind::Int64)
}

#[allow(dead_code)]
fn is_subscore_tensor(tensor: &Tensor) -> bool {
    matches!(tensor.kind(), Kind::Float | Kind::Double)
}
