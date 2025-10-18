// main.rs - Version corrigée avec les bonnes compatibilités
use clap::Parser;
use flexi_logger::Logger;
use http::{header, Method, StatusCode};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::net::TcpListener;
use tonic::body::BoxBody;
use tonic::transport::Body;
use tower::{Layer, Service};

use crate::neural::transformer::training::TransformerSample;
use crate::neural::{NeuralConfig, NeuralManager};
use crate::training::session::{train_and_evaluate, train_and_evaluate_offline};
use tch::{Kind, Tensor};

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
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum GameMode {
    /// Mode entraînement normal
    Training,
    /// Mode multijoueur
    Multiplayer,
    /// Mode entraînement du Transformer
    TransformerTraining,
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
// SIMPLE CORS LAYER FOR WEB SERVICES
// ============================================================================
#[derive(Clone)]
pub struct SimpleCors<S> {
    inner: S,
}

impl<S> SimpleCors<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S> Service<http::Request<Body>> for SimpleCors<S>
where
    S: Service<http::Request<Body>, Response = http::Response<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Handle preflight OPTIONS requests
            if req.method() == Method::OPTIONS {
                let response = http::Response::builder()
                    .status(StatusCode::OK)
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, OPTIONS")
                    .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type, x-grpc-web, x-user-agent, grpc-timeout, grpc-accept-encoding")
                    .header(header::ACCESS_CONTROL_MAX_AGE, "86400")
                    .body(BoxBody::default())
                    .unwrap();

                return Ok(response);
            }

            // Process normal request and add CORS headers to response
            let mut response = inner.call(req).await?;

            let headers = response.headers_mut();
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                header::HeaderValue::from_static("*"),
            );
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                header::HeaderValue::from_static("GET, POST, OPTIONS"),
            );
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                header::HeaderValue::from_static(
                    "content-type, x-grpc-web, x-user-agent, grpc-timeout, grpc-accept-encoding",
                ),
            );
            headers.insert(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                header::HeaderValue::from_static("grpc-status, grpc-message"),
            );

            Ok(response)
        })
    }
}

// Layer pour le middleware
#[derive(Clone)]
pub struct SimpleCorsLayer;

impl<S> Layer<S> for SimpleCorsLayer {
    type Service = SimpleCors<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SimpleCors::new(inner)
    }
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
        input_dim: (5, 5, 5), // Bronze GNN: 5 channels × 5×5 spatial grid
        model_path: "model_weights".to_string(),
        policy_lr: 1e-3,
        value_lr: 2e-4,
        value_wd: 1e-6,
        ..Default::default()
    };

    let neural_manager = NeuralManager::with_config(neural_config)?;

    // Match sur les modes
    match config.mode {
        GameMode::Training => {
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
        GameMode::TransformerTraining => {
            use crate::neural::transformer::training::TransformerTrainer;
            use crate::neural::transformer::TransformerConfig;
            use tch::Device;
            // Configs par défaut, à adapter si besoin
            let transformer_config = TransformerConfig::default();
            let training_config = crate::neural::transformer::training::TrainingConfig::default();
            let device = Device::cuda_if_available();
            let mut trainer = TransformerTrainer::new(transformer_config, training_config, device);
            let weights_dir = Path::new("transformer_weights");
            if let Err(e) = trainer.load_weights(weights_dir) {
                log::warn!(
                    "[Transformer] Chargement des poids échoué ({:?}): {:?}",
                    weights_dir,
                    e
                );
            }
            // Utilisation de fichiers dédiés pour le Transformer si présents, sinon fallback sur les fichiers MCTS
            let states_path = if std::path::Path::new("game_data_states_transformer.pt").exists() {
                "game_data_states_transformer.pt"
            } else {
                "game_data_states.pt"
            };
            let policy_raw_path =
                if std::path::Path::new("game_data_policy_raw_transformer.pt").exists() {
                    "game_data_policy_raw_transformer.pt"
                } else if std::path::Path::new("game_data_policy_transformer.pt").exists() {
                    "game_data_policy_transformer.pt"
                } else if std::path::Path::new("game_data_positions_transformer.pt").exists() {
                    "game_data_positions_transformer.pt"
                } else if std::path::Path::new("game_data_policy_raw.pt").exists() {
                    "game_data_policy_raw.pt"
                } else if std::path::Path::new("game_data_policy.pt").exists() {
                    "game_data_policy.pt"
                } else {
                    "game_data_positions.pt"
                };

            let policy_boosted_path =
                if std::path::Path::new("game_data_policy_boosted_transformer.pt").exists() {
                    Some("game_data_policy_boosted_transformer.pt")
                } else if std::path::Path::new("game_data_policy_boosted.pt").exists() {
                    Some("game_data_policy_boosted.pt")
                } else {
                    None
                };

            let boosts_path = if std::path::Path::new("game_data_boosts_transformer.pt").exists() {
                Some("game_data_boosts_transformer.pt")
            } else if std::path::Path::new("game_data_boosts.pt").exists() {
                Some("game_data_boosts.pt")
            } else {
                None
            };
            let subscores_path =
                if std::path::Path::new("game_data_subscores_transformer.pt").exists() {
                    "game_data_subscores_transformer.pt"
                } else {
                    "game_data_subscores.pt"
                };
            let data = load_transformer_training_data(
                states_path,
                policy_raw_path,
                policy_boosted_path,
                boosts_path,
                subscores_path,
                config.num_games,
            );
            println!(
                "[Transformer] Lancement de l'entraînement sur {} parties...",
                data.len()
            );
            trainer.train(data).unwrap();
            if let Err(e) = trainer.save_weights(weights_dir) {
                log::error!(
                    "[Transformer] Sauvegarde des poids échouée ({:?}): {:?}",
                    weights_dir,
                    e
                );
            }
        }
    }
    Ok(())
}

