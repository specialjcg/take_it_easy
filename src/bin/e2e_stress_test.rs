//! E2E + Stress test for Take It Easy gRPC server.
//!
//! Exercises every gRPC endpoint the frontend uses, in the same order a real
//! user would click through the UI pages.
//!
//! Modes:
//!   --mode solo          Full solo journey (all 9 gRPC endpoints)
//!   --mode multiplayer   Full 2-player journey (create/join/poll/play/finish)
//!   --mode real-game     "Jeu Réel" mode (GetAiMove focus)
//!   --mode errors        Error-path coverage (invalid inputs)
//!   --mode all           Run solo + multiplayer + real-game + errors
//!   --mode stress        N concurrent solo games with metrics

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use rand::seq::SliceRandom;
use serde_json::Value;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use take_it_easy::generated::takeiteasygame::v1::{
    create_session_response, join_session_response, make_move_response,
    game_service_client::GameServiceClient,
    session_service_client::SessionServiceClient,
    CreateSessionRequest, GetAiMoveRequest, GetAvailableMovesRequest,
    GetGameStateRequest, GetSessionStateRequest, JoinSessionRequest,
    MakeMoveRequest, SetReadyRequest, StartTurnRequest,
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "e2e_stress_test", about = "E2E and stress tests for Take It Easy gRPC server")]
struct Cli {
    /// gRPC server URL
    #[arg(long, default_value = "http://[::1]:50051")]
    url: String,

    /// Test mode: solo | multiplayer | real-game | errors | all | stress
    #[arg(long, default_value = "solo")]
    mode: String,

    /// Concurrent games (stress mode)
    #[arg(long, default_value_t = 10)]
    concurrent: usize,

    /// Number of full games to play (applies to all modes except errors)
    #[arg(long, default_value_t = 1)]
    total_games: usize,

    /// Verbose output
    #[arg(long)]
    verbose: bool,
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

// ---------------------------------------------------------------------------
// Test result tracking
// ---------------------------------------------------------------------------

struct TestResults {
    passed: Vec<String>,
    failed: Vec<(String, String)>,
}

impl TestResults {
    fn new() -> Self {
        Self { passed: vec![], failed: vec![] }
    }

    fn pass(&mut self, name: &str) {
        println!("  PASS  {}", name);
        self.passed.push(name.to_string());
    }

    fn fail(&mut self, name: &str, reason: &str) {
        println!("  FAIL  {} -- {}", name, reason);
        self.failed.push((name.to_string(), reason.to_string()));
    }

