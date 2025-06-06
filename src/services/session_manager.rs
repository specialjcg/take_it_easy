// src/services/session_manager.rs - 100% fonctionnel - TOUTES les fonctions extraites

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::generated::takeiteasygame::v1::*;

// ============================================================================
// TYPES DE DONNÉES IMMUTABLES PURS
// ============================================================================

#[derive(Debug, Clone)]
pub struct GameSession {
    pub id: String,
    pub code: String,
    pub players: HashMap<String, Player>,
    pub current_player_id: Option<String>,
    pub state: i32,
    pub max_players: i32,
    pub game_mode: String,
    pub created_at: std::time::Instant,
    pub board_state: String,
    pub turn_number: i32,
}

#[derive(Debug, Clone)]
pub enum SessionAction {
    CreateSession { session: GameSession },
    UpdateSession { session: GameSession },
    RemoveSession { session_id: String },
}

#[derive(Debug, Clone)]
pub struct SessionStoreState {
    pub sessions: HashMap<String, GameSession>,
    pub sessions_by_code: HashMap<String, String>,
}

impl SessionStoreState {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            sessions_by_code: HashMap::new(),
        }
    }
}

// SessionManager - Structure de données pure (pas de logique)
#[derive(Clone)]
pub struct SessionManager {
    store: Arc<RwLock<SessionStoreState>>,
}

// ============================================================================
// FONCTIONS DE CONSTRUCTION (FACTORY FUNCTIONS)
// ============================================================================

pub fn new_session_manager() -> SessionManager {
    SessionManager {
        store: Arc::new(RwLock::new(SessionStoreState::new())),
    }
}

pub fn get_store_from_manager(manager: &SessionManager) -> &Arc<RwLock<SessionStoreState>> {
    &manager.store
}

// ============================================================================
// FONCTIONS PURES - MANIPULATION D'ÉTAT
// ============================================================================

pub fn apply_session_action(state: SessionStoreState, action: SessionAction) -> SessionStoreState {
    match action {
        SessionAction::CreateSession { session } => {
            let mut new_state = state;
            new_state.sessions.insert(session.id.clone(), session.clone());
            new_state.sessions_by_code.insert(session.code.clone(), session.id.clone());
            new_state
        },
        SessionAction::UpdateSession { session } => {
            let mut new_state = state;
            new_state.sessions.insert(session.id.clone(), session.clone());
            new_state.sessions_by_code.insert(session.code.clone(), session.id.clone());
            new_state
        },
        SessionAction::RemoveSession { session_id } => {
            let mut new_state = state;
            if let Some(session) = new_state.sessions.remove(&session_id) {
                new_state.sessions_by_code.remove(&session.code);
            }
            new_state
        }
    }
}

// ============================================================================
// FONCTIONS PURES - REQUÊTES
// ============================================================================

pub fn find_session_by_code<'a>(state: &'a SessionStoreState, code: &str) -> Option<&'a GameSession> {
    state.sessions_by_code.get(code)
        .and_then(|session_id| state.sessions.get(session_id))
}

pub fn find_session_by_id<'a>(state: &'a SessionStoreState, session_id: &str) -> Option<&'a GameSession> {
    state.sessions.get(session_id)
}

pub fn count_sessions(state: &SessionStoreState) -> usize {
    state.sessions.len()
}

pub fn get_all_session_codes(state: &SessionStoreState) -> Vec<String> {
    state.sessions_by_code.keys().cloned().collect()
}

// ============================================================================
// FONCTIONS PURES - CRÉATION D'OBJETS
// ============================================================================

pub fn create_game_session(max_players: i32, game_mode: String) -> GameSession {
    GameSession {
        id: Uuid::new_v4().to_string(),
        code: generate_session_code(),
        players: HashMap::new(),
        current_player_id: None,
        state: 0, // WAITING
        max_players,
        game_mode,
        created_at: std::time::Instant::now(),
        board_state: "{}".to_string(),
        turn_number: 0,
    }
}

pub fn create_session_action(max_players: i32, game_mode: String) -> (SessionAction, String) {
    let session = create_game_session(max_players, game_mode);
    let session_code = session.code.clone();
    (SessionAction::CreateSession { session }, session_code)
}

