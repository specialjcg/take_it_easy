// main.rs - Version corrigée avec les bonnes compatibilités
use clap::Parser;
use flexi_logger::Logger;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::neural::{NeuralConfig, NeuralManager, QNetManager};
use crate::training::session::{train_and_evaluate, train_and_evaluate_offline};

#[cfg(test)]
mod test;

// Modules existants (inchangés)
mod auth;
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
            NnArchitectureCli::Cnn => neural::manager::NNArchitecture::Cnn,
            NnArchitectureCli::Gnn => neural::manager::NNArchitecture::Gnn,
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

    /// Enable Q-Net hybrid MCTS for improved AI performance (recommended)
    #[arg(long, default_value_t = true)]
    hybrid_mcts: bool,

    /// Path to Q-value network weights for hybrid MCTS
    #[arg(long, default_value = "model_weights/qvalue_net.params")]
    qnet_path: String,

    /// Top-K positions for Q-net pruning (6 is optimal)
    #[arg(long, default_value_t = 6)]
    top_k: usize,

    /// Enable authentication system
    #[arg(long, default_value_t = true)]
    enable_auth: bool,

    /// Path to SQLite database for authentication
    #[arg(long, default_value = "data/auth.db")]
    auth_db_path: String,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum GameMode {
    /// Mode entraînement normal
    Training,
    /// Mode multijoueur
    Multiplayer,
}

// ============================================================================
// SERVEUR WEB POUR LES FICHIERS STATIQUES + AUTHENTIFICATION
// ============================================================================

async fn start_web_server(
    port: u16,
    auth_state: Option<Arc<auth::AuthState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = servers::WebUiConfig {
        port: port + 1000,
        host: "0.0.0.0".to_string(),
    };

    let web_ui_server = match auth_state {
        Some(state) => servers::WebUiServer::with_auth(config, state),
        None => servers::WebUiServer::new(config),
    };

    web_ui_server.start().await
}

// ============================================================================
// SERVEUR GRPC AVEC GRPC-WEB
// ============================================================================

async fn start_multiplayer_server(
    neural_manager: NeuralManager,
    qnet_manager: Option<QNetManager>,
    num_simulations: usize,
    port: u16,
    single_player: bool,
    top_k: usize,
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

    // Create server with or without Q-Net hybrid
    let grpc_server = if let Some(qnet) = qnet_manager {
        log::info!("🚀 MCTS Hybrid activé avec Q-Net (top-{})", top_k);
        servers::GrpcServer::new_hybrid(
            grpc_config,
            components.policy_net,
            components.value_net,
            qnet.into_net(),
            num_simulations,
            single_player,
            top_k,
        )
    } else {
        log::info!("📊 MCTS CNN standard (sans Q-Net)");
        servers::GrpcServer::new(
            grpc_config,
            components.policy_net,
            components.value_net,
            num_simulations,
            single_player,
        )
    };

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
        input_dim: (9, 5, 5), // Enhanced feature stack: 8 channels × 5×5 spatial grid
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
            // Load Q-Net for hybrid MCTS if enabled
            let qnet_manager = if config.hybrid_mcts {
                match QNetManager::new(&config.qnet_path) {
                    Ok(qnet) => {
                        log::info!("✅ Q-Net chargé depuis {}", config.qnet_path);
                        Some(qnet)
                    }
                    Err(e) => {
                        log::warn!("⚠️ Impossible de charger Q-Net ({}), mode CNN standard", e);
                        None
                    }
                }
            } else {
                log::info!("ℹ️ Mode Hybrid désactivé, utilisation du MCTS CNN standard");
                None
            };

            // Initialize authentication if enabled
            let auth_state = if config.enable_auth {
                // Ensure data directory exists
                if let Some(parent) = std::path::Path::new(&config.auth_db_path).parent() {
                    std::fs::create_dir_all(parent).ok();
                }

                match auth::AuthState::new(&config.auth_db_path) {
                    Ok(state) => {
                        log::info!("🔐 Authentication enabled (db: {})", config.auth_db_path);
                        Some(Arc::new(state))
                    }
                    Err(e) => {
                        log::warn!("⚠️ Failed to initialize auth ({}), continuing without", e);
                        None
                    }
                }
            } else {
                log::info!("ℹ️ Authentication disabled");
                None
            };

            // Lancer le serveur web en arrière-plan
            let web_port = config.port;
            let auth_state_clone = auth_state.clone();
            tokio::spawn(async move {
                if let Err(e) = start_web_server(web_port, auth_state_clone).await {
                    log::error!("❌ Web server error: {}", e);
                }
            });

            // Donner un peu de temps au serveur web pour démarrer
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            // Lancer le serveur gRPC (bloquant) avec support hybrid
            start_multiplayer_server(
                neural_manager,
                qnet_manager,
                config.num_simulations,
                config.port,
                config.single_player,
                config.top_k,
            )
            .await?;
        }
    }
    Ok(())
}
