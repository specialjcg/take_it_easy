//! Analyze Human vs AI move differences
//!
//! This script analyzes recorded games to understand:
//! 1. When and why humans made different choices than the AI
//! 2. What patterns emerge in human-winning games
//! 3. What the AI might be missing
//!
//! Usage: cargo run --release --bin analyze_human_vs_ai

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tch::{nn, Device, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::Plateau;
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::load_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;

/// Line definitions for scoring analysis
const LINES: [&[usize]; 15] = [
    // Horizontal lines (value from tile.0)
    &[0, 1, 2],
    &[3, 4, 5, 6],
    &[7, 8, 9, 10, 11],
    &[12, 13, 14, 15],
    &[16, 17, 18],
    // Diagonal 1 (value from tile.1) - top-left to bottom-right
    &[0, 3, 7],
    &[1, 4, 8, 12],
    &[2, 5, 9, 13, 16],
    &[6, 10, 14, 17],
    &[11, 15, 18],
    // Diagonal 2 (value from tile.2) - top-right to bottom-left
    &[2, 6, 11],
    &[1, 5, 10, 15],
    &[0, 4, 9, 14, 18],
    &[3, 8, 13, 17],
    &[7, 12, 16],
];

#[derive(Debug, Clone)]
struct GameMove {
    game_id: String,
    turn: usize,
    player_type: String,
    plateau: [i32; 19],
    tile: (i32, i32, i32),
    position: usize,
    final_score: i32,
    human_won: bool,
}

#[derive(Debug, Default)]
struct AnalysisStats {
    total_moves: usize,
    different_from_ai: usize,
    human_better_moves: usize,  // When human chose differently AND won
    ai_better_moves: usize,     // When human chose differently AND lost

    // Position analysis
    center_preference_human: usize,  // Human chose center more
    center_preference_ai: usize,     // AI chose center more

    // Turn-based differences
    early_game_diffs: usize,   // Turns 0-5
    mid_game_diffs: usize,     // Turns 6-12
    late_game_diffs: usize,    // Turns 13-18

    // Line completion analysis
    human_completes_line: usize,
    ai_completes_line: usize,
    human_blocks_line: usize,  // Human places tile that blocks a potential line
    ai_blocks_line: usize,
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Analyze Human vs AI Move Differences                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Load the Graph Transformer model
    let device = Device::Cpu;
    let mut vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(&vs, 47, 128, 2, 4, 0.1);

    let model_path = "model_weights/graph_transformer_policy.safetensors";
    if let Err(e) = load_varstore(&mut vs, model_path) {
        eprintln!("Failed to load model: {}", e);
        return;
    }
    println!("✅ Loaded Graph Transformer model\n");

    // Load recorded games
    let data_dir = "data/recorded_games";
    let moves = load_all_games(data_dir);
    println!("📂 Loaded {} moves from recorded games\n", moves.len());

    // Separate human and AI moves by game
    let mut games: HashMap<String, Vec<GameMove>> = HashMap::new();
    for m in moves {
        games.entry(m.game_id.clone()).or_default().push(m);
    }

    let mut stats = AnalysisStats::default();
    let mut human_win_games = Vec::new();
    let mut ai_win_games = Vec::new();

    // Analyze each game
    for (game_id, game_moves) in &games {
        let human_moves: Vec<_> = game_moves.iter().filter(|m| m.player_type == "Human").collect();
        let ai_moves: Vec<_> = game_moves.iter().filter(|m| m.player_type != "Human").collect();

        if human_moves.is_empty() || ai_moves.is_empty() {
            continue;
        }

        let human_won = human_moves[0].human_won;
        let human_score = human_moves[0].final_score;
        let ai_score = ai_moves[0].final_score;

        if human_won {
            human_win_games.push((game_id.clone(), human_score, ai_score));
        } else {
            ai_win_games.push((game_id.clone(), human_score, ai_score));
        }

        // Compare moves at each turn
        for human_move in &human_moves {
            stats.total_moves += 1;

            // Get what AI would predict for this position
            let ai_prediction = get_ai_prediction(&policy_net, human_move);

            if human_move.position != ai_prediction {
                stats.different_from_ai += 1;

                if human_won {
                    stats.human_better_moves += 1;
                } else {
                    stats.ai_better_moves += 1;
                }

                // Turn-based analysis
                if human_move.turn <= 5 {
                    stats.early_game_diffs += 1;
                } else if human_move.turn <= 12 {
                    stats.mid_game_diffs += 1;
                } else {
                    stats.late_game_diffs += 1;
                }

                // Center preference (position 9 is center)
                let human_dist_to_center = position_distance_to_center(human_move.position);
                let ai_dist_to_center = position_distance_to_center(ai_prediction);
                if human_dist_to_center < ai_dist_to_center {
                    stats.center_preference_human += 1;
                } else if ai_dist_to_center < human_dist_to_center {
                    stats.center_preference_ai += 1;
                }

                // Line completion analysis
                let human_completes = would_complete_line(&human_move.plateau, human_move.position, human_move.tile);
                let ai_completes = would_complete_line(&human_move.plateau, ai_prediction, human_move.tile);
                if human_completes { stats.human_completes_line += 1; }
                if ai_completes { stats.ai_completes_line += 1; }

                let human_blocks = would_block_line(&human_move.plateau, human_move.position, human_move.tile);
                let ai_blocks = would_block_line(&human_move.plateau, ai_prediction, human_move.tile);
                if human_blocks { stats.human_blocks_line += 1; }
                if ai_blocks { stats.ai_blocks_line += 1; }
            }
        }
    }

    // Print results
    println!("═══════════════════════════════════════════════════════════════");
    println!("                    ANALYSIS RESULTS");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("📊 Overall Statistics:");
    println!("   Total human moves analyzed: {}", stats.total_moves);
    println!("   Moves different from AI prediction: {} ({:.1}%)",
        stats.different_from_ai,
        stats.different_from_ai as f64 / stats.total_moves as f64 * 100.0);
    println!();

    println!("🏆 When Human Chose Differently:");
    println!("   Human won (better choice): {} ({:.1}%)",
        stats.human_better_moves,
        stats.human_better_moves as f64 / stats.different_from_ai.max(1) as f64 * 100.0);
    println!("   AI won (AI was right): {} ({:.1}%)",
        stats.ai_better_moves,
        stats.ai_better_moves as f64 / stats.different_from_ai.max(1) as f64 * 100.0);
    println!();

    println!("⏱️ When Do Differences Occur:");
    println!("   Early game (turns 0-5):  {} ({:.1}%)",
        stats.early_game_diffs,
        stats.early_game_diffs as f64 / stats.different_from_ai.max(1) as f64 * 100.0);
    println!("   Mid game (turns 6-12):   {} ({:.1}%)",
        stats.mid_game_diffs,
        stats.mid_game_diffs as f64 / stats.different_from_ai.max(1) as f64 * 100.0);
    println!("   Late game (turns 13-18): {} ({:.1}%)",
        stats.late_game_diffs,
        stats.late_game_diffs as f64 / stats.different_from_ai.max(1) as f64 * 100.0);
    println!();

    println!("🎯 Position Strategy:");
    println!("   Human prefers center more: {}", stats.center_preference_human);
    println!("   AI prefers center more:    {}", stats.center_preference_ai);
    println!();

    println!("📏 Line Completion (when moves differ):");
    println!("   Human completes a line: {}", stats.human_completes_line);
    println!("   AI would complete a line: {}", stats.ai_completes_line);
    println!("   Human blocks a potential line: {}", stats.human_blocks_line);
    println!("   AI would block a potential line: {}", stats.ai_blocks_line);
    println!();

    println!("═══════════════════════════════════════════════════════════════");
    println!("                    GAME OUTCOMES");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("🎮 Human Victories ({} games):", human_win_games.len());
    for (id, h_score, a_score) in human_win_games.iter().take(5) {
        println!("   {} : Human {} vs AI {}", &id[..8], h_score, a_score);
    }
    if human_win_games.len() > 5 {
        println!("   ... and {} more", human_win_games.len() - 5);
    }
    println!();

    println!("🤖 AI Victories ({} games):", ai_win_games.len());
    for (id, h_score, a_score) in ai_win_games.iter().take(5) {
        println!("   {} : Human {} vs AI {}", &id[..8], h_score, a_score);
    }
    if ai_win_games.len() > 5 {
        println!("   ... and {} more", ai_win_games.len() - 5);
    }
    println!();

    // Detailed analysis of human wins
    println!("═══════════════════════════════════════════════════════════════");
    println!("                    KEY INSIGHTS");
    println!("═══════════════════════════════════════════════════════════════\n");

    if stats.human_better_moves > stats.ai_better_moves {
        println!("✨ Humans make better choices {:.1}% of the time when they disagree with AI!",
            stats.human_better_moves as f64 / stats.different_from_ai.max(1) as f64 * 100.0);
    }

    if stats.early_game_diffs > stats.late_game_diffs {
        println!("🔍 Most disagreements happen in early game - AI might be too conservative early on");
    } else if stats.late_game_diffs > stats.early_game_diffs {
        println!("🔍 Most disagreements happen in late game - AI might miss tactical opportunities");
    }

    if stats.center_preference_human > stats.center_preference_ai {
        println!("🎯 Humans prefer central positions more - consider increasing center position value");
    }

    if stats.human_completes_line > stats.ai_completes_line {
        println!("📏 Humans prioritize line completion more - AI might be too focused on flexibility");
    }

    if stats.human_blocks_line < stats.ai_blocks_line {
        println!("🛡️ Humans avoid blocking potential lines - AI might be too defensive");
    }
}

fn get_ai_prediction(policy_net: &GraphTransformerPolicyNet, game_move: &GameMove) -> usize {
    let deck = create_deck();

    // Convert plateau from encoded format
    let mut plateau = Plateau { tiles: vec![Tile(0, 0, 0); 19] };
    for i in 0..19 {
        let v = game_move.plateau[i];
        if v > 0 {
            let v1 = (v / 100) as i32;
            let v2 = ((v / 10) % 10) as i32;
            let v3 = (v % 10) as i32;
            plateau.tiles[i] = Tile(v1, v2, v3);
        }
    }

    let tile = Tile(game_move.tile.0, game_move.tile.1, game_move.tile.2);

    // Get AI prediction
    let feat = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, game_move.turn, 19).unsqueeze(0);
    let logits = policy_net.forward(&feat, false).squeeze_dim(0);

    // Mask occupied positions
    let mut mask = [0.0f32; 19];
    for i in 0..19 {
        if game_move.plateau[i] != 0 {
            mask[i] = f32::NEG_INFINITY;
        }
    }
    let mask_tensor = Tensor::from_slice(&mask);
    let masked = logits + mask_tensor;

    masked.argmax(-1, false).int64_value(&[]) as usize
}