// ============================================================================
// FONCTIONS PURES - TRANSFORMATIONS DE SESSIONS
// ============================================================================

// src/services/session_manager.rs
// src/services/session_manager.rs
pub fn add_player_to_session(
    session: GameSession,
    player_name: String
) -> Result<(GameSession, String), String> {
    if session.players.len() >= session.max_players as usize {
        return Err("SESSION_FULL".to_string());
    }

    if session.state != 0 {
        return Err("GAME_IN_PROGRESS".to_string());
    }

    let player_id = Uuid::new_v4().to_string();

    // ✅ Le créateur (premier joueur) est automatiquement prêt
    let is_first_player = session.players.is_empty();

    let player = Player {
        id: player_id.clone(),
        name: player_name,
        score: 0,
        is_ready: is_first_player, // ← Automatiquement prêt si c'est le créateur
        is_connected: true,
        joined_at: chrono::Utc::now().timestamp(),
    };

    let mut new_session = session;
    new_session.players.insert(player_id.clone(), player);

    Ok((new_session, player_id))
}

pub fn set_player_ready_in_session(
    session: GameSession,
    player_id: &str,
    ready: bool
) -> Result<(GameSession, bool), String> {
    let mut new_session = session;

    match new_session.players.get_mut(player_id) {
        Some(player) => {
            player.is_ready = ready;

            let game_started = if all_players_ready(&new_session) && new_session.players.len() >= 2 {
                new_session = start_game(new_session);
                true
            } else {
                false
            };

            Ok((new_session, game_started))
        },
        None => Err("PLAYER_NOT_FOUND".to_string())
    }
}

pub fn remove_player_from_session(
    session: GameSession,
    player_id: &str
) -> (GameSession, bool) {
    let mut new_session = session;
    let was_removed = new_session.players.remove(player_id).is_some();

    // If it was the current player's turn, advance to next player
    if new_session.current_player_id.as_ref() == Some(&player_id.to_string()) {
        new_session = advance_turn(new_session);
    }

    (new_session, was_removed)
}

pub fn advance_turn(session: GameSession) -> GameSession {
    let mut new_session = session;

    if let Some(current_id) = &new_session.current_player_id {
        let player_ids: Vec<String> = new_session.players.keys().cloned().collect();

        if let Some(current_index) = player_ids.iter().position(|id| id == current_id) {
            let next_index = (current_index + 1) % player_ids.len();
            new_session.current_player_id = Some(player_ids[next_index].clone());
            new_session.turn_number += 1;
        }
    }

    new_session
}

// ============================================================================
// FONCTIONS PURES - UTILITAIRES
// ============================================================================

fn all_players_ready(session: &GameSession) -> bool {
    !session.players.is_empty() &&
        session.players.values().all(|p| p.is_ready && p.is_connected)
}

fn start_game(session: GameSession) -> GameSession {
    let mut new_session = session;
    new_session.state = 1; // IN_PROGRESS

    // Set first player
    if let Some(first_player_id) = new_session.players.keys().next() {
        new_session.current_player_id = Some(first_player_id.clone());
    }

    new_session.turn_number = 1;
    new_session.board_state = r#"{"tiles": [], "available_positions": []}"#.to_string();

    new_session
}

pub fn session_to_game_state(session: &GameSession) -> GameState {
    GameState {
        session_id: session.id.clone(),
        players: session.players.values().cloned().collect(),
        current_player_id: session.current_player_id.clone().unwrap_or_default(),
        state: session.state,
        board_state: session.board_state.clone(),
        turn_number: session.turn_number,
    }
}

fn generate_session_code() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();

    (0..6)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect()
}

// ============================================================================
// FONCTIONS DE STORE - OPÉRATIONS ASYNCHRONES COMPOSABLES
// ============================================================================

pub async fn create_session_in_store<F, T>(
    store: &Arc<RwLock<SessionStoreState>>,
    max_players: i32,
    game_mode: String,
    continuation: F
) -> Result<T, String>
where
    F: FnOnce(String) -> Result<T, String>,
{
    let (action, session_code) = create_session_action(max_players, game_mode);

    {
        let mut state = store.write().await;
        *state = apply_session_action(state.clone(), action);
    }

    continuation(session_code)
}

