use axum::{
    extract::Json,
    response::{Html, Json as ResponseJson},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

// Structures pour l'API Web
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LaunchRequest {
    pub mode: String,
    pub command: String,
    pub options: Option<LaunchOptions>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LaunchOptions {
    pub rebuild: Option<bool>,
    pub simulations: Option<i32>,
    pub games: Option<i32>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ApiResponse {
    pub status: String,
    pub message: String,
}

// Configuration pour le serveur Web UI
#[derive(Debug, Clone)]
pub struct WebUiConfig {
    pub port: u16,
    pub host: String,
}

impl Default for WebUiConfig {
    fn default() -> Self {
        Self {
            port: 51051,
            host: "0.0.0.0".to_string(),
        }
    }
}

// Serveur Web UI principal
pub struct WebUiServer {
    config: WebUiConfig,
}

impl WebUiServer {
    pub fn new(config: WebUiConfig) -> Self {
        Self { config }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let app = self.create_router();
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        let listener = TcpListener::bind(addr).await?;

        log::info!(
            "🌐 Web UI server starting on http://localhost:{}",
            self.config.port
        );

        axum::serve(listener, app).await?;
        Ok(())
    }

    fn create_router(&self) -> Router {
        Router::new()
            .route("/", get(serve_index))
            .route("/api/status", get(api_status))
            .route("/api/launch-mode", post(api_launch_mode))
            .route("/api/stop-all", post(api_stop_all))
            .route("/api/logs", get(api_logs))
            .nest_service("/static", ServeDir::new("web"))
            .fallback_service(ServeDir::new("web"))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
    }
}

// Handlers - Real implementations migrated from main.rs
async fn serve_index() -> Html<String> {
    let index_content = tokio::fs::read_to_string("web/index.html")
        .await
        .unwrap_or_else(|_| {
            r#"<!DOCTYPE html>
<html><head><title>Take It Easy</title></head>
<body>
<h1>🎮 Take It Easy - Game Mode Selector</h1>
<p>Interface web pour lancer les modes de jeu</p>
<p>Files should be in ./web/ directory</p>
</body></html>"#
                .to_string()
        });

    Html(index_content)
}

async fn api_status() -> ResponseJson<ApiResponse> {
    ResponseJson(ApiResponse {
        status: "ready".to_string(),
        message: "Take It Easy server is running".to_string(),
    })
}

async fn api_launch_mode(
    Json(request): Json<LaunchRequest>,
) -> Result<ResponseJson<ApiResponse>, String> {
    log::info!("🚀 Launch request: mode={}", request.mode);

    // Check if rebuild is requested
    let should_rebuild = request
        .options
        .as_ref()
        .and_then(|opts| opts.rebuild)
        .unwrap_or(false);

    if should_rebuild {
        log::info!("🧹 Rebuild requested - performing cargo clean + build");
        let rebuild_result = std::process::Command::new("bash")
            .args(["-c", "cargo clean && cargo build --release"])
            .output()
            .map_err(|e| format!("Failed to execute rebuild: {}", e))?;

        if !rebuild_result.status.success() {
            let error_msg = String::from_utf8_lossy(&rebuild_result.stderr);
            log::error!("❌ Rebuild failed: {}", error_msg);
            return Err(format!("Rebuild failed: {}", error_msg));
        }
        log::info!("✅ Rebuild completed successfully");
    }

    match request.mode.as_str() {
        "single" => {
            // For single player mode, we need to launch a separate backend process
            // since the current process is in multiplayer mode
            let simulations = request
                .options
                .as_ref()
                .and_then(|opts| opts.simulations)
                .unwrap_or(300);

            let rebuild_option = if should_rebuild { "rebuild" } else { "" };
            let command = format!(
                "./launch_modes.sh single {} {} frontend",
                simulations, rebuild_option
            );

            log::info!("🤖 Launching single player mode: {}", command);

            let launch_result = std::process::Command::new("bash")
                .args(["-c", &command])
                .spawn();

            match launch_result {
                Ok(_) => {
                    log::info!("✅ Single player mode launched successfully");

                    // Give time for both backend and frontend to start
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                    let rebuild_msg = if should_rebuild { " (rebuilt)" } else { "" };
                    Ok(ResponseJson(ApiResponse {
                        status: "started".to_string(),
                        message: format!(
                            "Single player mode ready{} - Frontend: http://localhost:3000",
                            rebuild_msg
                        ),
                    }))
                }
                Err(e) => {
                    log::error!("❌ Failed to start single player mode: {}", e);
                    Err(format!("Failed to start single player mode: {}", e))
                }
            }
        }
        "multiplayer" => {
            // For multiplayer, just start the frontend since backend is already running
            let frontend_result = std::process::Command::new("bash")
                .args(["-c", "cd frontend && npm run dev > ../frontend.log 2>&1 &"])
                .spawn();

            match frontend_result {
                Ok(_) => {
                    log::info!("✅ Frontend launched successfully");

                    // Give frontend time to start
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                    let rebuild_msg = if should_rebuild { " (rebuilt)" } else { "" };
                    Ok(ResponseJson(ApiResponse {
                        status: "started".to_string(),
                        message: format!("Multiplayer mode ready{} - Backend: http://localhost:51051, Frontend: http://localhost:3000", rebuild_msg),
                    }))
                }
                Err(e) => {
                    log::error!("❌ Failed to start frontend: {}", e);
                    Err(format!("Failed to start frontend: {}", e))
                }
            }
        }
        "training" => {
            let rebuild_msg = if should_rebuild { " (rebuilt)" } else { "" };
            Ok(ResponseJson(ApiResponse {
                status: "started".to_string(),
                message: format!(
                    "Training mode{} uses console output - check terminal for progress",
                    rebuild_msg
                ),
            }))
        }
        _ => Err("Invalid mode".to_string()),
    }
}

#[cfg(not(test))]
async fn api_stop_all() -> ResponseJson<ApiResponse> {
    log::info!("🛑 Stop all requested");

    // Stop frontend processes
    let _ = std::process::Command::new("bash")
        .args(["-c", "pkill -f 'npm run dev' || true"])
        .output();

    // Stop any other game processes
    let _ = std::process::Command::new("bash")
        .args(["-c", "./launch_modes.sh stop"])
        .output();

    // Clean PID files
    let _ = std::process::Command::new("bash")
        .args(["-c", "rm -f .rust_game_pid .frontend_pid .rust_pid || true"])
        .output();

    ResponseJson(ApiResponse {
        status: "stopped".to_string(),
        message: "All processes stopped".to_string(),
    })
}

#[cfg(test)]
async fn api_stop_all() -> ResponseJson<ApiResponse> {
    log::info!("🛑 Stop all requested (test mode - mocked)");

    // In test mode, just return success without actually stopping processes
    ResponseJson(ApiResponse {
        status: "stopped".to_string(),
        message: "All processes stopped".to_string(),
    })
}

async fn api_logs() -> Html<String> {
    let mut logs_html = String::from(
        r#"<!DOCTYPE html>
<html><head><title>Take It Easy - Logs</title>
<style>
body { font-family: monospace; margin: 20px; }
pre { background: #f5f5f5; padding: 10px; border-radius: 5px; overflow: auto; }
h2 { color: #333; }
</style></head>
<body>"#,
    );

    // Read backend logs
    if let Ok(backend_logs) = tokio::fs::read_to_string("backend.log").await {
        logs_html.push_str(&format!("<h2>Backend Logs</h2><pre>{}</pre>", backend_logs));
    }

    // Read frontend logs
    if let Ok(frontend_logs) = tokio::fs::read_to_string("frontend.log").await {
        logs_html.push_str(&format!(
            "<h2>Frontend Logs</h2><pre>{}</pre>",
            frontend_logs
        ));
    }

    if !logs_html.contains("Backend Logs") && !logs_html.contains("Frontend Logs") {
        logs_html.push_str("<h2>No logs available</h2>");
    }

    logs_html.push_str("</body></html>");
    Html(logs_html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_ui_config_default() {
        let config = WebUiConfig::default();
        assert_eq!(config.port, 51051);
        assert_eq!(config.host, "0.0.0.0");
    }

    #[test]
    fn test_web_ui_server_creation() {
        let config = WebUiConfig::default();
        let server = WebUiServer::new(config);
        assert_eq!(server.config.port, 51051);
    }

    #[test]
    fn test_launch_request_serialization() {
        let request = LaunchRequest {
            mode: "single".to_string(),
            command: "test".to_string(),
            options: Some(LaunchOptions {
                rebuild: Some(true),
                simulations: Some(300),
                games: None,
            }),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: LaunchRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.mode, "single");
        assert_eq!(deserialized.options.unwrap().simulations, Some(300));
    }

    #[test]
    fn test_api_response_creation() {
        let response = ApiResponse {
            status: "ready".to_string(),
            message: "Test message".to_string(),
        };

        assert_eq!(response.status, "ready");
        assert_eq!(response.message, "Test message");
    }

    #[tokio::test]
    async fn test_api_status_endpoint() {
        let response = api_status().await;
        assert_eq!(response.0.status, "ready");
        assert_eq!(response.0.message, "Take It Easy server is running");
    }

    #[tokio::test]
    async fn test_api_stop_all_endpoint() {
        let response = api_stop_all().await;
        assert_eq!(response.0.status, "stopped");
        assert_eq!(response.0.message, "All processes stopped");
    }

    #[tokio::test]
    async fn test_api_logs_endpoint() {
        let response = api_logs().await;
        let content = response.0;
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Take It Easy - Logs"));
    }

    #[test]
    fn test_launch_request_training_mode() {
        let request = LaunchRequest {
            mode: "training".to_string(),
            command: "test".to_string(),
            options: None,
        };

        assert_eq!(request.mode, "training");
        assert!(request.options.is_none());
    }
}