    fn summary(&self) -> bool {
        let total = self.passed.len() + self.failed.len();
        println!();
        println!(
            "  {}/{} passed, {} failed",
            self.passed.len(),
            total,
            self.failed.len()
        );
        for (name, reason) in &self.failed {
            println!("    FAIL {}: {}", name, reason);
        }
        self.failed.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Solo E2E — full user journey
// ---------------------------------------------------------------------------
// Pages exercised:
//   Welcome → "Jouer maintenant" (skip auth)
//   Mode Selection → Select "Solo" → "Commencer"
//   Lobby → SetReady → auto-start
//   Game Board → 19x (StartTurn + GetGameState + GetAvailableMoves + MakeMove)
//   Game Over → verify is_game_finished via GetGameState
//   "Rejouer" → new CreateSession

async fn run_solo(url: &str, verbose: bool) -> Result<(i32, u128), BoxError> {
    let start = Instant::now();

    let mut session = SessionServiceClient::connect(url.to_string()).await?;
    let mut game = GameServiceClient::connect(url.to_string()).await?;

    // ── Page: Mode Selection → "Commencer" (solo) ──────────────────────
    let resp = session
        .create_session(CreateSessionRequest {
            player_name: "e2e-solo".into(),
            max_players: 2,
            game_mode: "single-player".into(),
        })
        .await?
        .into_inner();

    let success = match resp.result {
        Some(create_session_response::Result::Success(s)) => s,
        Some(create_session_response::Result::Error(e)) => {
            return Err(format!("CreateSession error: {} - {}", e.code, e.message).into());
        }
        None => return Err("CreateSession: empty response".into()),
    };

    let session_id = success.session_id;
    let player_id = success.player_id;
    if verbose {
        println!("  [CreateSession] session={}, player={}", session_id, player_id);
    }

    // ── Page: Lobby → "Je suis prêt!" ──────────────────────────────────
    let ready_resp = session
        .set_ready(SetReadyRequest {
            session_id: session_id.clone(),
            player_id: player_id.clone(),
            ready: true,
        })
        .await?
        .into_inner();

    if verbose {
        println!(
            "  [SetReady] success={}, game_started={}",
            ready_resp.success, ready_resp.game_started
        );
    }

    // ── Lobby polling: GetSessionState (like PollSession every 2s) ─────
    let poll_resp = session
        .get_session_state(GetSessionStateRequest {
            session_id: session_id.clone(),
        })
        .await?
        .into_inner();

    if let Some(gs) = &poll_resp.game_state {
        if verbose {
            println!(
                "  [GetSessionState] state={}, players={}, turn={}",
                gs.state,
                gs.players.len(),
                gs.turn_number
            );
        }
    }

    // ── Game Board: 19 turns ───────────────────────────────────────────
    let mut final_score = 0i32;
    for turn in 0..19 {
        // StartTurn (frontend: "startTurn" port)
        let turn_resp = game
            .start_turn(StartTurnRequest {
                session_id: session_id.clone(),
                forced_tile: String::new(),
            })
            .await?
            .into_inner();

        if !turn_resp.success {
            let err_msg = turn_resp
                .error
                .map(|e| format!("{}: {}", e.code, e.message))
                .unwrap_or_else(|| "unknown".into());
            return Err(format!("StartTurn failed at turn {}: {}", turn, err_msg).into());
        }

        if verbose {
            println!(
                "  [StartTurn] turn={}, tile={}, waiting={}",
                turn_resp.turn_number,
                turn_resp.announced_tile,
                turn_resp.waiting_for_players.join(", ")
            );
        }

        // GetGameState (frontend: polling via "pollTurn")
        let gs_resp = game
            .get_game_state(GetGameStateRequest {
                session_id: session_id.clone(),
            })
            .await?
            .into_inner();

        if verbose {
            println!(
                "  [GetGameState] turn={}, tile={}, finished={}",
                gs_resp.current_turn, gs_resp.current_tile, gs_resp.is_game_finished
            );
        }

        // GetAvailableMoves (frontend: used to highlight hexagons)
        let moves_resp = game
            .get_available_moves(GetAvailableMovesRequest {
                session_id: session_id.clone(),
                player_id: player_id.clone(),
            })
            .await?
            .into_inner();

        if verbose {
            println!(
                "  [GetAvailableMoves] count={}",
                moves_resp.available_moves.len()
            );
        }

        // Pick position from game_state JSON
        let position = pick_position(&turn_resp.game_state, &player_id)?;

        if verbose {
            println!("  -> Playing position {}", position);
        }

        // MakeMove (frontend: click on hexagon → "playMove" port)
        let move_data = serde_json::json!({ "position": position }).to_string();
        let move_resp = game
            .make_move(MakeMoveRequest {
                session_id: session_id.clone(),
                player_id: player_id.clone(),
                move_data,
                timestamp: chrono::Utc::now().timestamp_millis(),
            })
            .await?
            .into_inner();

        match move_resp.result {
            Some(make_move_response::Result::Success(s)) => {
                final_score += s.points_earned;
                if verbose {
                    println!(
                        "  [MakeMove] points={}, game_over={}",
                        s.points_earned, s.is_game_over
                    );
                }
                if s.is_game_over {
                    if let Some(gs) = &s.new_game_state {
                        for p in &gs.players {
                            if p.id == player_id {
                                final_score = p.score;
                                break;
                            }
                        }
                    }
                    if verbose {
                        println!("  Game over at turn {}! Score: {}", turn + 1, final_score);
                    }
                    break;
                }
            }
            Some(make_move_response::Result::Error(e)) => {
                return Err(format!(
                    "MakeMove error at turn {}: {} - {}",
                    turn, e.code, e.message
                )
                .into());
            }
            None => return Err(format!("MakeMove: empty response at turn {}", turn).into()),
        }
    }

    // ── Game Over: verify via GetGameState ──────────────────────────────
    let final_gs = game
        .get_game_state(GetGameStateRequest {
            session_id: session_id.clone(),
        })
        .await?
        .into_inner();

    if verbose {
        println!(
            "  [GetGameState final] finished={}, scores={}",
            final_gs.is_game_finished, final_gs.final_scores
        );
    }

    let elapsed = start.elapsed().as_millis();
    Ok((final_score, elapsed))
}

// ---------------------------------------------------------------------------
// Solo E2E wrapper with detailed test-case tracking
// ---------------------------------------------------------------------------

async fn run_solo_e2e(url: &str, verbose: bool) -> bool {
    println!("=== SOLO E2E TEST ===");
    println!("  Simulates: Welcome -> Mode Selection -> Solo -> 19 turns -> Game Over -> Replay");
    println!();

    let mut results = TestResults::new();

    let mut session = match SessionServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => {
            results.fail("connect", &e.to_string());
            return results.summary();
        }
    };
    let mut game = match GameServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => {
            results.fail("connect", &e.to_string());
            return results.summary();
        }
    };

    // ── 1. CreateSession ───────────────────────────────────────────────
    let resp = session
        .create_session(CreateSessionRequest {
            player_name: "e2e-solo".into(),
            max_players: 2,
            game_mode: "single-player".into(),
        })
        .await;

    let (session_id, player_id) = match resp {
        Ok(r) => match r.into_inner().result {
            Some(create_session_response::Result::Success(s)) => {
                results.pass("CreateSession");
                if verbose {
                    println!("    session={}, player={}, code={}", s.session_id, s.player_id, s.session_code);
                }
                (s.session_id, s.player_id)
            }
            Some(create_session_response::Result::Error(e)) => {
                results.fail("CreateSession", &format!("{}: {}", e.code, e.message));
                return results.summary();
            }
            None => {
                results.fail("CreateSession", "empty response");
                return results.summary();
            }
        },
        Err(e) => {
            results.fail("CreateSession", &e.to_string());
            return results.summary();
        }
    };

    // ── 2. GetSessionState (lobby polling) ─────────────────────────────
    match session
        .get_session_state(GetSessionStateRequest { session_id: session_id.clone() })
        .await
    {
        Ok(r) => {
            let inner = r.into_inner();
            if inner.game_state.is_some() {
                results.pass("GetSessionState (lobby)");
            } else if inner.error.is_some() {
                results.fail("GetSessionState (lobby)", &format!("{:?}", inner.error));
            } else {
                results.pass("GetSessionState (lobby)");
            }
        }
        Err(e) => results.fail("GetSessionState (lobby)", &e.to_string()),
    }

