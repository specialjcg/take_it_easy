//! Benchmark GAT with different channel counts: 8ch vs 47ch vs 95ch
//!
//! Usage: cargo run --release --bin benchmark_gat -- --games 200 --epochs 50

use clap::Parser;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::time::Instant;
use tch::{nn, nn::OptimizerConfig, Device, IndexOp, Kind, Tensor};

use take_it_easy::game::create_deck::create_deck;
use take_it_easy::game::plateau::create_plateau_empty;
use take_it_easy::game::remove_tile_from_deck::{get_available_tiles, replace_tile_in_deck};
use take_it_easy::game::tile::Tile;
use take_it_easy::neural::gat::GATPolicyNet;
use take_it_easy::neural::gnn::GraphPolicyNet;
use take_it_easy::neural::tensor_conversion::{convert_plateau_for_gat_47ch, convert_plateau_for_gat_extended, convert_plateau_for_gnn_with_tile};
use take_it_easy::scoring::scoring::result;

#[derive(Parser, Debug)]
#[command(name = "benchmark_gat")]
struct Args {
    #[arg(long, default_value_t = 200)]
    games: usize,
    #[arg(long, default_value_t = 50)]
    epochs: usize,
    #[arg(long, default_value_t = 4)]
    num_heads: usize,
    #[arg(long, default_value_t = 64)]
    hidden_dim: i64,
    #[arg(long, default_value_t = 2)]
    num_layers: usize,
    #[arg(long, default_value_t = 0.001)]
    lr: f64,
    #[arg(long, default_value_t = 0.1)]
    dropout: f64,
    #[arg(long, default_value_t = 100)]
    eval_games: usize,
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

struct Sample { features: Tensor, best_pos: i64 }

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       Channel Count Comparison: 8ch vs 47ch vs 95ch          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let device = Device::Cpu;
    println!("📍 Config: {} heads, {} hidden, {} layers", args.num_heads, args.hidden_dim, args.num_layers);

    println!("\n📊 Generating training data from {} games...", args.games);
    let start = Instant::now();
    let (data_8, data_47, data_95) = generate_all_data(args.games, args.seed);
    println!("   {} samples in {:.2}s\n", data_8.len(), start.elapsed().as_secs_f32());

    let hidden: Vec<i64> = vec![args.hidden_dim; args.num_layers];

    // Train all three
    println!("🔷 GAT (95ch - extended)...");
    let s95 = train_gat(&data_95, 95, &hidden, args.num_heads, args.dropout, args.epochs, args.lr, args.eval_games, args.seed, device);

    println!("\n🔶 GAT (47ch - lines)...");
    let s47 = train_gat(&data_47, 47, &hidden, args.num_heads, args.dropout, args.epochs, args.lr, args.eval_games, args.seed, device);

    println!("\n⚪ GNN (8ch - basic)...");
    let s8 = train_gnn(&data_8, &hidden, args.dropout, args.epochs, args.lr, args.eval_games, args.seed, device);

    println!("\n🟡 Baselines...");
    let rand_s = eval_random(args.eval_games, args.seed);
    let greedy_s = eval_greedy(args.eval_games, args.seed);

    // Results
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                        RESULTS                                ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Model          │ Channels │  Score  │ vs Random │ vs Greedy ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  GAT extended   │    95    │ {:>6.2}  │  {:>+6.2}   │  {:>+6.2}   ║", s95, s95-rand_s, s95-greedy_s);
    println!("║  GAT lines      │    47    │ {:>6.2}  │  {:>+6.2}   │  {:>+6.2}   ║", s47, s47-rand_s, s47-greedy_s);
    println!("║  GNN basic      │     8    │ {:>6.2}  │  {:>+6.2}   │  {:>+6.2}   ║", s8, s8-rand_s, s8-greedy_s);
    println!("║  Greedy         │     -    │ {:>6.2}  │  {:>+6.2}   │     -     ║", greedy_s, greedy_s-rand_s);
    println!("║  Random         │     -    │ {:>6.2}  │     -     │     -     ║", rand_s);
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\n📈 Improvement breakdown:");
    println!("   8ch → 47ch:  {:>+6.2} pts (line features)", s47 - s8);
    println!("   47ch → 95ch: {:>+6.2} pts (extended features)", s95 - s47);
    println!("   8ch → 95ch:  {:>+6.2} pts (total)", s95 - s8);

