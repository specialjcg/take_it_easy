use crate::generated::takeiteasygame::v1::game_service_server::GameServiceServer;
use crate::generated::takeiteasygame::v1::session_service_server::SessionServiceServer;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::qvalue_net::QValueNet;
use crate::services::game_service::GameServiceImpl;
use crate::services::session_manager;
use crate::services::session_service::SessionServiceImpl;
use http::{header, Method, StatusCode};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::try_join;
use tonic::body::Body as TonicBody;
use tonic::transport::Server;
use tonic_web::GrpcWebLayer;
use tower::{Layer, Service};

#[derive(Debug, Clone)]
pub struct GrpcConfig {
    pub port: u16,
    pub web_port: u16,
    pub host: String,
    pub enable_web_layer: bool,
    pub enable_cors: bool,
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

impl<S> Service<http::Request<TonicBody>> for SimpleCors<S>
where
    S: Service<http::Request<TonicBody>, Response = http::Response<TonicBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<TonicBody>) -> Self::Future {
        let mut inner = self.inner.clone();

        Box::pin(async move {
            if req.method() == Method::OPTIONS {
                let response = http::Response::builder()
                    .status(StatusCode::OK)
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, OPTIONS")
                    .header(
                        header::ACCESS_CONTROL_ALLOW_HEADERS,
                        "content-type, x-grpc-web, x-user-agent, grpc-timeout, grpc-accept-encoding",
                    )
                    .header(header::ACCESS_CONTROL_MAX_AGE, "86400")
                    .body(TonicBody::empty())
                    .expect("CORS preflight response construction should never fail with valid static headers");
                return Ok(response);
            }

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

#[derive(Clone)]
pub struct SimpleCorsLayer;

impl Default for SimpleCorsLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleCorsLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for SimpleCorsLayer {
    type Service = SimpleCors<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SimpleCors::new(inner)
    }
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            port: 50051,
            web_port: 8080,
            host: "0.0.0.0".to_string(),
            enable_web_layer: true,
            enable_cors: true,
        }
    }
}

pub struct GrpcServer {
    config: GrpcConfig,
    session_manager: Arc<session_manager::SessionManager>,
    policy_net: Arc<tokio::sync::Mutex<PolicyNet>>,
    value_net: Arc<tokio::sync::Mutex<ValueNet>>,
    qvalue_net: Option<Arc<tokio::sync::Mutex<QValueNet>>>,
    num_simulations: usize,
    single_player: bool,
    top_k: usize,
}

impl GrpcServer {
    pub fn new(
        config: GrpcConfig,
        policy_net: PolicyNet,
        value_net: ValueNet,
        num_simulations: usize,
        single_player: bool,
    ) -> Self {
        let session_manager = Arc::new(session_manager::new_session_manager());
        let policy_net_arc = Arc::new(tokio::sync::Mutex::new(policy_net));
        let value_net_arc = Arc::new(tokio::sync::Mutex::new(value_net));

        Self {
            config,
            session_manager,
            policy_net: policy_net_arc,
            value_net: value_net_arc,
            qvalue_net: None,
            num_simulations,
            single_player,
            top_k: 6,
        }
    }

    /// Create a new GrpcServer with Q-Net hybrid MCTS (recommended for best performance)
    pub fn new_hybrid(
        config: GrpcConfig,
        policy_net: PolicyNet,
        value_net: ValueNet,
        qvalue_net: QValueNet,
        num_simulations: usize,
        single_player: bool,
        top_k: usize,
    ) -> Self {
        let session_manager = Arc::new(session_manager::new_session_manager());
        let policy_net_arc = Arc::new(tokio::sync::Mutex::new(policy_net));
        let value_net_arc = Arc::new(tokio::sync::Mutex::new(value_net));
        let qvalue_net_arc = Arc::new(tokio::sync::Mutex::new(qvalue_net));

        Self {
            config,
            session_manager,
            policy_net: policy_net_arc,
            value_net: value_net_arc,
            qvalue_net: Some(qvalue_net_arc),
            num_simulations,
            single_player,
            top_k,
        }
    }