fn position_distance_to_center(pos: usize) -> usize {
    // Distance from position 9 (center of hex)
    const DISTANCES: [usize; 19] = [
        2, 2, 2,     // Row 0
        2, 1, 1, 2,  // Row 1
        2, 1, 0, 1, 2, // Row 2 (center)
        2, 1, 1, 2,  // Row 3
        2, 2, 2,     // Row 4
    ];
    DISTANCES[pos]
}

fn would_complete_line(plateau: &[i32; 19], position: usize, tile: (i32, i32, i32)) -> bool {
    for (line_idx, line) in LINES.iter().enumerate() {
        if !line.contains(&position) {
            continue;
        }

        // Get the value for this line direction
        let tile_value = match line_idx {
            0..=4 => tile.0,   // Horizontal
            5..=9 => tile.1,   // Diagonal 1
            _ => tile.2,      // Diagonal 2
        };

        // Check if placing this tile would complete the line
        let mut all_same = true;
        let mut count_filled = 0;

        for &pos in *line {
            if pos == position {
                count_filled += 1;
                continue;
            }

            let v = plateau[pos];
            if v == 0 {
                all_same = false;
                break;
            }

            // Decode the value for this direction
            let pos_value = match line_idx {
                0..=4 => (v / 100) as i32,
                5..=9 => ((v / 10) % 10) as i32,
                _ => (v % 10) as i32,
            };

            if pos_value != tile_value {
                all_same = false;
                break;
            }
            count_filled += 1;
        }

        if all_same && count_filled == line.len() {
            return true;
        }
    }
    false
}