    if s95 > greedy_s { println!("\n   🏆 GAT (95ch) BEATS Greedy!"); }
    else if s47 > greedy_s { println!("\n   🏆 GAT (47ch) BEATS Greedy!"); }
}

fn generate_all_data(n: usize, seed: u64) -> (Vec<Sample>, Vec<Sample>, Vec<Sample>) {
    let mut d8 = Vec::new();
    let mut d47 = Vec::new();
    let mut d95 = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);

    for g in 0..n {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();

            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            let f8 = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);
            let f47 = convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19);
            let f95 = convert_plateau_for_gat_extended(&plateau, &tile, &deck, turn, 19);

            let best = find_greedy(&plateau, &tile, &avail) as i64;

            d8.push(Sample { features: f8, best_pos: best });
            d47.push(Sample { features: f47, best_pos: best });
            d95.push(Sample { features: f95, best_pos: best });

            plateau.tiles[best as usize] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        if (g+1) % 50 == 0 { print!("   Game {}/{}\r", g+1, n); }
    }
    println!();
    (d8, d47, d95)
}

fn find_greedy(p: &take_it_easy::game::plateau::Plateau, t: &Tile, a: &[usize]) -> usize {
    a.iter().copied().max_by_key(|&pos| {
        let mut test = p.clone();
        test.tiles[pos] = *t;
        result(&test)
    }).unwrap_or(a[0])
}

fn train_gat(data: &[Sample], ch: i64, hid: &[i64], heads: usize, drop: f64, ep: usize, lr: f64, eval_n: usize, seed: u64, dev: Device) -> f64 {
    let vs = nn::VarStore::new(dev);
    let net = GATPolicyNet::new(&vs, ch, hid, heads, drop);
    let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

    let bs = 32;
    let nb = data.len() / bs;
    let mut rng = StdRng::seed_from_u64(seed);

    for epoch in 0..ep {
        let mut loss_sum = 0.0;
        let mut idx: Vec<usize> = (0..data.len()).collect();
        idx.shuffle(&mut rng);

        for b in 0..nb {
            let bi = &idx[b*bs..(b+1)*bs];
            let feats: Vec<Tensor> = bi.iter().map(|&i| data[i].features.shallow_clone()).collect();
            let tgts: Vec<i64> = bi.iter().map(|&i| data[i].best_pos).collect();

            let bf = Tensor::stack(&feats, 0);
            let bt = Tensor::from_slice(&tgts);
            let logits = net.forward(&bf, true);
            let loss = logits.cross_entropy_for_logits(&bt);
            opt.backward_step(&loss);
            loss_sum += f64::try_from(&loss).unwrap();
        }

        if (epoch+1) % 10 == 0 || epoch == 0 {
            println!("   Epoch {}/{}: loss={:.4}", epoch+1, ep, loss_sum/nb as f64);
        }
    }

    // Eval
    let mut total = 0;
    let mut rng = StdRng::seed_from_u64(seed + 10000);
    for _ in 0..eval_n {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();
        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();
            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            let f = if ch == 47 {
                convert_plateau_for_gat_47ch(&plateau, &tile, &deck, turn, 19)
            } else {
                convert_plateau_for_gat_extended(&plateau, &tile, &deck, turn, 19)
            };

            let logits = net.forward(&f.unsqueeze(0), false);
            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
            let best: i64 = (logits.squeeze_dim(0) + mask).argmax(0, false).try_into().unwrap();

            plateau.tiles[best as usize] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        total += result(&plateau);
    }
    let avg = total as f64 / eval_n as f64;
    println!("   Eval: {:.2} pts", avg);
    avg
}