pub async fn get_session_by_code_from_store(
    store: &Arc<RwLock<SessionStoreState>>,
    code: &str
) -> Option<GameSession> {
    let state = store.read().await;
    find_session_by_code(&*state, code).cloned()
}

pub async fn get_session_by_id_from_store(
    store: &Arc<RwLock<SessionStoreState>>,
    session_id: &str
) -> Option<GameSession> {
    let state = store.read().await;
    find_session_by_id(&*state, session_id).cloned()
}

pub async fn update_session_in_store(
    store: &Arc<RwLock<SessionStoreState>>,
    session: GameSession
) -> Result<(), String> {
    let action = SessionAction::UpdateSession { session };

    {
        let mut state = store.write().await;
        *state = apply_session_action(state.clone(), action);
    }

    Ok(())
}

pub async fn transform_session_in_store<F, T>(
    store: &Arc<RwLock<SessionStoreState>>,
    session_id: &str,
    transformation: F
) -> Result<Option<T>, String>
where
    F: FnOnce(GameSession) -> Result<(GameSession, T), String>,
{
    let current_session = get_session_by_id_from_store(store, session_id).await;

    match current_session {
        Some(session) => {
            match transformation(session) {
                Ok((updated_session, result)) => {
                    update_session_in_store(store, updated_session).await?;
                    Ok(Some(result))
                },
                Err(e) => Err(e)
            }
        },
        None => Ok(None)
    }
}

pub async fn query_store<F, T>(
    store: &Arc<RwLock<SessionStoreState>>,
    query: F
) -> T
where
    F: FnOnce(&SessionStoreState) -> T,
{
    let state = store.read().await;
    query(&*state)
}

// ============================================================================
// FONCTIONS DE NIVEAU SUPÉRIEUR - COMPOSITION AVEC SESSIONMANAGER
// ============================================================================

pub async fn create_session_with_manager(
    manager: &SessionManager,
    max_players: i32,
    game_mode: String
) -> String {
    create_session_in_store(get_store_from_manager(manager), max_players, game_mode, |code| Ok(code)).await
        .unwrap_or_else(|_| "ERROR".to_string())
}

pub async fn get_session_by_code_with_manager(
    manager: &SessionManager,
    code: &str
) -> Option<GameSession> {
    get_session_by_code_from_store(get_store_from_manager(manager), code).await
}

pub async fn get_session_by_id_with_manager(
    manager: &SessionManager,
    session_id: &str
) -> Option<GameSession> {
    get_session_by_id_from_store(get_store_from_manager(manager), session_id).await
}

pub async fn update_session_with_manager(
    manager: &SessionManager,
    session: GameSession
) -> Result<(), String> {
    update_session_in_store(get_store_from_manager(manager), session).await
}

pub async fn create_session_functional_with_manager(
    manager: &SessionManager,
    max_players: i32,
    game_mode: String
) -> Result<String, String> {
    create_session_in_store(get_store_from_manager(manager), max_players, game_mode, |code| Ok(code)).await
}

pub async fn transform_session_with_manager<F>(
    manager: &SessionManager,
    session_id: &str,
    f: F
) -> Result<Option<GameSession>, String>
where
    F: FnOnce(GameSession) -> Result<GameSession, String>,
{
    transform_session_in_store(get_store_from_manager(manager), session_id, |session| {
        f(session).map(|updated_session| (updated_session.clone(), updated_session))
    }).await
}

pub async fn with_session_state_from_manager<F, T>(
    manager: &SessionManager,
    f: F
) -> T
where
    F: FnOnce(&SessionStoreState) -> T,
{
    query_store(get_store_from_manager(manager), f).await
}

// ============================================================================
// IMPLÉMENTATION VIDE - SESSIONMANAGER DEVIENT JUSTE UNE STRUCTURE
// ============================================================================

impl SessionManager {
    // Seule fonction dans l'impl - construction
    pub fn new() -> Self {
        new_session_manager()
    }

    // Toutes les autres fonctions sont maintenant externes !
    // Utilisez les fonctions *_with_manager() à la place
}