fn would_block_line(plateau: &[i32; 19], position: usize, tile: (i32, i32, i32)) -> bool {
    for (line_idx, line) in LINES.iter().enumerate() {
        if !line.contains(&position) {
            continue;
        }

        // Get the value for this line direction
        let tile_value = match line_idx {
            0..=4 => tile.0,
            5..=9 => tile.1,
            _ => tile.2,
        };

        // Check if there are existing tiles with different values
        let mut has_different = false;
        let mut has_same = false;

        for &pos in *line {
            if pos == position {
                continue;
            }

            let v = plateau[pos];
            if v == 0 {
                continue;
            }

            let pos_value = match line_idx {
                0..=4 => (v / 100) as i32,
                5..=9 => ((v / 10) % 10) as i32,
                _ => (v % 10) as i32,
            };

            if pos_value == tile_value {
                has_same = true;
            } else {
                has_different = true;
            }
        }

        // Blocking if there were same values but we're adding a different one
        if has_same && !has_different && tile_value != 0 {
            // Actually check if our tile differs
            for &pos in *line {
                if pos == position { continue; }
                let v = plateau[pos];
                if v == 0 { continue; }

                let pos_value = match line_idx {
                    0..=4 => (v / 100) as i32,
                    5..=9 => ((v / 10) % 10) as i32,
                    _ => (v % 10) as i32,
                };

                if pos_value != tile_value {
                    return true;
                }
            }
        }
    }
    false
}

fn load_all_games(dir: &str) -> Vec<GameMove> {
    let mut moves = Vec::new();
    let path = Path::new(dir);

    if !path.exists() {
        return moves;
    }

    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let file_path = entry.path();
        if file_path.extension().map_or(false, |e| e == "csv") {
            moves.extend(load_csv(&file_path));
        }
    }

    moves
}

fn load_csv(path: &Path) -> Vec<GameMove> {
    let mut moves = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return moves,
    };

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Skip header
    let _ = lines.next();

    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 28 {
            continue;
        }

        let game_id = parts[0].to_string();
        let turn: usize = parts[1].parse().unwrap_or(0);
        let player_type = parts[2].to_string();

        let mut plateau = [0i32; 19];
        for i in 0..19 {
            plateau[i] = parts[3 + i].parse().unwrap_or(0);
        }

        let tile = (
            parts[22].parse().unwrap_or(0),
            parts[23].parse().unwrap_or(0),
            parts[24].parse().unwrap_or(0),
        );

        let position: usize = parts[25].parse().unwrap_or(0);
        let final_score: i32 = parts[26].parse().unwrap_or(0);
        let human_won: bool = parts[27].parse::<i32>().unwrap_or(0) == 1;

        moves.push(GameMove {
            game_id,
            turn,
            player_type,
            plateau,
            tile,
            position,
            final_score,
            human_won,
        });
    }

    moves
}