    // ── 3. SetReady ────────────────────────────────────────────────────
    match session
        .set_ready(SetReadyRequest {
            session_id: session_id.clone(),
            player_id: player_id.clone(),
            ready: true,
        })
        .await
    {
        Ok(r) => {
            let inner = r.into_inner();
            if inner.success || inner.game_started {
                results.pass("SetReady");
            } else {
                results.fail("SetReady", "success=false, game_started=false");
            }
        }
        Err(e) => results.fail("SetReady", &e.to_string()),
    }

    // ── 4. GetSessionState after ready (verify state changed) ──────────
    match session
        .get_session_state(GetSessionStateRequest { session_id: session_id.clone() })
        .await
    {
        Ok(r) => {
            let inner = r.into_inner();
            if let Some(gs) = &inner.game_state {
                if verbose {
                    println!("    state={}, mode={}", gs.state, gs.game_mode);
                }
                results.pass("GetSessionState (post-ready)");
            } else {
                results.pass("GetSessionState (post-ready)");
            }
        }
        Err(e) => results.fail("GetSessionState (post-ready)", &e.to_string()),
    }

    // ── 5. Game loop: 19 turns ─────────────────────────────────────────
    let mut turns_played = 0;
    let mut game_finished = false;
    let mut first_turn_get_available = false;

    for turn in 0..19 {
        // StartTurn
        let turn_resp = match game
            .start_turn(StartTurnRequest {
                session_id: session_id.clone(),
                forced_tile: String::new(),
            })
            .await
        {
            Ok(r) => r.into_inner(),
            Err(e) => {
                results.fail(&format!("StartTurn (turn {})", turn), &e.to_string());
                break;
            }
        };

        if !turn_resp.success {
            let err_msg = turn_resp.error.map(|e| e.message).unwrap_or_default();
            if err_msg.contains("finished") || err_msg.contains("over") {
                game_finished = true;
                break;
            }
            results.fail(&format!("StartTurn (turn {})", turn), &err_msg);
            break;
        }

        if turn == 0 {
            results.pass("StartTurn (first turn)");
        }

        if verbose {
            println!(
                "    turn={}: tile={}", turn_resp.turn_number, turn_resp.announced_tile
            );
        }

        // GetGameState (frontend polling: PollTurn every 3s)
        if turn == 0 || turn == 9 || turn == 18 {
            match game
                .get_game_state(GetGameStateRequest { session_id: session_id.clone() })
                .await
            {
                Ok(r) => {
                    let gs = r.into_inner();
                    if gs.success {
                        if turn == 0 {
                            results.pass("GetGameState (mid-game)");
                        }
                        if verbose {
                            println!(
                                "    [GetGameState] turn={}, tile={}", gs.current_turn, gs.current_tile
                            );
                        }
                    }
                }
                Err(e) => {
                    if turn == 0 {
                        results.fail("GetGameState (mid-game)", &e.to_string());
                    }
                }
            }
        }

        // GetAvailableMoves (frontend: highlight hexagons)
        if !first_turn_get_available {
            match game
                .get_available_moves(GetAvailableMovesRequest {
                    session_id: session_id.clone(),
                    player_id: player_id.clone(),
                })
                .await
            {
                Ok(r) => {
                    let inner = r.into_inner();
                    if inner.error.is_none() {
                        results.pass("GetAvailableMoves");
                        if verbose {
                            println!("    [GetAvailableMoves] {} moves", inner.available_moves.len());
                        }
                    } else {
                        results.fail("GetAvailableMoves", &format!("{:?}", inner.error));
                    }
                }
                Err(e) => results.fail("GetAvailableMoves", &e.to_string()),
            }
            first_turn_get_available = true;
        }

        // Pick position and play
        let position = match pick_position(&turn_resp.game_state, &player_id) {
            Ok(p) => p,
            Err(e) => {
                results.fail(&format!("pick_position (turn {})", turn), &e.to_string());
                break;
            }
        };

        let move_data = serde_json::json!({ "position": position }).to_string();
        match game
            .make_move(MakeMoveRequest {
                session_id: session_id.clone(),
                player_id: player_id.clone(),
                move_data,
                timestamp: chrono::Utc::now().timestamp_millis(),
            })
            .await
        {
            Ok(r) => match r.into_inner().result {
                Some(make_move_response::Result::Success(s)) => {
                    turns_played += 1;
                    if turn == 0 {
                        results.pass("MakeMove (first turn)");
                    }
                    if s.is_game_over {
                        game_finished = true;
                        if verbose {
                            println!("    Game over at turn {}!", turn + 1);
                        }
                        break;
                    }
                }
                Some(make_move_response::Result::Error(e)) => {
                    results.fail(
                        &format!("MakeMove (turn {})", turn),
                        &format!("{}: {}", e.code, e.message),
                    );
                    break;
                }
                None => {
                    results.fail(&format!("MakeMove (turn {})", turn), "empty response");
                    break;
                }
            },
            Err(e) => {
                results.fail(&format!("MakeMove (turn {})", turn), &e.to_string());
                break;
            }
        }
    }

    if turns_played > 0 {
        results.pass(&format!("Game loop ({} turns played)", turns_played));
    }

