//! Benchmark Strategy Comparison for Take It Easy
//!
//! Replays recorded games with multiple strategies to quantify the gap
//! between GT Direct inference and search-based or heuristic approaches.
//!
//! Usage: cargo run --release --bin benchmark_strategies [-- --mcts-sims 150]

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::time::Instant;
use tch::{nn, Device, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::deck::Deck;
use take_it_easy::game::get_legal_moves::get_legal_moves;
use take_it_easy::game::plateau::{create_plateau_empty, Plateau};
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::mcts::algorithm::mcts_find_best_position_for_tile_pure;
use take_it_easy::neural::graph_transformer::GraphTransformerPolicyNet;
use take_it_easy::neural::model_io::load_varstore;
use take_it_easy::neural::tensor_conversion::convert_plateau_for_gat_47ch;
use take_it_easy::scoring::scoring::result;
use take_it_easy::strategy::position_evaluation::enhanced_position_evaluation;

#[derive(Parser)]
#[command(name = "benchmark_strategies", about = "Benchmark strategy comparison")]
struct Args {
    /// Directory containing recorded game CSVs
    #[arg(long, default_value = "data/recorded_games")]
    data_dir: String,

    /// Number of MCTS simulations
    #[arg(long, default_value_t = 150)]
    mcts_sims: usize,

    /// Number of additional random-tile-sequence games to play
    #[arg(long, default_value_t = 100)]
    random_games: usize,

    /// Path to GT policy model weights
    #[arg(long, default_value = "model_weights/graph_transformer_policy.safetensors")]
    model_path: String,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

/// Line definitions: (positions, direction_index)
/// direction 0 = tile.0 (horizontal), 1 = tile.1 (diag1), 2 = tile.2 (diag2)
const LINES: [(&[usize], usize); 15] = [
    (&[0, 1, 2], 0),
    (&[3, 4, 5, 6], 0),
    (&[7, 8, 9, 10, 11], 0),
    (&[12, 13, 14, 15], 0),
    (&[16, 17, 18], 0),
    (&[0, 3, 7], 1),
    (&[1, 4, 8, 12], 1),
    (&[2, 5, 9, 13, 16], 1),
    (&[6, 10, 14, 17], 1),
    (&[11, 15, 18], 1),
    (&[7, 12, 16], 2),
    (&[3, 8, 13, 17], 2),
    (&[0, 4, 9, 14, 18], 2),
    (&[1, 5, 10, 15], 2),
    (&[2, 6, 11], 2),
];

// ─── Data structures ──────────────────────────────────────────────

#[derive(Debug, Clone)]
struct RecordedGame {
    game_id: String,
    tile_sequence: Vec<Tile>,
    human_score: i32,
    ai_score: i32,
    human_won: bool,
}

#[derive(Debug, Clone)]
struct GameResult {
    game_id: String,
    gt_direct_score: i32,
    mcts_score: i32,
    heuristic_score: i32,
    random_score: i32,
    human_score: i32,
    ai_recorded_score: i32,
}

#[derive(Debug, Default)]
struct LineCompletions {
    v1_cols: usize,   // horizontal (tile.0)
    v2_diags: usize,  // diagonal 1 (tile.1)
    v3_diags: usize,  // diagonal 2 (tile.2)
}

impl LineCompletions {
    fn total(&self) -> usize {
        self.v1_cols + self.v2_diags + self.v3_diags
    }
}

// ─── CSV loading ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CsvMove {
    game_id: String,
    turn: usize,
    player_type: String,
    tile: Tile,
    position: usize,
    final_score: i32,
    human_won: bool,
}

fn load_csv(path: &Path) -> Vec<CsvMove> {
    let mut moves = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return moves,
    };

    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let _ = lines.next(); // skip header

    for line in lines {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 28 {
            continue;
        }

        let tile_0: i32 = parts[22].parse().unwrap_or(0);
        let tile_1: i32 = parts[23].parse().unwrap_or(0);
        let tile_2: i32 = parts[24].parse().unwrap_or(0);

        moves.push(CsvMove {
            game_id: parts[0].to_string(),
            turn: parts[1].parse().unwrap_or(0),
            player_type: parts[2].to_string(),
            tile: Tile(tile_0, tile_1, tile_2),
            position: parts[25].parse().unwrap_or(0),
            final_score: parts[26].parse().unwrap_or(0),
            human_won: parts[27].parse::<i32>().unwrap_or(0) == 1,
        });
    }
    moves
}

