use chrono::Utc;
use clap::Parser;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use rand::{rng, Rng};
use serde_json;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use tch::nn::{Optimizer, OptimizerConfig};
use tch::{nn, Device, IndexOp, Tensor};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};

use crate::game::deck::Deck;
use crate::game::plateau::create_plateau_empty;
use crate::logging::setup_logging;
use crate::mcts::mcts_result::MCTSResult;
use crate::mcts_vs_human::play_mcts_vs_human;
use game::create_deck::create_deck;
use game::plateau::Plateau;
use game::remove_tile_from_deck::replace_tile_in_deck;
use game::tile::Tile;
use neural::policy_value_net::{PolicyNet, ValueNet};
use crate::data::append_result::append_to_results_file;
use crate::data::load_data::load_game_data;
use crate::data::save_data::save_game_data;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::simulate_game::simulate_games;
use crate::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use crate::neural::tensor_conversion::convert_plateau_to_tensor;
use crate::neural::training::gradient_clipping::enhanced_gradient_clipping;
use crate::neural::training::normalization::robust_state_normalization;
use crate::neural::training::trainer::train_network_with_game_data;
use crate::scoring::scoring::result;
use crate::strategy::position_evaluation::enhanced_position_evaluation;
use crate::training::evaluator::evaluate_model;
use crate::training::session::train_and_evaluate;
use crate::training::websocket::reconnect_websocket;
use crate::utils::image::generate_tile_image_names;
use crate::utils::random_index::random_index;

mod test;

mod game;
mod logging;
mod mcts;
mod mcts_vs_human;
mod neural;
mod utils;
mod strategy;
mod scoring;
mod data;
mod training;

#[derive(Parser, Debug)]
#[command(name = "take_it_easy")]
struct Config {
    /// Number of games to simulate
    #[arg(short = 'g', long, default_value_t = 200)]
    num_games: usize,

    /// Number of simulations per game state
    #[arg(short = 's', long, default_value_t = 150)]
    num_simulations: usize,

    /// Run MCTS vs Human instead of training
    #[arg(long, default_value_t = true)]
    mcts_vs_human: bool,
}

#[tokio::main]
async fn main() {
    let config = Config::parse();
    let model_path = "model_weights";
    setup_logging();

    // Initialize VarStore
    let mut vs_policy = nn::VarStore::new(Device::Cpu);
    let mut vs_value = nn::VarStore::new(Device::Cpu);
    let input_dim = (5, 47, 1);
    let mut policy_net = PolicyNet::new(&vs_policy, input_dim);
    let mut value_net = ValueNet::new(&mut vs_value, input_dim);

    // Load weights if the model directory exists
    if Path::new(model_path).exists() {
        log::info!("🔄 Loading model weights from {}", model_path);

        if let Err(e) = policy_net.load_model(&mut vs_policy, "model_weights/policy/policy.params")
        {
            log::error!("⚠️ Error loading PolicyNet: {:?}", e);
            log::info!("➡️  Initializing PolicyNet with random weights.");
        }

        if let Err(e) = value_net.load_model(&mut vs_value, "model_weights/value/value.params") {
            log::error!("⚠️ Error loading ValueNet: {:?}", e);
            log::info!("➡️  Initializing ValueNet with random weights.");
        }
    } else {
        log::info!("📭 No pre-trained model found. Initializing new models.");
    }
    let mut optimizer_policy = nn::Adam::default().build(&vs_policy, 1e-3).unwrap();
    // Change your optimizer (around line 100):
    let mut optimizer_value = nn::Adam {
        wd: 1e-6, // Was 1e-5
        ..Default::default()
    }
    .build(&vs_value, 2e-4)
    .unwrap(); // Was 1e-3

    // ➕ Duel Mode: MCTS vs Human
    if config.mcts_vs_human {
        let listener = TcpListener::bind("127.0.0.1:9001")
            .await
            .expect("Unable to bind WebSocket on port 9001 for MCTS vs Human");

        log::info!("🧍‍♂️🤖 Waiting for MCTS vs Human connection...");
        let (stream, _) = listener.accept().await.unwrap();
        let ws_stream = accept_async(stream).await.unwrap();
        let (mut write, mut read) = ws_stream.split();

        play_mcts_vs_human(
            &policy_net,
            &value_net,
            config.num_simulations,
            &mut write,
            &mut read,
        )
        .await;

        return; // Exit after duel game
    }

    // 🧠 Training Mode
    let listener = TcpListener::bind("127.0.0.1:9000")
        .await
        .expect("Unable to bind WebSocket on port 9000 for training");
    log::info!("🧠 Training WebSocket server started at ws://127.0.0.1:9000");

    train_and_evaluate(
        &vs_policy,
        &vs_value,
        &mut policy_net,
        &mut value_net,
        &mut optimizer_policy,
        &mut optimizer_value,
        config.num_games,
        config.num_simulations,
        50, // Evaluate every 50 games
        listener.into(),
    )
    .await;
}