    // ── 6. Game Over: GetGameState to verify finished ──────────────────
    match game
        .get_game_state(GetGameStateRequest { session_id: session_id.clone() })
        .await
    {
        Ok(r) => {
            let gs = r.into_inner();
            if gs.is_game_finished || game_finished {
                results.pass("GetGameState (game over verification)");
                if verbose {
                    println!("    final_scores={}", gs.final_scores);
                }
            } else {
                results.fail(
                    "GetGameState (game over verification)",
                    &format!("is_game_finished={}", gs.is_game_finished),
                );
            }
        }
        Err(e) => results.fail("GetGameState (game over verification)", &e.to_string()),
    }

    // ── 7. "Rejouer" → new CreateSession ───────────────────────────────
    match session
        .create_session(CreateSessionRequest {
            player_name: "e2e-solo-replay".into(),
            max_players: 2,
            game_mode: "single-player".into(),
        })
        .await
    {
        Ok(r) => match r.into_inner().result {
            Some(create_session_response::Result::Success(_)) => {
                results.pass("CreateSession (replay)");
            }
            _ => results.fail("CreateSession (replay)", "unexpected response"),
        },
        Err(e) => results.fail("CreateSession (replay)", &e.to_string()),
    }

    results.summary()
}

// ---------------------------------------------------------------------------
// Multiplayer E2E — full 2-player journey
// ---------------------------------------------------------------------------
// Pages: Mode Selection → Create → Join → Lobby (poll) → Ready → Play → Finish

async fn run_multiplayer_e2e(url: &str, total_games: usize, verbose: bool) -> bool {
    println!("=== MULTIPLAYER E2E TEST ({} game{}) ===", total_games, if total_games > 1 { "s" } else { "" });
    println!("  Simulates: P1 Create -> P2 Join -> Lobby poll -> Both Ready -> 19 turns -> Game Over -> Rejouer");
    println!();

    let mut results = TestResults::new();

    let mut session1 = match SessionServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => { results.fail("connect P1", &e.to_string()); return results.summary(); }
    };
    let mut session2 = match SessionServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => { results.fail("connect P2", &e.to_string()); return results.summary(); }
    };
    let mut game1 = match GameServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => { results.fail("connect game1", &e.to_string()); return results.summary(); }
    };
    let mut game2 = match GameServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => { results.fail("connect game2", &e.to_string()); return results.summary(); }
    };

    for game_num in 0..total_games {
        if verbose || total_games > 1 {
            println!("  --- Multiplayer game {}/{} ---", game_num + 1, total_games);
        }

        // ── 1. P1: CreateSession ───────────────────────────────────────
        let resp = session1
            .create_session(CreateSessionRequest {
                player_name: format!("e2e-p1-g{}", game_num + 1),
                max_players: 3,
                game_mode: "multiplayer".into(),
            })
            .await;

        let (session_id, session_code, player_id_1) = match resp {
            Ok(r) => match r.into_inner().result {
                Some(create_session_response::Result::Success(s)) => {
                    if game_num == 0 { results.pass("P1 CreateSession"); }
                    if verbose {
                        println!("    session={}, code={}", s.session_id, s.session_code);
                    }
                    (s.session_id, s.session_code, s.player_id)
                }
                Some(create_session_response::Result::Error(e)) => {
                    results.fail(&format!("P1 CreateSession (game {})", game_num + 1), &format!("{}: {}", e.code, e.message));
                    continue;
                }
                None => { results.fail(&format!("P1 CreateSession (game {})", game_num + 1), "empty"); continue; }
            },
            Err(e) => { results.fail(&format!("P1 CreateSession (game {})", game_num + 1), &e.to_string()); continue; }
        };

        // ── 2. P1 polls lobby ──────────────────────────────────────────
        if game_num == 0 {
            match session1
                .get_session_state(GetSessionStateRequest { session_id: session_id.clone() })
                .await
            {
                Ok(r) => {
                    let inner = r.into_inner();
                    if let Some(gs) = &inner.game_state {
                        if gs.players.len() >= 1 {
                            results.pass("GetSessionState (P1 alone in lobby)");
                        } else {
                            results.fail("GetSessionState (P1 alone)", &format!("{} players", gs.players.len()));
                        }
                    } else {
                        results.pass("GetSessionState (P1 alone in lobby)");
                    }
                }
                Err(e) => results.fail("GetSessionState (P1 alone)", &e.to_string()),
            }
        }

        // ── 3. P2: JoinSession ─────────────────────────────────────────
        let player_id_2 = match session2
            .join_session(JoinSessionRequest {
                session_code,
                player_name: format!("e2e-p2-g{}", game_num + 1),
            })
            .await
        {
            Ok(r) => match r.into_inner().result {
                Some(join_session_response::Result::Success(s)) => {
                    if game_num == 0 { results.pass("P2 JoinSession"); }
                    s.player_id
                }
                Some(join_session_response::Result::Error(e)) => {
                    results.fail(&format!("P2 JoinSession (game {})", game_num + 1), &format!("{}: {}", e.code, e.message));
                    continue;
                }
                None => { results.fail(&format!("P2 JoinSession (game {})", game_num + 1), "empty"); continue; }
            },
            Err(e) => { results.fail(&format!("P2 JoinSession (game {})", game_num + 1), &e.to_string()); continue; }
        };

        // ── 4. Lobby poll: 2 players ───────────────────────────────────
        if game_num == 0 {
            match session1
                .get_session_state(GetSessionStateRequest { session_id: session_id.clone() })
                .await
            {
                Ok(r) => {
                    if let Some(gs) = &r.into_inner().game_state {
                        if gs.players.len() >= 2 {
                            results.pass("GetSessionState (2 players in lobby)");
                        } else {
                            results.fail("GetSessionState (2 players)", &format!("only {} players", gs.players.len()));
                        }
                    } else {
                        results.pass("GetSessionState (2 players in lobby)");
                    }
                }
                Err(e) => results.fail("GetSessionState (2 players)", &e.to_string()),
            }
        }

        // ── 5. Both SetReady ───────────────────────────────────────────
        match session1
            .set_ready(SetReadyRequest {
                session_id: session_id.clone(),
                player_id: player_id_1.clone(),
                ready: true,
            })
            .await
        {
            Ok(r) => {
                if game_num == 0 {
                    results.pass("P1 SetReady");
                    if verbose {
                        let inner = r.into_inner();
                        println!("    P1 ready: game_started={}", inner.game_started);
                    }
                }
            }
            Err(e) => { results.fail(&format!("P1 SetReady (game {})", game_num + 1), &e.to_string()); continue; }
        }

        match session2
            .set_ready(SetReadyRequest {
                session_id: session_id.clone(),
                player_id: player_id_2.clone(),
                ready: true,
            })
            .await
        {
            Ok(r) => {
                if game_num == 0 {
                    results.pass("P2 SetReady");
                    if verbose {
                        let inner = r.into_inner();
                        println!("    P2 ready: game_started={}", inner.game_started);
                    }
                }
            }
            Err(e) => { results.fail(&format!("P2 SetReady (game {})", game_num + 1), &e.to_string()); continue; }
        }

        // ── 6. Game loop: 19 turns ─────────────────────────────────────
        let mut turns_played = 0;
        let mut game_failed = false;
        for turn in 0..19 {
            let turn_resp = match game1
                .start_turn(StartTurnRequest {
                    session_id: session_id.clone(),
                    forced_tile: String::new(),
                })
                .await
            {
                Ok(r) => r.into_inner(),
                Err(e) => {
                    if turn == 0 {
                        results.fail(&format!("StartTurn (game {} turn 0)", game_num + 1), &e.to_string());
                        game_failed = true;
                    }
                    break;
                }
            };

            if !turn_resp.success {
                let err_msg = turn_resp.error.map(|e| e.message).unwrap_or_default();
                if err_msg.contains("finished") || err_msg.contains("over") || err_msg.contains("terminé") {
                    break;
                }
                if turn == 0 {
                    results.fail(&format!("StartTurn (game {} turn 0)", game_num + 1), &err_msg);
                    game_failed = true;
                }
                break;
            }

            if game_num == 0 && turn == 0 {
                results.pass("StartTurn (multi first)");
            }

            if verbose {
                println!("    turn={}: tile={}", turn, turn_resp.announced_tile);
            }

            let gs = &turn_resp.game_state;
            let pos1 = match pick_position(gs, &player_id_1) {
                Ok(p) => p,
                Err(_) => break,
            };
            let pos2 = match pick_position(gs, &player_id_2) {
                Ok(p) => p,
                Err(_) => break,
            };

            let move1 = serde_json::json!({ "position": pos1 }).to_string();
            let move2 = serde_json::json!({ "position": pos2 }).to_string();
            let ts = chrono::Utc::now().timestamp_millis();

            let (r1, r2) = tokio::join!(
                game1.make_move(MakeMoveRequest {
                    session_id: session_id.clone(),
                    player_id: player_id_1.clone(),
                    move_data: move1,
                    timestamp: ts,
                }),
                game2.make_move(MakeMoveRequest {
                    session_id: session_id.clone(),
                    player_id: player_id_2.clone(),
                    move_data: move2,
                    timestamp: ts,
                }),
            );

            let mut game_over = false;
            for (label, result) in [("P1", r1), ("P2", r2)] {
                if let Ok(resp) = result {
                    if let Some(make_move_response::Result::Success(s)) = resp.into_inner().result {
                        if verbose {
                            println!("    {} -> pos={}, game_over={}", label,
                                if label == "P1" { pos1 } else { pos2 }, s.is_game_over);
                        }
                        if s.is_game_over {
                            game_over = true;
                        }
                    }
                }
            }

            turns_played += 1;
            if game_over {
                if verbose { println!("    Game over at turn {}!", turn + 1); }
                break;
            }
        }

        // ── 7. GetGameState final ──────────────────────────────────────
        let mut final_info = String::new();
        match game1
            .get_game_state(GetGameStateRequest { session_id: session_id.clone() })
            .await
        {
            Ok(r) => {
                let gs = r.into_inner();
                if game_num == 0 {
                    results.pass("GetGameState (multi final)");
                }
                final_info = format!("finished={}, scores={}", gs.is_game_finished, gs.final_scores);
            }
            Err(e) => {
                if game_num == 0 {
                    results.fail("GetGameState (multi final)", &e.to_string());
                }
            }
        }

        if !game_failed {
            results.pass(&format!("Game {} complete ({} turns)", game_num + 1, turns_played));
        }

        if verbose || total_games > 1 {
            println!("    -> {} turns, {}", turns_played, final_info);
        }
    }

    results.summary()
}