    /// Get a reference to the server configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &GrpcConfig {
        &self.config
    }

    /// Initialize single-player session if needed - DÉSACTIVÉ pour le mode sélection frontend
    async fn init_single_player_session(&self) -> Result<(), Box<dyn std::error::Error>> {
        // ✅ DÉSACTIVÉ: Les sessions sont maintenant créées via le frontend avec mode sélectionné
        log::info!("🎮 Auto-création de sessions désactivée - utiliser le frontend pour sélectionner le mode");
        Ok(())
    }

    /// Start the gRPC server
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let grpc_addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        let grpc_web_addr: SocketAddr =
            format!("{}:{}", self.config.host, self.config.web_port).parse()?;

        // Initialize single-player session if needed
        self.init_single_player_session().await?;

        // Create gRPC services
        let session_service = SessionServiceImpl::new_with_manager_and_mode(
            self.session_manager.clone(),
            self.single_player,
        );
        let game_service = GameServiceImpl::new_with_qnet(
            self.session_manager.clone(),
            self.policy_net.clone(),
            self.value_net.clone(),
            self.qvalue_net.clone(),
            self.num_simulations,
            self.top_k,
        );

        // Log server startup info
        let mcts_mode = if self.qvalue_net.is_some() {
            format!(
                "HYBRID Q-Net (top-{}, {} sims)",
                self.top_k, self.num_simulations
            )
        } else {
            format!("CNN ({} sims)", self.num_simulations)
        };

        if self.single_player {
            log::info!(
                "🤖 Mode SINGLE-PLAYER démarré : 1 joueur vs MCTS {}",
                mcts_mode
            );
        } else {
            log::info!(
                "👥 Mode MULTIJOUEUR démarré : Plusieurs joueurs + MCTS {}",
                mcts_mode
            );
        }
        let web_layer_info = if self.config.enable_web_layer {
            format!(
                "enabled on port {} (CORS {})",
                self.config.web_port,
                if self.config.enable_cors {
                    "default tonic-web"
                } else {
                    "disabled"
                }
            )
        } else {
            "disabled".to_string()
        };
        log::info!(
            "🔗 gRPC server starting on {}:{}, web layer {}",
            self.config.host,
            self.config.port,
            web_layer_info
        );

        let grpc_session_service = session_service.clone();
        let grpc_game_service = game_service.clone();

        let grpc_server = Server::builder()
            .add_service(SessionServiceServer::new(grpc_session_service))
            .add_service(GameServiceServer::new(grpc_game_service))
            .serve(grpc_addr);

        if self.config.enable_web_layer {
            let web_server: Pin<
                Box<dyn Future<Output = Result<(), tonic::transport::Error>> + Send>,
            > = if self.config.enable_cors {
                Box::pin(
                    Server::builder()
                        .accept_http1(true)
                        .layer(SimpleCorsLayer::new())
                        .layer(GrpcWebLayer::new())
                        .add_service(SessionServiceServer::new(session_service))
                        .add_service(GameServiceServer::new(game_service))
                        .serve(grpc_web_addr),
                )
            } else {
                Box::pin(
                    Server::builder()
                        .accept_http1(true)
                        .layer(GrpcWebLayer::new())
                        .add_service(SessionServiceServer::new(session_service))
                        .add_service(GameServiceServer::new(game_service))
                        .serve(grpc_web_addr),
                )
            };

            try_join!(grpc_server, web_server)?;
        } else {
            grpc_server.await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_config_default() {
        let config = GrpcConfig::default();
        assert_eq!(config.port, 50051);
        assert_eq!(config.web_port, 8080);
        assert_eq!(config.host, "0.0.0.0");
        assert!(config.enable_web_layer);
        assert!(config.enable_cors);
    }

    #[test]
    fn test_grpc_config_custom() {
        let config = GrpcConfig {
            port: 8080,
            web_port: 18080,
            host: "127.0.0.1".to_string(),
            enable_web_layer: false,
            enable_cors: false,
        };
        assert_eq!(config.port, 8080);
        assert_eq!(config.web_port, 18080);
        assert_eq!(config.host, "127.0.0.1");
        assert!(!config.enable_web_layer);
        assert!(!config.enable_cors);
    }

    #[test]
    fn test_grpc_server_creation() {
        use crate::neural::manager::NNArchitecture;
        use crate::neural::policy_value_net::PolicyNet;
        use crate::neural::policy_value_net::ValueNet;
        use tch::{nn, Device};

        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (5, 47, 1);
        let policy_net = PolicyNet::new(&vs, input_dim, NNArchitecture::Cnn);
        let value_net = ValueNet::new(&vs, input_dim, NNArchitecture::Cnn);

        let config = GrpcConfig::default();
        let server = GrpcServer::new(config, policy_net, value_net, 300, true);
        assert_eq!(server.config().port, 50051);
        assert_eq!(server.config().web_port, 8080);
        assert_eq!(server.config().host, "0.0.0.0");
        assert!(server.single_player);
        assert_eq!(server.num_simulations, 300);
    }

    #[test]
    fn test_grpc_server_config_access() {
        use crate::neural::manager::NNArchitecture;
        use crate::neural::policy_value_net::PolicyNet;
        use crate::neural::policy_value_net::ValueNet;
        use tch::{nn, Device};

        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (5, 47, 1);
        let policy_net = PolicyNet::new(&vs, input_dim, NNArchitecture::Cnn);
        let value_net = ValueNet::new(&vs, input_dim, NNArchitecture::Cnn);

        let config = GrpcConfig {
            port: 9000,
            web_port: 19000,
            host: "localhost".to_string(),
            enable_web_layer: true,
            enable_cors: true,
        };

        let server = GrpcServer::new(config, policy_net, value_net, 500, false);
        let server_config = server.config();
        assert_eq!(server_config.port, 9000);
        assert_eq!(server_config.web_port, 19000);
        assert_eq!(server_config.host, "localhost");
        assert!(server_config.enable_web_layer);
        assert!(server_config.enable_cors);
    }
}