/// Charge les données d'entraînement pour le Transformer à partir des fichiers .pt, limité à num_games exemples
fn load_transformer_training_data(
    states_path: &str,
    policy_raw_path: &str,
    policy_boosted_path: Option<&str>,
    boosts_path: Option<&str>,
    subscores_path: &str,
    num_games: usize,
) -> Vec<TransformerSample> {
    let state_tensor = Tensor::load(states_path).expect("Failed to load states");
    let policy_raw_tensor = Tensor::load(policy_raw_path)
        .unwrap_or_else(|e| panic!("Failed to load policy raw {}: {:?}", policy_raw_path, e));
    let policy_boosted_tensor = policy_boosted_path
        .and_then(|path| Path::new(path).exists().then(|| Tensor::load(path).ok()))
        .flatten()
        .unwrap_or_else(|| policy_raw_tensor.shallow_clone());
    let boosts_tensor = boosts_path
        .and_then(|path| Path::new(path).exists().then(|| Tensor::load(path).ok()))
        .flatten()
        .unwrap_or_else(|| {
            Tensor::zeros(
                [policy_raw_tensor.size()[0], 1],
                (Kind::Float, state_tensor.device()),
            )
        });
    let subscore_tensor = Tensor::load(subscores_path).expect("Failed to load subscores");

    const POLICY_OUTPUTS: i64 = 19;

    let policy_raw_tensor = if policy_raw_tensor.dim() >= 2 && policy_raw_tensor.size()[1] > 1 {
        policy_raw_tensor.to_kind(Kind::Float)
    } else {
        positions_to_one_hot_tensor(&policy_raw_tensor, POLICY_OUTPUTS)
    };

    let policy_boosted_tensor =
        if policy_boosted_tensor.dim() >= 2 && policy_boosted_tensor.size()[1] > 1 {
            policy_boosted_tensor.to_kind(Kind::Float)
        } else {
            positions_to_one_hot_tensor(&policy_boosted_tensor, POLICY_OUTPUTS)
        };

    let boosts_tensor = if boosts_tensor.dim() == 2 {
        boosts_tensor.to_kind(Kind::Float)
    } else {
        boosts_tensor.to_kind(Kind::Float).view([-1, 1])
    };

    let num_samples = state_tensor.size()[0].min(num_games as i64);
    let mut data = Vec::with_capacity(num_samples as usize);
    for i in 0..num_samples {
        let input = state_tensor.get(i);
        let target_policy_raw = policy_raw_tensor.get(i);
        let target_policy_boosted = policy_boosted_tensor.get(i);
        let target_value = subscore_tensor.get(i);
        let boost = boosts_tensor.get(i).double_value(&[]) as f32;
        if i == 0 {
            println!(
                "[Transformer] Sample state tensor shape: {:?}",
                input.size()
            );
            println!(
                "[Transformer] Sample policy tensor shape: {:?}",
                target_policy_raw.size()
            );
            println!(
                "[Transformer] Sample value tensor shape: {:?}",
                target_value.size()
            );
        }
        data.push(TransformerSample {
            state: input,
            policy_raw: target_policy_raw,
            policy_boosted: target_policy_boosted,
            value: target_value,
            boost_intensity: boost,
        });
    }
    data
}

fn positions_to_one_hot_tensor(tensor: &Tensor, vocab: i64) -> Tensor {
    let samples = tensor.size()[0];
    let mut distributions = Vec::with_capacity(samples as usize);
    for idx in 0..samples {
        let mut position = tensor.get(idx).int64_value(&[]);
        if position < 0 {
            position = 0;
        }
        if position >= vocab {
            position = vocab - 1;
        }
        let mut distribution = vec![0f32; vocab as usize];
        distribution[position as usize] = 1.0;
        distributions.push(Tensor::from_slice(&distribution));
    }
    Tensor::stack(&distributions, 0)
}