fn train_gnn(data: &[Sample], hid: &[i64], drop: f64, ep: usize, lr: f64, eval_n: usize, seed: u64, dev: Device) -> f64 {
    let vs = nn::VarStore::new(dev);
    let net = GraphPolicyNet::new(&vs, 8, hid, drop);
    let mut opt = nn::Adam::default().build(&vs, lr).unwrap();

    let bs = 32;
    let nb = data.len() / bs;
    let mut rng = StdRng::seed_from_u64(seed);

    for epoch in 0..ep {
        let mut loss_sum = 0.0;
        let mut idx: Vec<usize> = (0..data.len()).collect();
        idx.shuffle(&mut rng);

        for b in 0..nb {
            let bi = &idx[b*bs..(b+1)*bs];
            let feats: Vec<Tensor> = bi.iter().map(|&i| data[i].features.shallow_clone()).collect();
            let tgts: Vec<i64> = bi.iter().map(|&i| data[i].best_pos).collect();

            let bf = Tensor::stack(&feats, 0);
            let bt = Tensor::from_slice(&tgts);
            let logits = net.forward(&bf, true);
            let loss = logits.cross_entropy_for_logits(&bt);
            opt.backward_step(&loss);
            loss_sum += f64::try_from(&loss).unwrap();
        }

        if (epoch+1) % 10 == 0 || epoch == 0 {
            println!("   Epoch {}/{}: loss={:.4}", epoch+1, ep, loss_sum/nb as f64);
        }
    }

    let mut total = 0;
    let mut rng = StdRng::seed_from_u64(seed + 10000);
    for _ in 0..eval_n {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();
        for turn in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();
            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }

            let f = convert_plateau_for_gnn_with_tile(&plateau, &tile, turn, 19);
            let logits = net.forward(&f.unsqueeze(0), false);
            let mask = Tensor::full([19], f64::NEG_INFINITY, (Kind::Float, Device::Cpu));
            for &pos in &avail { let _ = mask.i(pos as i64).fill_(0.0); }
            let best: i64 = (logits.squeeze_dim(0) + mask).argmax(0, false).try_into().unwrap();

            plateau.tiles[best as usize] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        total += result(&plateau);
    }
    let avg = total as f64 / eval_n as f64;
    println!("   Eval: {:.2} pts", avg);
    avg
}

fn eval_random(n: usize, seed: u64) -> f64 {
    let mut total = 0;
    let mut rng = StdRng::seed_from_u64(seed + 20000);
    for _ in 0..n {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();
        for _ in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();
            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }
            let pos = *avail.choose(&mut rng).unwrap();
            plateau.tiles[pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        total += result(&plateau);
    }
    println!("   Random: {:.2} pts", total as f64 / n as f64);
    total as f64 / n as f64
}

fn eval_greedy(n: usize, seed: u64) -> f64 {
    let mut total = 0;
    let mut rng = StdRng::seed_from_u64(seed + 30000);
    for _ in 0..n {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();
        for _ in 0..19 {
            let tiles = get_available_tiles(&deck);
            if tiles.is_empty() { break; }
            let tile = *tiles.choose(&mut rng).unwrap();
            let avail: Vec<usize> = (0..19).filter(|&i| plateau.tiles[i] == Tile(0, 0, 0)).collect();
            if avail.is_empty() { break; }
            let pos = find_greedy(&plateau, &tile, &avail);
            plateau.tiles[pos] = tile;
            deck = replace_tile_in_deck(&deck, &tile);
        }
        total += result(&plateau);
    }
    println!("   Greedy: {:.2} pts", total as f64 / n as f64);
    total as f64 / n as f64
}
