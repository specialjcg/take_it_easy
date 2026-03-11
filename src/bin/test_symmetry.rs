//! Test hexagonal symmetry augmentation on TSV data.
//!
//! The Take It Easy hex board has D3 symmetry (3 rotations × 2 mirrors = 6 transforms).
//! Each transform permutes positions and tile face values while preserving the score.
//!
//! Board layout:
//!     0  1  2
//!    3  4  5  6
//!   7  8  9 10 11
//!    12 13 14 15
//!      16 17 18

use std::collections::HashSet;
use std::io::{BufRead, BufReader};

use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::tile::Tile;
use take_it_easy::scoring::scoring::result;

// Hex axial coordinates for each position (q, r)
const HEX_COORDS: [(i32, i32); 19] = [
    (0, -2), (1, -2), (2, -2),           // row 0: pos 0-2
    (-1, -1), (0, -1), (1, -1), (2, -1), // row 1: pos 3-6
    (-2, 0), (-1, 0), (0, 0), (1, 0), (2, 0), // row 2: pos 7-11
    (-2, 1), (-1, 1), (0, 1), (1, 1),    // row 3: pos 12-15
    (-2, 2), (-1, 2), (0, 2),            // row 4: pos 16-18
];

fn coord_to_pos(q: i32, r: i32) -> Option<usize> {
    HEX_COORDS.iter().position(|&(cq, cr)| cq == q && cr == r)
}

/// 120° clockwise rotation: (q, r) → (-r, q+r)
/// Direction mapping: dir0→dir2, dir1→dir0, dir2→dir1
/// Tile: (a,b,c) → (b,c,a)
fn rotation_120() -> ([usize; 19], fn(i32, i32, i32) -> (i32, i32, i32)) {
    let mut perm = [0usize; 19];
    for i in 0..19 {
        let (q, r) = HEX_COORDS[i];
        let new_q = -r;
        let new_r = q + r;
        perm[i] = coord_to_pos(new_q, new_r).expect(&format!("rot120 failed for pos {}", i));
    }
    (perm, |a, b, c| (b, c, a))
}

/// 240° clockwise rotation: apply 120° twice
/// Tile: (a,b,c) → (c,a,b)
fn rotation_240() -> ([usize; 19], fn(i32, i32, i32) -> (i32, i32, i32)) {
    let mut perm = [0usize; 19];
    for i in 0..19 {
        let (q, r) = HEX_COORDS[i];
        // Apply 120° twice: (q,r) → (-r, q+r) → (-(q+r), -r+q+r) = (-q-r, q)
        let new_q = -q - r;
        let new_r = q;
        perm[i] = coord_to_pos(new_q, new_r).expect(&format!("rot240 failed for pos {}", i));
    }
    (perm, |a, b, c| (c, a, b))
}

/// Reflection across q-axis: (q, r) → (q, -q-r)
/// Swaps dir0 ↔ dir1, keeps dir2
/// Tile: (a,b,c) → (b,a,c)
fn reflection_a() -> ([usize; 19], fn(i32, i32, i32) -> (i32, i32, i32)) {
    let mut perm = [0usize; 19];
    for i in 0..19 {
        let (q, r) = HEX_COORDS[i];
        let new_q = q;
        let new_r = -q - r;
        perm[i] = coord_to_pos(new_q, new_r).expect(&format!("refA failed for pos {}", i));
    }
    (perm, |a, b, c| (b, a, c))
}

/// Reflection B = Rot120 ∘ RefA
/// Tile: (a,b,c) → (a,c,b) [swap dir1↔dir2]
fn reflection_b() -> ([usize; 19], fn(i32, i32, i32) -> (i32, i32, i32)) {
    let (rot, _) = rotation_120();
    let (refa, _) = reflection_a();
    let mut perm = [0usize; 19];
    for i in 0..19 {
        perm[i] = rot[refa[i]]; // first refA, then rot120
    }
    (perm, |a, b, c| (a, c, b))
}

/// Reflection C = Rot240 ∘ RefA
/// Tile: (a,b,c) → (c,b,a) [swap dir0↔dir2]
fn reflection_c() -> ([usize; 19], fn(i32, i32, i32) -> (i32, i32, i32)) {
    let (rot, _) = rotation_240();
    let (refa, _) = reflection_a();
    let mut perm = [0usize; 19];
    for i in 0..19 {
        perm[i] = rot[refa[i]]; // first refA, then rot240
    }
    (perm, |a, b, c| (c, b, a))
}

