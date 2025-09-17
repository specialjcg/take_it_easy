// src/services/game_service/session_utils.rs - Utilitaires de gestion des sessions

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::services::session_manager::{SessionStoreState, GameSession};

// ============================================================================
// UTILITAIRES DE SESSION
// ============================================================================

pub async fn get_session_by_code_or_id_from_store(
    store: &Arc<RwLock<SessionStoreState>>,
    identifier: &str
) -> Option<GameSession> {
    let state = store.read().await;
    
    // Optimized single-pass lookup
    if identifier.len() == 36 && identifier.chars().nth(8) == Some('-') {
        // Likely UUID format - check sessions directly
        if let Some(session) = state.sessions.get(identifier) {
            return Some(session.clone());
        }
    }
    
    // Check by code
    if let Some(session_id) = state.sessions_by_code.get(identifier) {
        return state.sessions.get(session_id).cloned();
    }
    
    // Fallback: check if identifier is actually a session ID
    state.sessions.get(identifier).cloned()
}