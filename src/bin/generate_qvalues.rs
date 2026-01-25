//! Generate Q-values dataset from existing supervised data
//!
//! For each state in the input dataset, this script:
//! 1. Reconstructs the plateau and deck
//! 2. For each empty position, runs rollouts to estimate Q-value
//! 3. Outputs a new CSV with Q-values for all 19 positions

use clap::Parser;
use rayon::prelude::*;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::replace_tile_in_deck;
use take_it_easy::game::simulate_game_smart::simulate_games_smart;
use take_it_easy::game::tile::Tile;

#[derive(Parser, Debug)]
#[command(name = "generate-qvalues", about = "Generate Q-values dataset using rollouts")]
struct Args {
    /// Input CSV file with supervised data
    #[arg(short, long)]
    input: String,

    /// Output CSV file for Q-values dataset
    #[arg(short, long, default_value = "supervised_qvalues_full.csv")]
    output: String,

    /// Number of rollouts per position
    #[arg(short, long, default_value_t = 50)]
    rollouts: usize,

    /// Maximum number of examples to process (0 = all)
    #[arg(short, long, default_value_t = 0)]
    max_examples: usize,

    /// Number of parallel workers
    #[arg(short, long, default_value_t = 8)]
    workers: usize,
}

#[derive(Debug, Clone)]
struct InputExample {
    game_id: i32,
    turn: i32,
    plateau_state: Vec<i32>,
    tile: (i32, i32, i32),
    position: usize,
    final_score: i32,
}

#[derive(Debug, Clone)]
struct QValueExample {
    game_id: i32,
    turn: i32,
    plateau_state: Vec<i32>,
    tile: (i32, i32, i32),
    best_position: usize,
    final_score: i32,
    qvalues: [f32; 19], // Q-value for each position
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    println!("Loading input data from {}...", args.input);
    let examples = load_examples(&args.input, args.max_examples)?;
    println!("Loaded {} examples", examples.len());

    // Configure thread pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.workers)
        .build_global()?;

    println!(
        "Generating Q-values with {} rollouts per position using {} workers...",
        args.rollouts, args.workers
    );

    let processed = AtomicUsize::new(0);
    let total = examples.len();

    // Process examples in parallel
    let qvalue_examples: Vec<QValueExample> = examples
        .par_iter()
        .map(|ex| {
            let result = compute_qvalues(ex, args.rollouts);
            let count = processed.fetch_add(1, Ordering::Relaxed);
            if count % 500 == 0 {
                println!("Progress: {}/{} ({:.1}%)", count, total, count as f64 / total as f64 * 100.0);
            }
            result
        })
        .collect();

    println!("Done!");

    println!("Writing output to {}...", args.output);
    write_output(&args.output, &qvalue_examples)?;

    println!("Generated {} Q-value examples", qvalue_examples.len());

    Ok(())
}

fn load_examples(path: &str, max_examples: usize) -> Result<Vec<InputExample>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut examples = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        if line_num == 0 {
            continue; // Skip header
        }

        let line = line?;
        let fields: Vec<&str> = line.split(',').collect();

        if fields.len() < 25 {
            continue;
        }

        let game_id: i32 = fields[0].parse()?;
        let turn: i32 = fields[1].parse()?;

        let mut plateau_state = Vec::with_capacity(19);
        for i in 2..21 {
            plateau_state.push(fields[i].parse()?);
        }

        let tile = (
            fields[21].parse()?,
            fields[22].parse()?,
            fields[23].parse()?,
        );
        let position: usize = fields[24].parse()?;
        let final_score: i32 = fields[25].parse().unwrap_or(0);

        examples.push(InputExample {
            game_id,
            turn,
            plateau_state,
            tile,
            position,
            final_score,
        });

        if max_examples > 0 && examples.len() >= max_examples {
            break;
        }
    }

    Ok(examples)
}

fn compute_qvalues(example: &InputExample, num_rollouts: usize) -> QValueExample {
    // Reconstruct plateau
    let mut plateau = create_plateau_empty();
    for (pos, &encoded) in example.plateau_state.iter().enumerate() {
        if encoded != 0 {
            let v1 = encoded / 100;
            let v2 = (encoded % 100) / 10;
            let v3 = encoded % 10;
            plateau.tiles[pos] = Tile(v1, v2, v3);
        }
    }

    // Reconstruct deck (remove placed tiles and current tile)
    let mut deck = create_deck();
    for &encoded in &example.plateau_state {
        if encoded != 0 {
            let v1 = encoded / 100;
            let v2 = (encoded % 100) / 10;
            let v3 = encoded % 10;
            let tile = Tile(v1, v2, v3);
            deck = replace_tile_in_deck(&deck, &tile);
        }
    }
    let current_tile = Tile(example.tile.0, example.tile.1, example.tile.2);
    deck = replace_tile_in_deck(&deck, &current_tile);

    // Compute Q-value for each position
    let mut qvalues = [0.0f32; 19];
    let mut best_pos = example.position;
    let mut best_qvalue = f32::MIN;

    for pos in 0..19 {
        if plateau.tiles[pos] == Tile(0, 0, 0) {
            // Position is empty, compute Q-value via rollouts
            let mut temp_plateau = plateau.clone();
            temp_plateau.tiles[pos] = current_tile;

            let mut total_score = 0.0;
            for _ in 0..num_rollouts {
                let score = simulate_games_smart(temp_plateau.clone(), deck.clone(), None);
                total_score += score as f64;
            }
            let avg_score = (total_score / num_rollouts as f64) as f32;

            // Normalize to [0, 1] range (scores typically 0-200)
            qvalues[pos] = (avg_score / 200.0).clamp(0.0, 1.0);

            if avg_score > best_qvalue {
                best_qvalue = avg_score;
                best_pos = pos;
            }
        } else {
            // Position is occupied, Q-value is -1 (invalid)
            qvalues[pos] = -1.0;
        }
    }

    QValueExample {
        game_id: example.game_id,
        turn: example.turn,
        plateau_state: example.plateau_state.clone(),
        tile: example.tile,
        best_position: best_pos,
        final_score: example.final_score,
        qvalues,
    }
}

fn write_output(path: &str, examples: &[QValueExample]) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(path)?;

    // Write header
    let mut header = String::from("game_id,turn,");
    for i in 0..19 {
        header.push_str(&format!("plateau_{},", i));
    }
    header.push_str("tile_0,tile_1,tile_2,best_position,final_score,");
    for i in 0..19 {
        if i < 18 {
            header.push_str(&format!("qvalue_{},", i));
        } else {
            header.push_str(&format!("qvalue_{}", i));
        }
    }
    writeln!(file, "{}", header)?;

    // Write examples
    for ex in examples {
        let mut line = format!("{},{},", ex.game_id, ex.turn);

        for &p in &ex.plateau_state {
            line.push_str(&format!("{},", p));
        }

        line.push_str(&format!(
            "{},{},{},{},{},",
            ex.tile.0, ex.tile.1, ex.tile.2, ex.best_position, ex.final_score
        ));

        for (i, &qv) in ex.qvalues.iter().enumerate() {
            if i < 18 {
                line.push_str(&format!("{:.4},", qv));
            } else {
                line.push_str(&format!("{:.4}", qv));
            }
        }

        writeln!(file, "{}", line)?;
    }

    Ok(())
}