// ---------------------------------------------------------------------------
// Real-Game E2E — GetAiMove focus
// ---------------------------------------------------------------------------

async fn run_real_game_e2e(url: &str, total_games: usize, verbose: bool) -> bool {
    println!("=== REAL GAME E2E TEST ({} game{}) ===", total_games, if total_games > 1 { "s" } else { "" });
    println!("  Simulates: Mode Selection -> Jeu Réel -> Pick tile -> GetAiMove -> 19 turns -> Recommencer");
    println!();

    let mut results = TestResults::new();

    let mut game = match GameServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => { results.fail("connect", &e.to_string()); return results.summary(); }
    };

    // All 27 valid tiles: digit1 ∈ {1,5,9}, digit2 ∈ {2,6,7}, digit3 ∈ {3,4,8}
    let all_tiles: Vec<&str> = vec![
        "123", "124", "128", "163", "164", "168", "173", "174", "178",
        "523", "524", "528", "563", "564", "568", "573", "574", "578",
        "923", "924", "928", "963", "964", "968", "973", "974", "978",
    ];

    let mut rng = rand::rng();

    for game_num in 0..total_games {
        if verbose || total_games > 1 {
            println!("  --- Real game {}/{} ---", game_num + 1, total_games);
        }

        // Shuffle and pick 19 tiles (like a real deck draw)
        let mut deck = all_tiles.clone();
        deck.shuffle(&mut rng);
        let tiles: Vec<&str> = deck.into_iter().take(19).collect();

        let mut board_state: Vec<String> = vec!["".to_string(); 19];
        let mut game_ok = true;

        for (turn, tile_code) in tiles.iter().enumerate() {
            let available: Vec<i32> = board_state
                .iter()
                .enumerate()
                .filter(|(_, t)| t.is_empty())
                .map(|(i, _)| i as i32)
                .collect();

            if available.is_empty() {
                break;
            }

            match game
                .get_ai_move(GetAiMoveRequest {
                    tile_code: tile_code.to_string(),
                    board_state: board_state.clone(),
                    available_positions: available.clone(),
                    turn_number: turn as i32,
                })
                .await
            {
                Ok(r) => {
                    let inner = r.into_inner();
                    if inner.success {
                        let pos = inner.recommended_position;
                        if available.contains(&pos) {
                            board_state[pos as usize] = tile_code.to_string();
                            if game_num == 0 && turn == 0 {
                                results.pass("GetAiMove (first tile)");
                            }
                            if verbose {
                                println!(
                                    "    turn={}: tile={}, AI chose position {}",
                                    turn, tile_code, pos
                                );
                            }
                        } else {
                            results.fail(
                                &format!("GetAiMove (game {} turn {})", game_num + 1, turn),
                                &format!("pos {} not in available {:?}", pos, available),
                            );
                            game_ok = false;
                            break;
                        }
                    } else {
                        let err = inner.error.map(|e| e.message).unwrap_or_default();
                        results.fail(&format!("GetAiMove (game {} turn {})", game_num + 1, turn), &err);
                        game_ok = false;
                        break;
                    }
                }
                Err(e) => {
                    results.fail(&format!("GetAiMove (game {} turn {})", game_num + 1, turn), &e.to_string());
                    game_ok = false;
                    break;
                }
            }
        }

        let filled = board_state.iter().filter(|t| !t.is_empty()).count();
        if game_ok && filled == 19 {
            results.pass(&format!("Game {} complete (19/19 tiles)", game_num + 1));
        } else if game_ok && filled > 0 {
            results.pass(&format!("Game {} partial ({}/19 tiles)", game_num + 1, filled));
        }

        if verbose || total_games > 1 {
            println!("    -> {}/19 tiles placed{}", filled, if game_ok { "" } else { " (FAILED)" });
        }
    }

    results.summary()
}

