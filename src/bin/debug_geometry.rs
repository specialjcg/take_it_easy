//! Debug tool to understand geometry mismatch between scoring and tensor encoding
//!
//! This tool visualizes:
//! 1. The hexagonal game layout
//! 2. The 5x5 tensor grid mapping
//! 3. How scoring lines appear in the tensor
//! 4. Why CNN might fail to learn the patterns

use take_it_easy::game::tile::Tile;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::simulate_game_smart::simulate_games_smart;
use take_it_easy::game::get_legal_moves::get_legal_moves;

/// The HEX_TO_GRID_MAP from tensor_conversion.rs
const HEX_TO_GRID_MAP: [(usize, usize); 19] = [
    // Column 0 (positions 0-2)
    (1, 0), (2, 0), (3, 0),
    // Column 1 (positions 3-6)
    (1, 1), (2, 1), (3, 1), (4, 1),
    // Column 2 (positions 7-11)
    (0, 2), (1, 2), (2, 2), (3, 2), (4, 2),
    // Column 3 (positions 12-15)
    (1, 3), (2, 3), (3, 3), (4, 3),
    // Column 4 (positions 16-18)
    (1, 4), (2, 4), (3, 4),
];

/// Scoring line definitions
const SCORING_LINES: &[(&[usize], &str, usize)] = &[
    // Direction 1 (tile.0) - "Horizontal" rows
    (&[0, 1, 2], "Dir1-Row0", 0),
    (&[3, 4, 5, 6], "Dir1-Row1", 0),
    (&[7, 8, 9, 10, 11], "Dir1-Row2", 0),
    (&[12, 13, 14, 15], "Dir1-Row3", 0),
    (&[16, 17, 18], "Dir1-Row4", 0),
    // Direction 2 (tile.1) - NE-SW diagonals
    (&[0, 3, 7], "Dir2-Diag0", 1),
    (&[1, 4, 8, 12], "Dir2-Diag1", 1),
    (&[2, 5, 9, 13, 16], "Dir2-Diag2", 1),
    (&[6, 10, 14, 17], "Dir2-Diag3", 1),
    (&[11, 15, 18], "Dir2-Diag4", 1),
    // Direction 3 (tile.2) - NW-SE diagonals
    (&[7, 12, 16], "Dir3-Diag0", 2),
    (&[3, 8, 13, 17], "Dir3-Diag1", 2),
    (&[0, 4, 9, 14, 18], "Dir3-Diag2", 2),
    (&[1, 5, 10, 15], "Dir3-Diag3", 2),
    (&[2, 6, 11], "Dir3-Diag4", 2),
];

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║          TAKE IT EASY - GEOMETRY ANALYSIS & DEBUG TOOL                   ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝\n");

    // Section 1: Hexagonal Layout
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("1. HEXAGONAL GAME LAYOUT (position indices)");
    println!("═══════════════════════════════════════════════════════════════════════════\n");
    println!("       0   1   2");
    println!("      3   4   5   6");
    println!("     7   8   9  10  11");
    println!("      12  13  14  15");
    println!("        16  17  18\n");

    // Section 2: 5x5 Grid Mapping
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("2. TENSOR 5×5 GRID MAPPING (showing position indices)");
    println!("═══════════════════════════════════════════════════════════════════════════\n");

    let mut grid = [["."; 5]; 5];
    let pos_names = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18"];

    for (pos, &(row, col)) in HEX_TO_GRID_MAP.iter().enumerate() {
        grid[row][col] = pos_names[pos];
    }

    println!("         Col0  Col1  Col2  Col3  Col4");
    for (row, cols) in grid.iter().enumerate() {
        print!("   Row{}: ", row);
        for col in cols {
            print!("{:>4}  ", col);
        }
        println!();
    }
    println!();

    // Section 3: Line Analysis
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("3. SCORING LINES IN TENSOR GRID (checking if they form straight lines)");
    println!("═══════════════════════════════════════════════════════════════════════════\n");

    let mut straight_lines = 0;
    let mut broken_lines = 0;

    for (positions, name, direction) in SCORING_LINES {
        let grid_coords: Vec<(usize, usize)> = positions.iter()
            .map(|&pos| HEX_TO_GRID_MAP[pos])
            .collect();

        // Check if it's a straight line (all same row, all same col, or perfect diagonal)
        let is_straight = check_straight_line(&grid_coords);
        let status = if is_straight { "✓ STRAIGHT" } else { "✗ BROKEN" };

        if is_straight {
            straight_lines += 1;
        } else {
            broken_lines += 1;
        }

        println!("{} [Dir{}] {:?}", name, direction, positions);
        print!("   Grid coords: ");
        for (i, &(row, col)) in grid_coords.iter().enumerate() {
            if i > 0 { print!(" → "); }
            print!("({},{})", row, col);
        }
        println!("  {}", status);

        // Visualize the line in grid
        let mut line_grid = [[' '; 5]; 5];
        for &(row, col) in &grid_coords {
            line_grid[row][col] = '█';
        }
        println!("   Visualization:");
        for row in &line_grid {
            print!("      ");
            for &cell in row {
                print!("{} ", cell);
            }
            println!();
        }
        println!();
    }

    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("   SUMMARY: {} straight lines, {} broken lines", straight_lines, broken_lines);
    println!("═══════════════════════════════════════════════════════════════════════════\n");

    // Section 4: What the CNN "sees" vs what rollouts compute
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("4. CNN vs ROLLOUTS: Position evaluation comparison");
    println!("═══════════════════════════════════════════════════════════════════════════\n");

    // Create a test game state
    let mut plateau = create_plateau_empty();
    let deck = create_deck();
    let test_tile = Tile(9, 7, 8);  // High-value tile

    println!("Test scenario: Empty board, placing tile {:?}", test_tile);
    println!("This tile has high values, so placement strategy matters.\n");

    // Evaluate each position with rollouts
    let legal_moves = get_legal_moves(&plateau);
    let mut rollout_scores: Vec<(usize, f64)> = Vec::new();

    println!("Computing rollout scores for each position...");
    for &pos in &legal_moves {
        let mut temp_plateau = plateau.clone();
        temp_plateau.tiles[pos] = test_tile;

        // Run 100 rollouts
        let mut total_score = 0.0;
        for _ in 0..100 {
            let score = simulate_games_smart(temp_plateau.clone(), deck.clone(), None) as f64;
            total_score += score;
        }
        let avg_score = total_score / 100.0;
        rollout_scores.push((pos, avg_score));
    }

    rollout_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!("\nRollout rankings (what the AI SHOULD learn):");
    for (rank, (pos, score)) in rollout_scores.iter().enumerate() {
        let (row, col) = HEX_TO_GRID_MAP[*pos];
        println!("   #{:2}: Position {:2} → Grid({},{}) → Avg score: {:.1}",
                 rank + 1, pos, row, col, score);
    }

    // Section 5: Geometric insight
    println!("\n═══════════════════════════════════════════════════════════════════════════");
    println!("5. KEY GEOMETRIC INSIGHTS");
    println!("═══════════════════════════════════════════════════════════════════════════\n");

    println!("The PROBLEM: The 5×5 grid mapping creates NON-STRAIGHT lines for Dir2/Dir3!");
    println!();
    println!("Why this breaks CNN learning:");
    println!("  - CNNs use 3×3 convolutions to detect LOCAL patterns");
    println!("  - For Dir1 lines: Positions are VERTICAL in grid → 1×N conv can detect");
    println!("  - For Dir2/Dir3 lines: Positions are ZIGZAG → no simple conv filter works!");
    println!();
    println!("Example of the problem (Dir2-Diag0 = [0, 3, 7]):");
    println!("  - Position 0 → Grid (1, 0)");
    println!("  - Position 3 → Grid (1, 1)  ← same row as 0");
    println!("  - Position 7 → Grid (0, 2)  ← DIFFERENT row! Breaks the line!");
    println!();
    println!("What rollouts 'see' that CNN cannot:");
    println!("  1. Complete line potential (all positions in a line with same value)");
    println!("  2. Probability of completing a line (based on remaining tiles)");
    println!("  3. Interactions between 3 directions at once");
    println!();

    // Section 6: Suggested fixes
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("6. POSSIBLE SOLUTIONS");
    println!("═══════════════════════════════════════════════════════════════════════════\n");

    println!("Option A: Add EXPLICIT LINE FEATURES (15 channels for 15 lines)");
    println!("  - Each channel = line completion status (0=blocked, 0.5=partial, 1=complete)");
    println!("  - CNN doesn't need to 'discover' line patterns, they're given directly");
    println!();
    println!("Option B: Use GNN (Graph Neural Network) with true hexagonal adjacency");
    println!("  - Nodes = 19 positions, edges = scoring line connections");
    println!("  - Message passing respects actual game geometry");
    println!();
    println!("Option C: Train CNN with LINE-BASED targets instead of position targets");
    println!("  - Output: 15 line scores instead of 19 position logits");
    println!("  - Then derive position from line contributions");
    println!();
    println!("Option D: Use 3-channel per direction (stacked vertically)");
    println!("  - Separate the 3 directions into different grid layouts");
    println!("  - Each direction's lines become proper straight lines");
}

