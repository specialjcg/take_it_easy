use clap::Parser;
use futures_util::StreamExt;
use std::path::Path;
use tch::nn::{self, OptimizerConfig};
use tch::Device;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

use crate::logging::setup_logging;
use crate::mcts_vs_human::play_mcts_vs_human;
use neural::policy_value_net::{PolicyNet, ValueNet};
use crate::training::session::train_and_evaluate;

#[cfg(test)]
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

    /// Mode de jeu
    #[arg(long, value_enum, default_value = "mcts-vs-human")]
    mode: GameMode,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum GameMode {
    /// Mode entraînement normal
    Training,
    /// MCTS vs un seul humain
    MctsVsHuman,
    /// MCTS vs plusieurs humains (style Kahoot)
    Multiplayer,
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
    match config.mode {
        GameMode::Training => {
            log::info!("🧠 Starting training mode...");
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
                50,
                listener.into(),
            )
                .await;
        }

        GameMode::MctsVsHuman => {
            log::info!("🧍‍♂️🤖 Starting MCTS vs Human mode...");
            let listener = TcpListener::bind("127.0.0.1:9001")
                .await
                .expect("Unable to bind WebSocket on port 9001 for MCTS vs Human");

            let (stream, _) = listener.accept().await.unwrap();
            let ws_stream = accept_async(stream).await.unwrap();
            let (mut write, mut read) = ws_stream.split();

            play_mcts_vs_human(
                &policy_net,
                &value_net,
                config.num_simulations,
                &mut write,
                &mut read,
                (&listener).into(),
            )
                .await;
        }

        GameMode::Multiplayer => {
            log::info!("🎮👥 Starting Multiplayer mode (MCTS vs Multiple Humans)...");
            log::info!("🔗 Players can connect and create/join sessions");
            log::info!("📋 Session codes will be generated for easy joining");

            // start_multiplayer_server(
            //     policy_net,
            //     value_net,
            //     config.num_simulations,
            // )
            //     .await;
        }
    }
}