fn load_all_games(dir: &str) -> Vec<RecordedGame> {
    let path = Path::new(dir);
    if !path.exists() {
        eprintln!("Data directory not found: {}", dir);
        return Vec::new();
    }

    let mut all_moves = Vec::new();
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let file_path = entry.path();
        if file_path.extension().map_or(false, |e| e == "csv") {
            let csv_moves = load_csv(&file_path);
            println!(
                "  {} : {} rows",
                file_path.file_name().unwrap().to_string_lossy(),
                csv_moves.len()
            );
            all_moves.extend(csv_moves);
        }
    }

    // Group by game_id
    let mut by_game: HashMap<String, Vec<CsvMove>> = HashMap::new();
    for m in all_moves {
        by_game.entry(m.game_id.clone()).or_default().push(m);
    }

    // Extract unique games with tile sequences
    let mut games = Vec::new();
    for (game_id, mut moves) in by_game {
        moves.sort_by_key(|m| (m.turn, m.player_type.clone()));

        // Get tile sequence from human moves (one tile per turn)
        let human_moves: Vec<&CsvMove> = moves
            .iter()
            .filter(|m| m.player_type == "Human")
            .collect();
        let ai_moves: Vec<&CsvMove> = moves
            .iter()
            .filter(|m| m.player_type != "Human")
            .collect();

        if human_moves.is_empty() {
            continue;
        }

        // Extract tile sequence sorted by turn
        let mut tile_seq: Vec<(usize, Tile)> = human_moves
            .iter()
            .map(|m| (m.turn, m.tile))
            .collect();
        tile_seq.sort_by_key(|(t, _)| *t);
        let tile_sequence: Vec<Tile> = tile_seq.into_iter().map(|(_, t)| t).collect();

        if tile_sequence.len() != 19 {
            continue; // incomplete game
        }

        let human_score = human_moves[0].final_score;
        let ai_score = if !ai_moves.is_empty() {
            ai_moves[0].final_score
        } else {
            0
        };
        let human_won = human_moves[0].human_won;

        games.push(RecordedGame {
            game_id,
            tile_sequence,
            human_score,
            ai_score,
            human_won,
        });
    }

    games.sort_by(|a, b| a.game_id.cmp(&b.game_id));
    games
}

// ─── Strategy implementations ─────────────────────────────────────

fn play_gt_direct(
    tiles: &[Tile],
    policy_net: &GraphTransformerPolicyNet,
) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        // Convert to tensor and get logits
        let feat =
            convert_plateau_for_gat_47ch(&plateau, tile, &deck, turn, 19).unsqueeze(0);
        let logits = policy_net.forward(&feat, false).squeeze_dim(0);

        // Mask occupied positions
        let mut mask = [0.0f32; 19];
        for i in 0..19 {
            if plateau.tiles[i] != Tile(0, 0, 0) {
                mask[i] = f32::NEG_INFINITY;
            }
        }
        let mask_tensor = Tensor::from_slice(&mask);
        let masked = logits + mask_tensor;

        let best_pos = masked.argmax(-1, false).int64_value(&[]) as usize;
        plateau.tiles[best_pos] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

fn play_pure_mcts(tiles: &[Tile], num_sims: usize) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();
    let mut deck = create_deck();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let mcts_result = mcts_find_best_position_for_tile_pure(
            &mut plateau,
            &mut deck,
            *tile,
            num_sims,
            turn,
            19,
            None,
        );

        plateau.tiles[mcts_result.best_position] = *tile;
        deck = replace_tile_in_deck(&deck, tile);
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