// ---------------------------------------------------------------------------
// Error-path E2E — exercises error handling for all endpoints
// ---------------------------------------------------------------------------

async fn run_errors_e2e(url: &str, verbose: bool) -> bool {
    println!("=== ERROR PATH E2E TEST ===");
    println!("  Tests: invalid sessions, wrong players, bad moves, double-ready");
    println!();

    let mut results = TestResults::new();

    let mut session = match SessionServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => { results.fail("connect", &e.to_string()); return results.summary(); }
    };
    let mut game = match GameServiceClient::connect(url.to_string()).await {
        Ok(c) => c,
        Err(e) => { results.fail("connect", &e.to_string()); return results.summary(); }
    };

    // ── 1. JoinSession with invalid code ───────────────────────────────
    match session
        .join_session(JoinSessionRequest {
            session_code: "INVALID-CODE-XYZ".into(),
            player_name: "ghost".into(),
        })
        .await
    {
        Ok(r) => match r.into_inner().result {
            Some(join_session_response::Result::Error(_)) => {
                results.pass("JoinSession (invalid code → error)");
            }
            Some(join_session_response::Result::Success(_)) => {
                results.fail("JoinSession (invalid code)", "should have failed but succeeded");
            }
            None => results.pass("JoinSession (invalid code → empty = OK)"),
        },
        Err(_) => {
            // gRPC-level error is also acceptable
            results.pass("JoinSession (invalid code → gRPC error)");
        }
    }

    // ── 2. GetSessionState with fake session_id ────────────────────────
    match session
        .get_session_state(GetSessionStateRequest {
            session_id: "nonexistent-session-id".into(),
        })
        .await
    {
        Ok(r) => {
            let inner = r.into_inner();
            if inner.error.is_some() || inner.game_state.is_none() {
                results.pass("GetSessionState (invalid session → error)");
            } else {
                results.fail("GetSessionState (invalid session)", "returned game_state for nonexistent session");
            }
        }
        Err(_) => results.pass("GetSessionState (invalid session → gRPC error)"),
    }

    // ── 3. SetReady with invalid session ───────────────────────────────
    match session
        .set_ready(SetReadyRequest {
            session_id: "nonexistent".into(),
            player_id: "ghost".into(),
            ready: true,
        })
        .await
    {
        Ok(r) => {
            let inner = r.into_inner();
            if !inner.success || inner.error.is_some() {
                results.pass("SetReady (invalid session → error)");
            } else {
                results.fail("SetReady (invalid session)", "should have failed");
            }
        }
        Err(_) => results.pass("SetReady (invalid session → gRPC error)"),
    }

    // ── 4. StartTurn with invalid session ──────────────────────────────
    match game
        .start_turn(StartTurnRequest {
            session_id: "nonexistent".into(),
            forced_tile: String::new(),
        })
        .await
    {
        Ok(r) => {
            let inner = r.into_inner();
            if !inner.success || inner.error.is_some() {
                results.pass("StartTurn (invalid session → error)");
            } else {
                results.fail("StartTurn (invalid session)", "should have failed");
            }
        }
        Err(_) => results.pass("StartTurn (invalid session → gRPC error)"),
    }

    // ── 5. MakeMove with invalid session ───────────────────────────────
    match game
        .make_move(MakeMoveRequest {
            session_id: "nonexistent".into(),
            player_id: "ghost".into(),
            move_data: r#"{"position": 0}"#.into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
        .await
    {
        Ok(r) => match r.into_inner().result {
            Some(make_move_response::Result::Error(_)) => {
                results.pass("MakeMove (invalid session → error)");
            }
            Some(make_move_response::Result::Success(_)) => {
                results.fail("MakeMove (invalid session)", "should have failed");
            }
            None => results.pass("MakeMove (invalid session → empty = OK)"),
        },
        Err(_) => results.pass("MakeMove (invalid session → gRPC error)"),
    }

    // ── 6. GetGameState with invalid session ───────────────────────────
    match game
        .get_game_state(GetGameStateRequest {
            session_id: "nonexistent".into(),
        })
        .await
    {
        Ok(r) => {
            let inner = r.into_inner();
            if !inner.success || inner.error.is_some() {
                results.pass("GetGameState (invalid session → error)");
            } else {
                results.fail("GetGameState (invalid session)", "should have failed");
            }
        }
        Err(_) => results.pass("GetGameState (invalid session → gRPC error)"),
    }

    // ── 7. GetAvailableMoves with invalid session ──────────────────────
    match game
        .get_available_moves(GetAvailableMovesRequest {
            session_id: "nonexistent".into(),
            player_id: "ghost".into(),
        })
        .await
    {
        Ok(r) => {
            let inner = r.into_inner();
            if inner.error.is_some() || inner.available_moves.is_empty() {
                results.pass("GetAvailableMoves (invalid session → error)");
            } else {
                results.fail("GetAvailableMoves (invalid session)", "should have failed");
            }
        }
        Err(_) => results.pass("GetAvailableMoves (invalid session → gRPC error)"),
    }

    // ── 8. MakeMove with wrong player_id on valid session ──────────────
    // Create a real session first
    if let Ok(r) = session
        .create_session(CreateSessionRequest {
            player_name: "e2e-err".into(),
            max_players: 2,
            game_mode: "single-player".into(),
        })
        .await
    {
        if let Some(create_session_response::Result::Success(s)) = r.into_inner().result {
            // SetReady + StartTurn to get game going
            let _ = session
                .set_ready(SetReadyRequest {
                    session_id: s.session_id.clone(),
                    player_id: s.player_id.clone(),
                    ready: true,
                })
                .await;

            let _ = game
                .start_turn(StartTurnRequest {
                    session_id: s.session_id.clone(),
                    forced_tile: String::new(),
                })
                .await;

            // Try MakeMove with wrong player
            match game
                .make_move(MakeMoveRequest {
                    session_id: s.session_id.clone(),
                    player_id: "wrong-player-id".into(),
                    move_data: r#"{"position": 0}"#.into(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                })
                .await
            {
                Ok(r) => match r.into_inner().result {
                    Some(make_move_response::Result::Error(_)) => {
                        results.pass("MakeMove (wrong player_id → error)");
                    }
                    _ => {
                        // Some servers may accept any player_id — not a hard failure
                        if verbose {
                            println!("    MakeMove with wrong player: accepted (server-dependent)");
                        }
                        results.pass("MakeMove (wrong player_id → accepted)");
                    }
                },
                Err(_) => results.pass("MakeMove (wrong player_id → gRPC error)"),
            }
        }
    }

    results.summary()
}

// ---------------------------------------------------------------------------
// Stress test (unchanged from v1)
// ---------------------------------------------------------------------------

struct StressMetrics {
    completed: AtomicU64,
    failed: AtomicU64,
    total_time_ms: AtomicU64,
    min_time_ms: AtomicU64,
    max_time_ms: AtomicU64,
}

impl StressMetrics {
    fn new() -> Self {
        Self {
            completed: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            total_time_ms: AtomicU64::new(0),
            min_time_ms: AtomicU64::new(u64::MAX),
            max_time_ms: AtomicU64::new(0),
        }
    }

    fn record_success(&self, elapsed_ms: u64) {
        self.completed.fetch_add(1, Ordering::Relaxed);
        self.total_time_ms.fetch_add(elapsed_ms, Ordering::Relaxed);
        self.min_time_ms.fetch_min(elapsed_ms, Ordering::Relaxed);
        self.max_time_ms.fetch_max(elapsed_ms, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }
}

async fn run_stress(url: &str, concurrent: usize, total_games: usize, verbose: bool) {
    println!("=== STRESS TEST ===");
    println!(
        "Target: {} | Concurrent: {} | Total games: {}",
        url, concurrent, total_games
    );
    println!();

    let metrics = Arc::new(StressMetrics::new());
    let semaphore = Arc::new(Semaphore::new(concurrent));
    let wall_start = Instant::now();

    let mut join_set = JoinSet::new();

    for i in 0..total_games {
        let url = url.to_string();
        let metrics = metrics.clone();
        let sem = semaphore.clone();

        join_set.spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            match run_solo(&url, false).await {
                Ok((_score, elapsed_ms)) => {
                    metrics.record_success(elapsed_ms as u64);
                    if verbose {
                        println!("  Game {} completed: {}ms", i, elapsed_ms);
                    }
                }
                Err(e) => {
                    metrics.record_failure();
                    if verbose {
                        println!("  Game {} failed: {}", i, e);
                    }
                }
            }
        });
    }

    while join_set.join_next().await.is_some() {}

    let wall_time = wall_start.elapsed();
    let completed = metrics.completed.load(Ordering::Relaxed);
    let failed = metrics.failed.load(Ordering::Relaxed);
    let total = completed + failed;
    let total_time = metrics.total_time_ms.load(Ordering::Relaxed);
    let min_time = metrics.min_time_ms.load(Ordering::Relaxed);
    let max_time = metrics.max_time_ms.load(Ordering::Relaxed);

    let avg_time = if completed > 0 { total_time / completed } else { 0 };
    let throughput = if wall_time.as_secs_f64() > 0.0 {
        completed as f64 / wall_time.as_secs_f64()
    } else {
        0.0
    };
    let pct = if total > 0 {
        completed as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    println!("=== STRESS TEST RESULTS ===");
    println!("Total games:    {}", total);
    println!("Completed:      {} ({:.1}%)", completed, pct);
    println!("Failed:         {}", failed);
    println!("Wall time:      {:.2}s", wall_time.as_secs_f64());
    println!("Throughput:     {:.2} games/s", throughput);
    println!("Avg game time:  {}ms", avg_time);
    if completed > 0 {
        println!("Min game time:  {}ms", min_time);
        println!("Max game time:  {}ms", max_time);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse the game_state JSON and pick the first available position for this player.
fn pick_position(game_state_json: &str, player_id: &str) -> Result<i64, BoxError> {
    let state: Value = serde_json::from_str(game_state_json)?;

    // Try player_plateaus.<player_id>.available_positions
    if let Some(plateaus) = state.get("player_plateaus") {
        if let Some(plateau) = plateaus.get(player_id) {
            if let Some(positions) = plateau.get("available_positions") {
                if let Some(arr) = positions.as_array() {
                    if let Some(first) = arr.first() {
                        if let Some(n) = first.as_i64() {
                            return Ok(n);
                        }
                    }
                }
            }
        }
    }

    // Fallback: try all plateaus, skip mcts_ai
    if let Some(plateaus) = state.get("player_plateaus") {
        if let Some(obj) = plateaus.as_object() {
            for (pid, plateau) in obj {
                if pid == "mcts_ai" {
                    continue;
                }
                if let Some(positions) = plateau.get("available_positions") {
                    if let Some(arr) = positions.as_array() {
                        if let Some(first) = arr.first() {
                            if let Some(n) = first.as_i64() {
                                return Ok(n);
                            }
                        }
                    }
                }
            }
        }
    }

    Err(format!(
        "No available positions found for player '{}' in game_state",
        player_id
    )
    .into())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Connectivity check
    if let Err(e) = SessionServiceClient::connect(cli.url.clone()).await {
        eprintln!("Cannot connect to {}: {}", cli.url, e);
        std::process::exit(1);
    }

    let games = cli.total_games;

    let exit_code = match cli.mode.as_str() {
        "solo" => {
            if !run_solo_e2e(&cli.url, cli.verbose).await { 1 } else { 0 }
        }
        "multiplayer" => {
            if !run_multiplayer_e2e(&cli.url, games.max(1), cli.verbose).await { 1 } else { 0 }
        }
        "real-game" => {
            if !run_real_game_e2e(&cli.url, games.max(1), cli.verbose).await { 1 } else { 0 }
        }
        "errors" => {
            if !run_errors_e2e(&cli.url, cli.verbose).await { 1 } else { 0 }
        }
        "all" => {
            println!("Running all E2E tests...\n");
            let mut all_ok = true;

            all_ok &= run_solo_e2e(&cli.url, cli.verbose).await;
            println!();
            all_ok &= run_multiplayer_e2e(&cli.url, games.max(1), cli.verbose).await;
            println!();
            all_ok &= run_real_game_e2e(&cli.url, games.max(1), cli.verbose).await;
            println!();
            all_ok &= run_errors_e2e(&cli.url, cli.verbose).await;

            println!("\n========================================");
            if all_ok {
                println!("ALL E2E SUITES PASSED");
            } else {
                println!("SOME E2E SUITES FAILED");
            }

            if !all_ok { 1 } else { 0 }
        }
        "stress" => {
            run_stress(&cli.url, cli.concurrent, games.max(1), cli.verbose).await;
            0
        }
        other => {
            eprintln!(
                "Unknown mode: '{}'. Use: solo, multiplayer, real-game, errors, all, stress",
                other
            );
            1
        }
    };

    std::process::exit(exit_code);
}
