//! SQLite database operations for authentication

use rusqlite::{params, Connection, Result as SqliteResult};
use serde::Serialize;
use std::sync::{Arc, Mutex};

use super::models::{OAuthAccount, OAuthProvider, TokenType, User, VerificationToken};

#[derive(Debug, Clone, Serialize)]
pub struct GameHistoryRow {
    pub score: i32,
    pub game_mode: String,
    pub won: bool,
    pub played_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeaderboardRow {
    pub username: String,
    pub best_score: i32,
    pub games_played: i32,
    pub games_won: i32,
}

/// Database connection wrapper
pub struct AuthDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl AuthDatabase {
    /// Create a new database connection and initialize tables
    pub fn new(path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_tables()?;
        db.ensure_guest_user()?;
        Ok(db)
    }

    /// Create in-memory database (for testing)
    pub fn in_memory() -> SqliteResult<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_tables()?;
        Ok(db)
    }

    /// Initialize database tables
    fn init_tables(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                username TEXT NOT NULL,
                password_hash TEXT,
                email_verified INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS oauth_accounts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                provider_user_id TEXT NOT NULL,
                access_token TEXT,
                refresh_token TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                UNIQUE(provider, provider_user_id)
            );

            CREATE TABLE IF NOT EXISTS verification_tokens (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                token TEXT UNIQUE NOT NULL,
                token_type TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                used INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS game_stats (
                user_id TEXT PRIMARY KEY,
                games_played INTEGER NOT NULL DEFAULT 0,
                games_won INTEGER NOT NULL DEFAULT 0,
                total_score INTEGER NOT NULL DEFAULT 0,
                best_score INTEGER NOT NULL DEFAULT 0,
                elo_rating INTEGER NOT NULL DEFAULT 1000,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS game_history (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                game_mode TEXT NOT NULL,
                score INTEGER NOT NULL,
                won INTEGER NOT NULL DEFAULT 0,
                played_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
            CREATE INDEX IF NOT EXISTS idx_oauth_provider ON oauth_accounts(provider, provider_user_id);
            CREATE INDEX IF NOT EXISTS idx_tokens_token ON verification_tokens(token);
            CREATE INDEX IF NOT EXISTS idx_game_history_user_score ON game_history(user_id, score DESC);
            CREATE INDEX IF NOT EXISTS idx_game_history_score ON game_history(score DESC);
            "#,
        )?;

        Ok(())
    }

    // ==================== Guest User ====================

    pub const GUEST_USER_ID: &'static str = "00000000-0000-0000-0000-000000000000";

    /// Ensure the guest user exists (for anonymous game recording)
    pub fn ensure_guest_user(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO users (id, email, username, password_hash, email_verified, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, 0, ?4, ?4)",
            params![Self::GUEST_USER_ID, "guest@localhost", "Joueur invit\u{00e9}", now],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO game_stats (user_id) VALUES (?1)",
            params![Self::GUEST_USER_ID],
        )?;
        Ok(())
    }

    // ==================== User Operations ====================