fn play_heuristic(tiles: &[Tile]) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();

    for (turn, tile) in tiles.iter().enumerate() {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }

        let best_pos = legal
            .iter()
            .copied()
            .max_by(|&a, &b| {
                let score_a = enhanced_position_evaluation(&plateau, a, tile, turn);
                let score_b = enhanced_position_evaluation(&plateau, b, tile, turn);
                score_a.partial_cmp(&score_b).unwrap()
            })
            .unwrap();

        plateau.tiles[best_pos] = *tile;
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

fn play_random(tiles: &[Tile], rng: &mut StdRng) -> (i32, LineCompletions) {
    let mut plateau = create_plateau_empty();

    for tile in tiles {
        let legal = get_legal_moves(&plateau);
        if legal.is_empty() {
            break;
        }
        let pos = *legal.choose(rng).unwrap();
        plateau.tiles[pos] = *tile;
    }

    let score = result(&plateau);
    let completions = count_line_completions(&plateau);
    (score, completions)
}

// ─── Line completion analysis ─────────────────────────────────────

fn count_line_completions(plateau: &Plateau) -> LineCompletions {
    let mut lc = LineCompletions::default();

    for &(positions, direction) in &LINES {
        let get_value = |tile: &Tile| match direction {
            0 => tile.0,
            1 => tile.1,
            _ => tile.2,
        };

        let first = &plateau.tiles[positions[0]];
        if *first == Tile(0, 0, 0) {
            continue;
        }
        let target = get_value(first);
        if target == 0 {
            continue;
        }

        let all_match = positions
            .iter()
            .all(|&i| {
                let t = &plateau.tiles[i];
                *t != Tile(0, 0, 0) && get_value(t) == target
            });

        if all_match {
            match direction {
                0 => lc.v1_cols += 1,
                1 => lc.v2_diags += 1,
                _ => lc.v3_diags += 1,
            }
        }
    }

    lc
}

// ─── Random game generation ───────────────────────────────────────

fn generate_random_tile_sequence(rng: &mut StdRng) -> Vec<Tile> {
    let mut deck = create_deck();
    let mut tiles = Vec::with_capacity(19);
    for _ in 0..19 {
        let available = get_available_tiles(&deck);
        if available.is_empty() {
            break;
        }
        let tile = *available.choose(rng).unwrap();
        tiles.push(tile);
        deck = replace_tile_in_deck(&deck, &tile);
    }
    tiles
}

