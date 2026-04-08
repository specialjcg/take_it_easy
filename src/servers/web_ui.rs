use axum::{
    extract::Json,
    response::{Html, Json as ResponseJson},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;

use crate::auth::{auth_router, AuthState};
use crate::recording::get_recorder;

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

/// Response for recording statistics
#[derive(Serialize, Debug, Clone)]
pub struct RecordingStats {
    pub enabled: bool,
    pub active_games: usize,
    pub recording_dir: String,
    pub today_file: Option<String>,
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
    auth_state: Option<Arc<AuthState>>,
}

impl WebUiServer {
    pub fn new(config: WebUiConfig) -> Self {
        Self {
            config,
            auth_state: None,
        }
    }

    /// Create server with authentication enabled
    pub fn with_auth(config: WebUiConfig, auth_state: Arc<AuthState>) -> Self {
        Self {
            config,
            auth_state: Some(auth_state),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let app = self.create_router();
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        let listener = TcpListener::bind(addr).await?;

        log::info!(
            "🌐 Web UI server starting on http://localhost:{}",
            self.config.port
        );

        if self.auth_state.is_some() {
            log::info!("🔐 Authentication enabled at /auth/*");
        }

        axum::serve(listener, app).await?;
        Ok(())
    }

    fn create_router(&self) -> Router {
        let mut router = Router::new()
            .route("/", get(serve_index))
            .route("/api/status", get(api_status))
            .route("/api/launch-mode", post(api_launch_mode))
            .route("/api/stop-all", post(api_stop_all))
            .route("/api/logs", get(api_logs))
            .route("/api/recording-stats", get(api_recording_stats))
            .route("/api/player-stats", get(api_player_stats))
            .route("/stats", get(serve_stats_page));

        // Add auth routes if auth is enabled
        if let Some(auth_state) = &self.auth_state {
            router = router.nest("/auth", auth_router(Arc::clone(auth_state)));
            log::debug!("Auth routes mounted at /auth/*");
        }

        router
            .nest_service("/static", ServeDir::new("web"))
            .fallback_service(ServeDir::new("web"))
            .layer(
                CorsLayer::new()
                    .allow_origin(AllowOrigin::list([
                        "https://eigenplay.com".parse().unwrap(),
                        "https://www.eigenplay.com".parse().unwrap(),
                        "https://takeitasy.mooo.com".parse().unwrap(),
                    ]))
                    .allow_methods([
                        http::Method::GET,
                        http::Method::POST,
                        http::Method::OPTIONS,
                    ])
                    .allow_headers([
                        http::header::CONTENT_TYPE,
                        http::header::AUTHORIZATION,
                    ])
                    .allow_credentials(true),
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

/// Get recording statistics
async fn api_recording_stats() -> ResponseJson<RecordingStats> {
    let (enabled, active_games) = match get_recorder() {
        Some(recorder) => (recorder.is_enabled(), recorder.active_game_count()),
        None => (false, 0),
    };

    // Get today's recording file
    let today = chrono::Utc::now().format("%Y%m%d").to_string();
    let recording_dir = "data/recorded_games".to_string();
    let today_file = format!("{}/games_{}.csv", recording_dir, today);
    let today_file_exists = std::path::Path::new(&today_file).exists();

    ResponseJson(RecordingStats {
        enabled,
        active_games,
        recording_dir,
        today_file: if today_file_exists {
            Some(today_file)
        } else {
            None
        },
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

// ─── Player Stats API ──────────────────────────────────────────

#[derive(Serialize, Debug)]
struct PlayerGameResult {
    date: String,
    human_score: i32,
    ai_score: i32,
    human_won: bool,
}

#[derive(Serialize, Debug)]
struct PlayerSummary {
    username: String,
    games_played: usize,
    wins: usize,
    winrate: f64,
    avg_score: f64,
    best_score: i32,
    avg_ai_score: f64,
    games: Vec<PlayerGameResult>,
}

#[derive(Serialize, Debug)]
struct StatsResponse {
    players: Vec<PlayerSummary>,
    total_games: usize,
}

/// Parse all recorded game CSVs and return per-player stats.
fn load_all_game_stats() -> StatsResponse {
    use std::io::BufRead;

    let recording_dir = std::env::var("RECORDING_DIR")
        .unwrap_or_else(|_| "/opt/takeitasy/data/recorded_games".to_string());
    log::info!("Loading stats from: {} (exists={})", recording_dir, std::path::Path::new(&recording_dir).exists());
    let dir_entries: Vec<_> = std::fs::read_dir(&recording_dir)
        .map(|rd| rd.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();
    let csv_count = dir_entries.iter().filter(|e| e.path().extension().map_or(false, |ext| ext == "csv")).count();
    log::info!("Found {} dir entries, {} CSV files", dir_entries.len(), csv_count);

    // Load user mapping from auth.db
    let user_map: HashMap<String, String> = {
        let db_path = std::env::var("AUTH_DB_PATH")
            .unwrap_or_else(|_| "/opt/takeitasy/data/auth.db".to_string());
        let mut map = HashMap::new();
        if let Ok(conn) = rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
            if let Ok(mut stmt) = conn.prepare("SELECT id, username FROM users") {
                let rows = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                });
                if let Ok(rows) = rows {
                    for row in rows.flatten() {
                        map.insert(row.0, row.1);
                    }
                }
            }
        }
        log::info!("Loaded {} users from auth.db", map.len());
        map
    };

    let mut player_games: HashMap<String, Vec<PlayerGameResult>> = HashMap::new();
    let mut total_games = 0usize;

    // Read all CSV files (reuse dir_entries from debug above)
    for entry in &dir_entries {
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "csv") {
            continue;
        }
        let date = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .replace("games_", "");

        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let reader = std::io::BufReader::new(file);

        // Track games: game_id -> {player_type -> (last_turn, final_score, human_won, player_id)}
        let mut games: HashMap<String, HashMap<String, (usize, i32, bool, String)>> = HashMap::new();

        for line in reader.lines().skip(1) {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() < 28 {
                continue;
            }

            let game_id = fields[0].to_string();
            let turn: usize = fields[1].parse().unwrap_or(0);
            let player_type = fields[2].to_string();
            let final_score: i32 = fields[26].parse().unwrap_or(0);
            let human_won: bool = fields[27].trim_end().parse::<i32>().unwrap_or(0) == 1;
            // New column: player_id (index 28, may not exist in old CSVs)
            let player_id = fields.get(28).unwrap_or(&"").trim().to_string();

            let game = games.entry(game_id).or_default();
            let entry = game.entry(player_type).or_insert((0, 0, false, String::new()));
            if turn >= entry.0 {
                *entry = (turn, final_score, human_won, player_id);
            }
        }

        // Process completed games
        for (_gid, players) in &games {
            let human = players.iter().find(|(k, _)| k.as_str() != "MCTS" && k.as_str() != "mcts_ai");
            let ai = players.get("MCTS").or_else(|| players.get("mcts_ai"));

            if let (Some((_ptype, (_, h_score, h_won, player_id))), Some((_, ai_score, _, _))) = (human, ai) {
                if *h_score < 30 && *ai_score < 30 {
                    continue;
                }
                total_games += 1;

                // Resolve username: player_id -> auth.db -> username, fallback to "Unknown"
                let username = if !player_id.is_empty() {
                    user_map.get(player_id).cloned().unwrap_or_else(|| "Unknown".to_string())
                } else {
                    "Unknown".to_string()
                };

                let result = PlayerGameResult {
                    date: date.clone(),
                    human_score: *h_score,
                    ai_score: *ai_score,
                    human_won: *h_won,
                };
                player_games.entry(username).or_default().push(result);
            }
        }
    }

    // Build summaries
    let mut players: Vec<PlayerSummary> = player_games
        .into_iter()
        .filter(|(name, _)| name != "MCTS" && name != "mcts_ai")
        .map(|(username, games)| {
            let games_played = games.len();
            let wins = games.iter().filter(|g| g.human_won).count();
            let avg_score = if games_played > 0 {
                games.iter().map(|g| g.human_score as f64).sum::<f64>() / games_played as f64
            } else { 0.0 };
            let best_score = games.iter().map(|g| g.human_score).max().unwrap_or(0);
            let avg_ai_score = if games_played > 0 {
                games.iter().map(|g| g.ai_score as f64).sum::<f64>() / games_played as f64
            } else { 0.0 };
            let winrate = if games_played > 0 { wins as f64 / games_played as f64 * 100.0 } else { 0.0 };

            PlayerSummary {
                username,
                games_played,
                wins,
                winrate,
                avg_score,
                best_score,
                avg_ai_score,
                games,
            }
        })
        .collect();

    players.sort_by(|a, b| b.games_played.cmp(&a.games_played));

    StatsResponse {
        players,
        total_games,
    }
}

async fn api_player_stats() -> ResponseJson<StatsResponse> {
    ResponseJson(load_all_game_stats())
}

async fn serve_stats_page() -> Html<String> {
    let html = tokio::fs::read_to_string("web/stats.html")
        .await
        .or_else(|_| std::fs::read_to_string("frontend-elm/public/stats.html"))
        .unwrap_or_else(|_| "<h1>Stats page not found</h1><p>Place stats.html in web/ or frontend-elm/public/</p>".to_string());
    Html(html)
}