fn check_straight_line(coords: &[(usize, usize)]) -> bool {
    if coords.len() < 2 {
        return true;
    }

    // Check if all same row
    let same_row = coords.iter().all(|(r, _)| *r == coords[0].0);
    if same_row {
        // Check if columns are consecutive
        let mut cols: Vec<_> = coords.iter().map(|(_, c)| *c).collect();
        cols.sort();
        return cols.windows(2).all(|w| w[1] == w[0] + 1);
    }

    // Check if all same column
    let same_col = coords.iter().all(|(_, c)| *c == coords[0].1);
    if same_col {
        // Check if rows are consecutive
        let mut rows: Vec<_> = coords.iter().map(|(r, _)| *r).collect();
        rows.sort();
        return rows.windows(2).all(|w| w[1] == w[0] + 1);
    }

    // Check diagonal (row and col both change by 1)
    let mut sorted: Vec<_> = coords.to_vec();
    sorted.sort_by_key(|(r, c)| (*r, *c));

    let is_down_right = sorted.windows(2).all(|w| {
        let diff_row = w[1].0 as i32 - w[0].0 as i32;
        let diff_col = w[1].1 as i32 - w[0].1 as i32;
        diff_row == 1 && diff_col == 1
    });

    let is_down_left = sorted.windows(2).all(|w| {
        let diff_row = w[1].0 as i32 - w[0].0 as i32;
        let diff_col = w[1].1 as i32 - w[0].1 as i32;
        diff_row == 1 && diff_col == -1
    });

    is_down_right || is_down_left
}