// ─── Main ─────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();
    let mut rng = StdRng::seed_from_u64(args.seed);

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Strategy Benchmark — Take It Easy                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Load model
    let device = Device::Cpu;
    let mut vs = nn::VarStore::new(device);
    let policy_net = GraphTransformerPolicyNet::new(&vs, 47, 128, 2, 4, 0.1);

    print!("Loading GT model from {}... ", &args.model_path);
    if let Err(e) = load_varstore(&mut vs, &args.model_path) {
        eprintln!("FAILED: {}", e);
        return;
    }
    println!("OK");

    // Load recorded games
    println!("\nLoading recorded games from {}:", &args.data_dir);
    let recorded_games = load_all_games(&args.data_dir);
    println!("  Total: {} complete games\n", recorded_games.len());

    if recorded_games.is_empty() {
        eprintln!("No complete games found. Exiting.");
        return;
    }

    // ─── Benchmark on recorded games ──────────────────────────────

    println!(
        "Running benchmark on {} recorded games (MCTS: {} sims)...\n",
        recorded_games.len(),
        args.mcts_sims
    );

    let mut results: Vec<GameResult> = Vec::new();
    let mut gt_completions = Vec::new();
    let mut mcts_completions = Vec::new();
    let mut heur_completions = Vec::new();
    let mut rand_completions = Vec::new();

    let total = recorded_games.len();
    let start = Instant::now();

    for (i, game) in recorded_games.iter().enumerate() {
        let game_start = Instant::now();

        // GT Direct
        let (gt_score, gt_lc) = play_gt_direct(&game.tile_sequence, &policy_net);
        gt_completions.push(gt_lc);

        // Pure MCTS
        let (mcts_score, mcts_lc) = play_pure_mcts(&game.tile_sequence, args.mcts_sims);
        mcts_completions.push(mcts_lc);

        // Heuristic
        let (heur_score, heur_lc) = play_heuristic(&game.tile_sequence);
        heur_completions.push(heur_lc);

        // Random (average of 5 runs)
        let mut rand_total = 0;
        let mut rand_lc_best = LineCompletions::default();
        let n_rand = 5;
        for _ in 0..n_rand {
            let (rs, rlc) = play_random(&game.tile_sequence, &mut rng);
            rand_total += rs;
            if rlc.total() > rand_lc_best.total() {
                rand_lc_best = rlc;
            }
        }
        let rand_score = rand_total / n_rand as i32;
        rand_completions.push(rand_lc_best);

        results.push(GameResult {
            game_id: game.game_id.clone(),
            gt_direct_score: gt_score,
            mcts_score,
            heuristic_score: heur_score,
            random_score: rand_score,
            human_score: game.human_score,
            ai_recorded_score: game.ai_score,
        });

        let elapsed = game_start.elapsed();
        print!(
            "\r  [{}/{}] Game {} : GT={}, MCTS={}, Heur={}, Rand={}, Human={} ({:.1}s)",
            i + 1,
            total,
            &game.game_id[..8],
            gt_score,
            mcts_score,
            heur_score,
            rand_score,
            game.human_score,
            elapsed.as_secs_f64()
        );
    }
    println!("\n");

    let total_elapsed = start.elapsed();
    println!(
        "Recorded games benchmark completed in {:.1}s\n",
        total_elapsed.as_secs_f64()
    );

    // ─── Random games benchmark ───────────────────────────────────

    let mut rand_game_results: Vec<(i32, i32, i32, i32)> = Vec::new(); // (gt, mcts, heur, rand)
    let mut rand_gt_comp = Vec::new();
    let mut rand_mcts_comp = Vec::new();
    let mut rand_heur_comp = Vec::new();
    let mut rand_rand_comp = Vec::new();

    if args.random_games > 0 {
        println!(
            "Running benchmark on {} random tile sequences (MCTS: {} sims)...\n",
            args.random_games, args.mcts_sims
        );
        let rg_start = Instant::now();

        for i in 0..args.random_games {
            let tiles = generate_random_tile_sequence(&mut rng);

            let (gt_s, gt_lc) = play_gt_direct(&tiles, &policy_net);
            rand_gt_comp.push(gt_lc);

            let (mcts_s, mcts_lc) = play_pure_mcts(&tiles, args.mcts_sims);
            rand_mcts_comp.push(mcts_lc);

            let (heur_s, heur_lc) = play_heuristic(&tiles);
            rand_heur_comp.push(heur_lc);

            let (rand_s, rand_lc) = play_random(&tiles, &mut rng);
            rand_rand_comp.push(rand_lc);

            rand_game_results.push((gt_s, mcts_s, heur_s, rand_s));

            print!(
                "\r  [{}/{}] GT={}, MCTS={}, Heur={}, Rand={}",
                i + 1,
                args.random_games,
                gt_s,
                mcts_s,
                heur_s,
                rand_s
            );
        }
        println!(
            "\n\nRandom games benchmark completed in {:.1}s\n",
            rg_start.elapsed().as_secs_f64()
        );
    }

    // ─── Print results ────────────────────────────────────────────

    print_results_table("Recorded Games", &results, &gt_completions, &mcts_completions, &heur_completions, &rand_completions);

    if !rand_game_results.is_empty() {
        print_random_results_table(
            &rand_game_results,
            &rand_gt_comp,
            &rand_mcts_comp,
            &rand_heur_comp,
            &rand_rand_comp,
            args.mcts_sims,
        );
    }

    // ─── Save CSV ─────────────────────────────────────────────────

    save_results_csv(&results);
}

