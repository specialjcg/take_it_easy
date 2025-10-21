// main.rs - Version corrigée avec les bonnes compatibilités
use clap::Parser;
use flexi_logger::Logger;
use http::{header, Method, StatusCode};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::net::TcpListener;
use tonic::body::BoxBody;
use tonic::transport::Body;
use tower::{Layer, Service};

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
        input_dim: (8, 5, 5), // Enhanced feature stack: 8 channels × 5×5 spatial grid
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
    }
    Ok(())
}
