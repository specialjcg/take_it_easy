// main.rs - Version corrigée avec les bonnes compatibilités
use clap::Parser;
use flexi_logger::Logger;
use tokio::net::TcpListener;

use crate::neural::{NeuralConfig, NeuralManager};
use crate::training::session::{train_and_evaluate, train_and_evaluate_offline};

#[cfg(test)]
mod test;

// Modules existants (inchangés)
mod data;
mod game;
mod logging;
mod mcts;
mod neural;
mod scoring;
mod strategy;
mod training;
mod utils;

// Nouveaux modules avec paradigme fonctionnel
mod generated;
mod servers;
mod services;

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum NnArchitectureCli {
    Cnn,
    Gnn,
}

impl From<NnArchitectureCli> for neural::manager::NNArchitecture {
    fn from(cli: NnArchitectureCli) -> Self {
        match cli {
            NnArchitectureCli::Cnn => neural::manager::NNArchitecture::CNN,
            NnArchitectureCli::Gnn => neural::manager::NNArchitecture::GNN,
        }
    }
}

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
    #[arg(long, value_enum, default_value = "multiplayer")]
    mode: GameMode,

    /// Port pour le serveur gRPC Multiplayer
    #[arg(short = 'p', long, default_value_t = 50051)]
    port: u16,

    /// Mode entraînement sans interface WebSocket
    #[arg(long, default_value_t = false)]
    offline_training: bool,

    /// Nombre de parties par vague d'entraînement avant évaluation
    #[arg(long, default_value_t = 50)]
    evaluation_interval: usize,

    /// Mode un seul joueur contre MCTS (pour mode multiplayer)
    #[arg(long, default_value_t = false)]
    single_player: bool,

    /// Score minimum pour considérer une partie comme haute qualité
    #[arg(long, default_value_t = 140.0)]
    min_score_high: f64,

    /// Score minimum pour ajouter quelques parties moyennes
    #[arg(long, default_value_t = 120.0)]
    min_score_medium: f64,

    /// Ratio de parties moyennes injectées par rapport au nombre de parties hautes
    #[arg(long, default_value_t = 0.2)]
    medium_mix_ratio: f32,

    /// Surcoût de simulations en début/fin de partie
    #[arg(long, default_value_t = 50)]
    dynamic_sim_boost: usize,

    /// Architecture du réseau de neurones (cnn ou gnn)
    #[arg(long, value_enum, default_value = "cnn")]
    nn_architecture: NnArchitectureCli,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum GameMode {
    /// Mode entraînement normal
    Training,
    /// Mode multijoueur
    Multiplayer,
}

// ============================================================================
// SERVEUR WEB POUR LES FICHIERS STATIQUES
// ============================================================================

async fn start_web_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let config = servers::WebUiConfig {
        port: port + 1000,
        host: "0.0.0.0".to_string(),
    };

    let web_ui_server = servers::WebUiServer::new(config);
    web_ui_server.start().await
}

// ============================================================================
// SERVEUR GRPC AVEC GRPC-WEB
// ============================================================================

async fn start_multiplayer_server(
    neural_manager: NeuralManager,
    num_simulations: usize,
    port: u16,
    single_player: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("🎯 Interface web : http://localhost:{}", port + 1000);

    let grpc_config = servers::GrpcConfig {
        port,
        web_port: port + 1,
        host: "0.0.0.0".to_string(),
        enable_web_layer: true,
        enable_cors: true,
    };

    // Extract components from neural manager
    let components = neural_manager.into_components();

    let grpc_server = servers::GrpcServer::new(
        grpc_config,
        components.policy_net,
        components.value_net,
        num_simulations,
        single_player,
    );

    grpc_server.start().await
}

// ============================================================================
// FONCTION PRINCIPALE
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::parse();

    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    // Initialize neural network manager with configuration
    let neural_config = NeuralConfig {
        input_dim: (8, 5, 5), // Enhanced feature stack: 8 channels × 5×5 spatial grid
        model_path: "model_weights".to_string(),
        policy_lr: 1e-3,
        value_lr: 2e-4,
        value_wd: 1e-6,
        nn_architecture: config.nn_architecture.clone().into(),
        ..Default::default()
    };

    let neural_manager = NeuralManager::with_config(neural_config)?;

    // Match sur les modes
    match config.mode {
        GameMode::Training => {
            let training_options = training::session::TrainingOptions {
                min_score_high: config.min_score_high,
                min_score_medium: config.min_score_medium.min(config.min_score_high),
                medium_mix_ratio: config.medium_mix_ratio.clamp(0.0, 1.0),
                dynamic_sim_boost: config.dynamic_sim_boost,
            };
            if config.offline_training {
                log::info!("[Training] Mode offline activé (sans WebSocket)");
                let mut components = neural_manager.into_components();
                train_and_evaluate_offline(
                    &components.vs_policy,
                    &components.vs_value,
                    &mut components.policy_net,
                    &mut components.value_net,
                    &mut components.optimizer_policy,
                    &mut components.optimizer_value,
                    config.num_games,
                    config.num_simulations,
                    config.evaluation_interval,
                    &training_options,
                )
                .await;
            } else {
                let listener = TcpListener::bind("127.0.0.1:9000")
                    .await
                    .expect("Unable to bind WebSocket on port 9000 for training");

                // For training, extract all components since we need mutable access
                let mut components = neural_manager.into_components();
                train_and_evaluate(
                    &components.vs_policy,
                    &components.vs_value,
                    &mut components.policy_net,
                    &mut components.value_net,
                    &mut components.optimizer_policy,
                    &mut components.optimizer_value,
                    config.num_games,
                    config.num_simulations,
                    config.evaluation_interval,
                    listener.into(),
                    &training_options,
                )
                .await;
            }
        }
        GameMode::Multiplayer => {
            // Lancer le serveur web en arrière-plan
            let web_port = config.port;
            tokio::spawn(async move {
                if let Err(e) = start_web_server(web_port).await {
                    log::error!("❌ Web server error: {}", e);
                }
            });

            // Donner un peu de temps au serveur web pour démarrer
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Lancer le serveur gRPC (bloquant)
            start_multiplayer_server(
                neural_manager,
                config.num_simulations,
                config.port,
                config.single_player,
            )
            .await?;
        }
    }
    Ok(())
}