type TileTransform = fn(i32, i32, i32) -> (i32, i32, i32);

fn get_all_symmetries() -> Vec<(&'static str, [usize; 19], TileTransform)> {
    let (r120, t120) = rotation_120();
    let (r240, t240) = rotation_240();
    let (ra, ta) = reflection_a();
    let (rb, tb) = reflection_b();
    let (rc, tc) = reflection_c();
    vec![
        ("rot120", r120, t120),
        ("rot240", r240, t240),
        ("reflA", ra, ta),
        ("reflB", rb, tb),
        ("reflC", rc, tc),
    ]
}

fn decode_tile(encoded: i32) -> Tile {
    if encoded == 0 {
        Tile(0, 0, 0)
    } else {
        Tile(encoded / 100, (encoded / 10) % 10, encoded % 10)
    }
}

fn encode_tile(t: &Tile) -> i32 {
    if *t == Tile(0, 0, 0) { 0 } else { t.0 * 100 + t.1 * 10 + t.2 }
}

/// Apply a symmetry transform to a plateau and return the new plateau
fn transform_plateau(
    plateau: &Plateau,
    pos_perm: &[usize; 19],
    tile_fn: TileTransform,
) -> Plateau {
    let mut new_tiles = vec![Tile(0, 0, 0); 19];
    for i in 0..19 {
        let t = &plateau.tiles[i];
        if *t != Tile(0, 0, 0) {
            let (a, b, c) = tile_fn(t.0, t.1, t.2);
            new_tiles[pos_perm[i]] = Tile(a, b, c);
        }
    }
    Plateau { tiles: new_tiles }
}

/// Apply symmetry to a TSV sample (plateau_encoded, tile, position)
fn transform_sample(
    plateau: &[i32; 19],
    tile: (i32, i32, i32),
    position: usize,
    pos_perm: &[usize; 19],
    tile_fn: TileTransform,
) -> ([i32; 19], (i32, i32, i32), usize) {
    let mut new_plateau = [0i32; 19];
    for i in 0..19 {
        if plateau[i] != 0 {
            let t = decode_tile(plateau[i]);
            let (a, b, c) = tile_fn(t.0, t.1, t.2);
            new_plateau[pos_perm[i]] = encode_tile(&Tile(a, b, c));
        }
    }
    let new_tile = tile_fn(tile.0, tile.1, tile.2);
    let new_pos = pos_perm[position];
    (new_plateau, new_tile, new_pos)
}