fn print_results_table(
    title: &str,
    results: &[GameResult],
    gt_comp: &[LineCompletions],
    mcts_comp: &[LineCompletions],
    heur_comp: &[LineCompletions],
    rand_comp: &[LineCompletions],
) {
    let n = results.len() as f64;
    if n == 0.0 {
        return;
    }

    let gt_scores: Vec<i32> = results.iter().map(|r| r.gt_direct_score).collect();
    let mcts_scores: Vec<i32> = results.iter().map(|r| r.mcts_score).collect();
    let heur_scores: Vec<i32> = results.iter().map(|r| r.heuristic_score).collect();
    let rand_scores: Vec<i32> = results.iter().map(|r| r.random_score).collect();
    let human_scores: Vec<i32> = results.iter().map(|r| r.human_score).collect();

    let gt_beats_count = results
        .iter()
        .filter(|r| r.mcts_score > r.gt_direct_score)
        .count();
    let heur_beats_count = results
        .iter()
        .filter(|r| r.heuristic_score > r.gt_direct_score)
        .count();
    let rand_beats_count = results
        .iter()
        .filter(|r| r.random_score > r.gt_direct_score)
        .count();
    let human_beats_count = results
        .iter()
        .filter(|r| r.human_score > r.gt_direct_score)
        .count();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!(
        "║     Strategy Benchmark — {} ({} games){}║",
        title,
        results.len(),
        " ".repeat(60usize.saturating_sub(35 + title.len() + format!("{}", results.len()).len()))
    );
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!(
        "{:<20} | {:>9} | {:>6} | {:>5} | {:>5} | {:>10}",
        "Strategy", "Avg Score", "Median", "Min", "Max", "> GT Direct"
    );
    println!(
        "{:-<20}-+-{:-<9}-+-{:-<6}-+-{:-<5}-+-{:-<5}-+-{:-<10}",
        "", "", "", "", "", ""
    );

    print_strategy_row("GT Direct", &gt_scores, None);
    print_strategy_row(
        &format!("Pure MCTS ({})", results.len().min(999)), // placeholder, replaced below
        &mcts_scores,
        Some((gt_beats_count, results.len())),
    );
    // Reprint with correct label
    print!(
        "\x1b[1A\r{:<20} | {:>9.1} | {:>6} | {:>5} | {:>5} | {:>9.0}%\n",
        format!("Pure MCTS ({})", 150),
        avg(&mcts_scores),
        median(&mcts_scores),
        mcts_scores.iter().min().unwrap_or(&0),
        mcts_scores.iter().max().unwrap_or(&0),
        gt_beats_count as f64 / n * 100.0
    );
    print_strategy_row(
        "Heuristic",
        &heur_scores,
        Some((heur_beats_count, results.len())),
    );
    print_strategy_row(
        "Random",
        &rand_scores,
        Some((rand_beats_count, results.len())),
    );
    print_strategy_row(
        "Human (recorded)",
        &human_scores,
        Some((human_beats_count, results.len())),
    );

    println!();

    // Line completions
    println!(
        "Line completions (avg per game):\n{:<20} | {:>7} | {:>8} | {:>8} | {:>11}",
        "Strategy", "v1 cols", "v2 diags", "v3 diags", "Total lines"
    );
    println!(
        "{:-<20}-+-{:-<7}-+-{:-<8}-+-{:-<8}-+-{:-<11}",
        "", "", "", "", ""
    );
    print_lc_row("GT Direct", gt_comp);
    print_lc_row("Pure MCTS", mcts_comp);
    print_lc_row("Heuristic", heur_comp);
    print_lc_row("Random", rand_comp);
    println!();
}

