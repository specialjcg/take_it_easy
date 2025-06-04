// main.rs - Version corrigée avec les bonnes compatibilités
use clap::Parser;
use futures_util::StreamExt;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tch::nn::{self, OptimizerConfig};
use tch::Device;
use tokio::fs;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tonic::transport::Server;
use tonic_web::GrpcWebLayer;
use http::{header, Method, StatusCode};
use tonic::body::BoxBody;
use tonic::transport::Body;
use tower::{Layer, Service};
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;

// Imports Axum pour le serveur web
use axum::{
    response::Html,
    routing::get,
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::logging::setup_logging;
use crate::mcts_vs_human::play_mcts_vs_human;
use neural::policy_value_net::{PolicyNet, ValueNet};
use crate::training::session::train_and_evaluate;

// Services gRPC
use crate::services::session_service::SessionServiceImpl;
use crate::services::game_service::GameServiceImpl;
use crate::generated::takeiteasygame::v1::session_service_server::SessionServiceServer;
use crate::generated::takeiteasygame::v1::game_service_server::GameServiceServer;

#[cfg(test)]
mod test;

// Modules existants (inchangés)
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

// Nouveaux modules avec paradigme fonctionnel
mod generated;
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
    #[arg(long, value_enum, default_value = "mcts-vs-human")]
    mode: GameMode,

    /// Port pour le serveur gRPC Multiplayer
    #[arg(short = 'p', long, default_value_t = 50051)]
    port: u16,
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

// ============================================================================
// SERVEUR WEB POUR LES FICHIERS STATIQUES
// ============================================================================

async fn start_web_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/", get(|| async {
            Html(fs::read_to_string("web/index.html").await.unwrap_or_else(|_|
                r#"<!DOCTYPE html>
<html><head><title>Take It Easy</title></head>
<body>
<h1>🎮 Take It Easy - Multiplayer</h1>
<p>Place your frontend files in ./web/ directory</p>
<p>gRPC server is running on port 50051</p>
</body></html>"#.to_string()
            ))
        }))
        .nest_service("/static", ServeDir::new("web"))
        .fallback_service(ServeDir::new("web"))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        );

    let addr: SocketAddr = format!("0.0.0.0:{}", port + 1000).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
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
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, header::HeaderValue::from_static("*"));
            headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS, header::HeaderValue::from_static("GET, POST, OPTIONS"));
            headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, header::HeaderValue::from_static("content-type, x-grpc-web, x-user-agent, grpc-timeout, grpc-accept-encoding"));
            headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, header::HeaderValue::from_static("grpc-status, grpc-message"));

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
    policy_net: PolicyNet,
    value_net: ValueNet,
    num_simulations: usize,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    // Créer le session manager partagé
    let session_manager = Arc::new(crate::services::session_manager::new_session_manager());

    // Wrapper les réseaux de neurones pour le partage
    let policy_net_arc = Arc::new(Mutex::new(policy_net));
    let value_net_arc = Arc::new(Mutex::new(value_net));

    // Créer les services gRPC (avec les nouvelles méthodes gameplay)
    let session_service = SessionServiceImpl::new_with_manager(session_manager.clone()); // ← Modifié
    let game_service = GameServiceImpl::new(
        session_manager.clone(),
        policy_net_arc.clone(),
        value_net_arc.clone(),
        num_simulations
    );

    // Serveur gRPC UNIQUEMENT - plus de REST
    Server::builder()
        .accept_http1(true)
        .layer(SimpleCorsLayer)
        .layer(GrpcWebLayer::new())
        .add_service(SessionServiceServer::new(session_service))
        .add_service(GameServiceServer::new(game_service)) // ← Maintenant avec StartTurn et GetGameState
        .serve(addr)
        .await?;

    Ok(())
}

// ============================================================================
// FONCTION PRINCIPALE
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::parse();
    let model_path = "model_weights";
    setup_logging();
    // Initialize VarStore (votre système existant)
    let mut vs_policy = nn::VarStore::new(Device::Cpu);
    let mut vs_value = nn::VarStore::new(Device::Cpu);
    let input_dim = (5, 47, 1);
    let mut policy_net = PolicyNet::new(&vs_policy, input_dim);
    let mut value_net = ValueNet::new(&mut vs_value, input_dim);

    // Load weights if the model directory exists
    if Path::new(model_path).exists() {
        if let Err(e) = policy_net.load_model(&mut vs_policy, "model_weights/policy/policy.params")
        {
            log::error!("⚠️ Error loading PolicyNet: {:?}", e);        }

        if let Err(e) = value_net.load_model(&mut vs_value, "model_weights/value/value.params") {
            log::error!("⚠️ Error loading ValueNet: {:?}", e);        }
    } else {    }

    let mut optimizer_policy = nn::Adam::default().build(&vs_policy, 1e-3).unwrap();
    let mut optimizer_value = nn::Adam {
        wd: 1e-6,
        ..Default::default()
    }
        .build(&vs_value, 2e-4)
        .unwrap();

    // Match sur les modes
    match config.mode {
        GameMode::Training => {            let listener = TcpListener::bind("127.0.0.1:9000")
                .await
                .expect("Unable to bind WebSocket on port 9000 for training");
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

        GameMode::MctsVsHuman => {            let listener = TcpListener::bind("127.0.0.1:9001")
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
                policy_net,
                value_net,
                config.num_simulations,
                config.port,
            )
                .await?;
        }
    }    Ok(())
}