fn main() {
    // First: verify symmetries preserve scoring on complete boards
    println!("=== Test 1: Verify symmetries preserve scores ===\n");

    let symmetries = get_all_symmetries();

    // Print position permutations
    for (name, perm, _) in &symmetries {
        println!("{}: {:?}", name, perm);
    }
    println!();

    // Read TSV file (use gt_direct_200k if available, or expectimax)
    let tsv_paths = [
        "data/expectimax_2ply_10k.tsv",
        "data/gt_direct_200k.tsv",
    ];

    for tsv_path in &tsv_paths {
        let file = match std::fs::File::open(tsv_path) {
            Ok(f) => f,
            Err(_) => { println!("Skipping {} (not found)", tsv_path); continue; }
        };
        let reader = BufReader::new(file);

        println!("\n=== Analyzing {} ===\n", tsv_path);

        let mut samples: Vec<([i32; 19], (i32, i32, i32), usize, usize, i32, f64)> = Vec::new();

        for line in reader.lines() {
            let line = line.unwrap();
            if line.starts_with('#') || line.is_empty() { continue; }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 6 { continue; }

            let plat_vals: Vec<i32> = parts[0].split(',').map(|v| v.parse().unwrap()).collect();
            let mut plateau = [0i32; 19];
            for (i, &v) in plat_vals.iter().enumerate().take(19) {
                plateau[i] = v;
            }

            let tile_vals: Vec<i32> = parts[1].split(',').map(|v| v.parse().unwrap()).collect();
            let tile = (tile_vals[0], tile_vals[1], tile_vals[2]);
            let position: usize = parts[2].parse().unwrap();
            let turn: usize = parts[3].parse().unwrap();
            let score: i32 = parts[4].parse().unwrap();
            let weight: f64 = parts[5].parse().unwrap();

            samples.push((plateau, tile, position, turn, score, weight));
        }

        println!("Loaded {} samples", samples.len());

        // Score distribution
        let mut scores: Vec<i32> = samples.iter().map(|s| s.4).collect();
        scores.sort();
        scores.dedup();
        let total_games_approx = samples.len() / 19; // rough estimate

        // Verify scoring on a few complete game end-states
        // Find samples at turn 18 (last turn) - these have full boards
        let last_turn_samples: Vec<_> = samples.iter().filter(|s| s.3 == 18).collect();
        println!("Samples at turn 18 (near-complete boards): {}", last_turn_samples.len());

        // Test score preservation on first 100 near-complete boards
        let mut score_mismatches = 0;
        let test_count = last_turn_samples.len().min(500);

        for sample in last_turn_samples.iter().take(test_count) {
            // Reconstruct plateau (place the tile at position to get complete board)
            let mut plat = Plateau { tiles: vec![Tile(0, 0, 0); 19] };
            for i in 0..19 {
                plat.tiles[i] = decode_tile(sample.0[i]);
            }
            let tile = Tile(sample.1.0, sample.1.1, sample.1.2);
            plat.tiles[sample.2] = tile;

            let original_score = result(&plat);

            for (name, perm, tile_fn) in &symmetries {
                let transformed = transform_plateau(&plat, perm, *tile_fn);
                let transformed_score = result(&transformed);
                if original_score != transformed_score {
                    score_mismatches += 1;
                    println!("MISMATCH! {} original={} transformed={} ({})",
                        name, original_score, transformed_score, name);
                    // Print original and transformed
                    for i in 0..19 {
                        if plat.tiles[i] != Tile(0, 0, 0) {
                            print!("  pos{}: ({},{},{}) → pos{}: ({},{},{})  ",
                                i, plat.tiles[i].0, plat.tiles[i].1, plat.tiles[i].2,
                                perm[i], transformed.tiles[perm[i]].0,
                                transformed.tiles[perm[i]].1, transformed.tiles[perm[i]].2);
                        }
                    }
                    println!();
                    break; // only show first mismatch per sample
                }
            }
        }
        println!("\nScore verification: {}/{} boards × 5 transforms = {} checks, {} mismatches",
            test_count, test_count, test_count * 5, score_mismatches);

        // Test 2: Count unique samples before/after augmentation
        println!("\n=== Test 2: Unique samples with augmentation ===\n");

        // Sample a subset for speed
        let subset_size = samples.len().min(100_000);
        let subset = &samples[..subset_size];

        let mut original_keys: HashSet<String> = HashSet::new();
        let mut augmented_keys: HashSet<String> = HashSet::new();

        for s in subset {
            let key = format!("{:?}|{},{},{}|{}", &s.0, s.1.0, s.1.1, s.1.2, s.2);
            original_keys.insert(key.clone());
            augmented_keys.insert(key);

            for (_name, perm, tile_fn) in &symmetries {
                let (new_plat, new_tile, new_pos) = transform_sample(
                    &s.0, s.1, s.2, perm, *tile_fn
                );
                let aug_key = format!("{:?}|{},{},{}|{}", &new_plat, new_tile.0, new_tile.1, new_tile.2, new_pos);
                augmented_keys.insert(aug_key);
            }
        }

        let augmentation_ratio = augmented_keys.len() as f64 / original_keys.len() as f64;
        println!("Subset: {} samples", subset_size);
        println!("Original unique: {}", original_keys.len());
        println!("After 6× augmentation: {} unique", augmented_keys.len());
        println!("Augmentation factor: {:.2}×", augmentation_ratio);

        // Test 3: Score distribution of augmented data
        println!("\n=== Test 3: Score distribution ===\n");
        let score_thresholds = [100, 120, 140, 150, 160, 170, 180, 200];
        println!("Score    | Original samples | After 6× augmentation (estimate)");
        println!("---------+------------------+----------------------------------");
        for &thresh in &score_thresholds {
            let count = subset.iter().filter(|s| s.4 >= thresh).count();
            let aug_count = count as f64 * augmentation_ratio;
            println!(">= {:3} pts | {:>8} ({:4.1}%) | ~{:.0}",
                thresh, count, count as f64 / subset_size as f64 * 100.0, aug_count);
        }
    }
}