    /// Create a new user
    pub fn create_user(&self, user: &User) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, email, username, password_hash, email_verified, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                user.id,
                user.email,
                user.username,
                user.password_hash,
                user.email_verified as i32,
                user.created_at,
                user.updated_at,
            ],
        )?;

        // Initialize game stats
        conn.execute(
            "INSERT INTO game_stats (user_id) VALUES (?1)",
            params![user.id],
        )?;

        Ok(())
    }

    /// Find user by email
    pub fn find_user_by_email(&self, email: &str) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, email, username, password_hash, email_verified, created_at, updated_at
             FROM users WHERE email = ?1 COLLATE NOCASE",
        )?;

        let mut rows = stmt.query(params![email])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                email: row.get(1)?,
                username: row.get(2)?,
                password_hash: row.get(3)?,
                email_verified: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Find user by ID
    pub fn find_user_by_id(&self, id: &str) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, email, username, password_hash, email_verified, created_at, updated_at
             FROM users WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                email: row.get(1)?,
                username: row.get(2)?,
                password_hash: row.get(3)?,
                email_verified: row.get::<_, i32>(4)? != 0,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Update user's email verification status
    pub fn verify_user_email(&self, user_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE users SET email_verified = 1, updated_at = ?1 WHERE id = ?2",
            params![now, user_id],
        )?;
        Ok(())
    }

    /// Update user's password
    pub fn update_password(&self, user_id: &str, password_hash: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
            params![password_hash, now, user_id],
        )?;
        Ok(())
    }

    // ==================== OAuth Operations ====================

    /// Create OAuth account link
    pub fn create_oauth_account(&self, account: &OAuthAccount) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO oauth_accounts (id, user_id, provider, provider_user_id, access_token, refresh_token, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                account.id,
                account.user_id,
                account.provider.as_str(),
                account.provider_user_id,
                account.access_token,
                account.refresh_token,
                account.created_at,
            ],
        )?;
        Ok(())
    }

    /// Find OAuth account by provider and provider user ID
    pub fn find_oauth_account(
        &self,
        provider: OAuthProvider,
        provider_user_id: &str,
    ) -> SqliteResult<Option<OAuthAccount>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, provider, provider_user_id, access_token, refresh_token, created_at
             FROM oauth_accounts WHERE provider = ?1 AND provider_user_id = ?2",
        )?;

        let mut rows = stmt.query(params![provider.as_str(), provider_user_id])?;
        if let Some(row) = rows.next()? {
            let provider_str: String = row.get(2)?;
            Ok(Some(OAuthAccount {
                id: row.get(0)?,
                user_id: row.get(1)?,
                provider: OAuthProvider::parse(&provider_str).unwrap_or(OAuthProvider::Google),
                provider_user_id: row.get(3)?,
                access_token: row.get(4)?,
                refresh_token: row.get(5)?,
                created_at: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    // ==================== Token Operations ====================

    /// Create verification token
    pub fn create_verification_token(&self, token: &VerificationToken) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO verification_tokens (id, user_id, token, token_type, expires_at, used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                token.id,
                token.user_id,
                token.token,
                token.token_type.as_str(),
                token.expires_at,
                token.used as i32,
            ],
        )?;
        Ok(())
    }

    /// Find and validate verification token
    pub fn find_valid_token(&self, token: &str) -> SqliteResult<Option<VerificationToken>> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, user_id, token, token_type, expires_at, used
             FROM verification_tokens
             WHERE token = ?1 AND used = 0 AND expires_at > ?2",
        )?;

        let mut rows = stmt.query(params![token, now])?;
        if let Some(row) = rows.next()? {
            let token_type_str: String = row.get(3)?;
            Ok(Some(VerificationToken {
                id: row.get(0)?,
                user_id: row.get(1)?,
                token: row.get(2)?,
                token_type: TokenType::parse(&token_type_str)
                    .unwrap_or(TokenType::EmailVerification),
                expires_at: row.get(4)?,
                used: row.get::<_, i32>(5)? != 0,
            }))
        } else {
            Ok(None)
        }
    }

    /// Mark token as used
    pub fn mark_token_used(&self, token_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE verification_tokens SET used = 1 WHERE id = ?1",
            params![token_id],
        )?;
        Ok(())
    }

    /// Delete expired tokens (cleanup)
    pub fn cleanup_expired_tokens(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let deleted = conn.execute(
            "DELETE FROM verification_tokens WHERE expires_at < ?1 OR used = 1",
            params![now],
        )?;
        Ok(deleted)
    }

    // ==================== Game History Operations ====================

    /// Record a finished game and update aggregated stats
    pub fn record_game(
        &self,
        user_id: &str,
        game_mode: &str,
        score: i32,
        won: bool,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO game_history (id, user_id, game_mode, score, won, played_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, user_id, game_mode, score, won as i32, now],
        )?;

        conn.execute(
            "UPDATE game_stats SET
                games_played = games_played + 1,
                total_score = total_score + ?2,
                best_score = MAX(best_score, ?2),
                games_won = games_won + ?3
             WHERE user_id = ?1",
            params![user_id, score, won as i32],
        )?;

        Ok(())
    }

    /// Get a user's game history ordered by best score
    pub fn get_user_game_history(
        &self,
        user_id: &str,
        limit: i32,
    ) -> SqliteResult<Vec<GameHistoryRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT score, game_mode, won, played_at
             FROM game_history
             WHERE user_id = ?1
             ORDER BY score DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![user_id, limit], |row| {
            Ok(GameHistoryRow {
                score: row.get(0)?,
                game_mode: row.get(1)?,
                won: row.get::<_, i32>(2)? != 0,
                played_at: row.get(3)?,
            })
        })?;

        rows.collect()
    }

    /// Get the global leaderboard ordered by best score
    pub fn get_leaderboard(&self, limit: i32) -> SqliteResult<Vec<LeaderboardRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT u.username, gs.best_score, gs.games_played, gs.games_won
             FROM game_stats gs
             JOIN users u ON u.id = gs.user_id
             WHERE gs.games_played > 0
             ORDER BY gs.best_score DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(LeaderboardRow {
                username: row.get(0)?,
                best_score: row.get(1)?,
                games_played: row.get(2)?,
                games_won: row.get(3)?,
            })
        })?;

        rows.collect()
    }
}

impl Clone for AuthDatabase {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_find_user() {
        let db = AuthDatabase::in_memory().unwrap();

        let user = User {
            id: "user_123".to_string(),
            email: "test@example.com".to_string(),
            username: "testuser".to_string(),
            password_hash: Some("hash123".to_string()),
            email_verified: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        db.create_user(&user).unwrap();

        let found = db.find_user_by_email("test@example.com").unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.username, "testuser");
        assert!(!found.email_verified);
    }

    #[test]
    fn test_verify_email() {
        let db = AuthDatabase::in_memory().unwrap();

        let user = User {
            id: "user_456".to_string(),
            email: "verify@example.com".to_string(),
            username: "verifyuser".to_string(),
            password_hash: Some("hash456".to_string()),
            email_verified: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        db.create_user(&user).unwrap();
        db.verify_user_email("user_456").unwrap();

        let found = db.find_user_by_id("user_456").unwrap().unwrap();
        assert!(found.email_verified);
    }
}