fn print_random_results_table(
    results: &[(i32, i32, i32, i32)],
    gt_comp: &[LineCompletions],
    mcts_comp: &[LineCompletions],
    heur_comp: &[LineCompletions],
    rand_comp: &[LineCompletions],
    mcts_sims: usize,
) {
    let n = results.len() as f64;

    let gt_scores: Vec<i32> = results.iter().map(|r| r.0).collect();
    let mcts_scores: Vec<i32> = results.iter().map(|r| r.1).collect();
    let heur_scores: Vec<i32> = results.iter().map(|r| r.2).collect();
    let rand_scores: Vec<i32> = results.iter().map(|r| r.3).collect();

    let mcts_beats = results.iter().filter(|r| r.1 > r.0).count();
    let heur_beats = results.iter().filter(|r| r.2 > r.0).count();
    let rand_beats = results.iter().filter(|r| r.3 > r.0).count();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!(
        "║     Strategy Benchmark — Random Games ({} games){}║",
        results.len(),
        " ".repeat(60usize.saturating_sub(44 + format!("{}", results.len()).len()))
    );
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!(
        "{:<20} | {:>9} | {:>6} | {:>5} | {:>5} | {:>10}",
        "Strategy", "Avg Score", "Median", "Min", "Max", "> GT Direct"
    );
    println!(
        "{:-<20}-+-{:-<9}-+-{:-<6}-+-{:-<5}-+-{:-<5}-+-{:-<10}",
        "", "", "", "", "", ""
    );

    print_strategy_row("GT Direct", &gt_scores, None);
    println!(
        "{:<20} | {:>9.1} | {:>6} | {:>5} | {:>5} | {:>9.0}%",
        format!("Pure MCTS ({})", mcts_sims),
        avg(&mcts_scores),
        median(&mcts_scores),
        mcts_scores.iter().min().unwrap_or(&0),
        mcts_scores.iter().max().unwrap_or(&0),
        mcts_beats as f64 / n * 100.0
    );
    print_strategy_row(
        "Heuristic",
        &heur_scores,
        Some((heur_beats, results.len())),
    );
    print_strategy_row(
        "Random",
        &rand_scores,
        Some((rand_beats, results.len())),
    );

    println!();

    println!(
        "Line completions (avg per game):\n{:<20} | {:>7} | {:>8} | {:>8} | {:>11}",
        "Strategy", "v1 cols", "v2 diags", "v3 diags", "Total lines"
    );
    println!(
        "{:-<20}-+-{:-<7}-+-{:-<8}-+-{:-<8}-+-{:-<11}",
        "", "", "", "", ""
    );
    print_lc_row("GT Direct", gt_comp);
    print_lc_row("Pure MCTS", mcts_comp);
    print_lc_row("Heuristic", heur_comp);
    print_lc_row("Random", rand_comp);
    println!();
}

fn print_strategy_row(name: &str, scores: &[i32], beats_gt: Option<(usize, usize)>) {
    let n = scores.len() as f64;
    let beats_str = match beats_gt {
        Some((count, total)) => format!("{:.0}%", count as f64 / total as f64 * 100.0),
        None => "—".to_string(),
    };

    println!(
        "{:<20} | {:>9.1} | {:>6} | {:>5} | {:>5} | {:>10}",
        name,
        avg(scores),
        median(scores),
        scores.iter().min().unwrap_or(&0),
        scores.iter().max().unwrap_or(&0),
        beats_str
    );
}

fn print_lc_row(name: &str, completions: &[LineCompletions]) {
    let n = completions.len() as f64;
    if n == 0.0 {
        return;
    }
    let v1_avg = completions.iter().map(|c| c.v1_cols).sum::<usize>() as f64 / n;
    let v2_avg = completions.iter().map(|c| c.v2_diags).sum::<usize>() as f64 / n;
    let v3_avg = completions.iter().map(|c| c.v3_diags).sum::<usize>() as f64 / n;
    let total_avg = completions.iter().map(|c| c.total()).sum::<usize>() as f64 / n;

    println!(
        "{:<20} | {:>7.1} | {:>8.1} | {:>8.1} | {:>11.1}",
        name, v1_avg, v2_avg, v3_avg, total_avg
    );
}

fn avg(scores: &[i32]) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }
    scores.iter().sum::<i32>() as f64 / scores.len() as f64
}

fn median(scores: &[i32]) -> i32 {
    if scores.is_empty() {
        return 0;
    }
    let mut sorted = scores.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}

fn save_results_csv(results: &[GameResult]) {
    let path = "benchmark_results.csv";
    let mut file = match File::create(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create {}: {}", path, e);
            return;
        }
    };

    writeln!(
        file,
        "game_id,gt_direct,pure_mcts,heuristic,random,human,ai_recorded"
    )
    .unwrap();

    for r in results {
        writeln!(
            file,
            "{},{},{},{},{},{},{}",
            r.game_id,
            r.gt_direct_score,
            r.mcts_score,
            r.heuristic_score,
            r.random_score,
            r.human_score,
            r.ai_recorded_score
        )
        .unwrap();
    }

    println!("Per-game details saved to: {}\n", path);
}
