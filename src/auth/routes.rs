//! Authentication REST API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;

use super::{
    database::AuthDatabase,
    email::EmailSender,
    jwt::{JwtConfig, JwtManager},
    models::*,
    oauth::{OAuthConfig, OAuthManager},
    password::{hash_password, validate_password, verify_password},
};

/// Shared authentication state
pub struct AuthState {
    pub db: AuthDatabase,
    pub jwt: JwtManager,
    pub oauth: OAuthManager,
    pub email: EmailSender,
}

impl AuthState {
    pub fn new(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let db = AuthDatabase::new(db_path)?;
        let jwt = JwtManager::new(JwtConfig::from_env());
        let oauth = OAuthManager::new(OAuthConfig::from_env());
        let email = EmailSender::from_env();

        Ok(Self {
            db,
            jwt,
            oauth,
            email,
        })
    }
}

/// Create auth router
pub fn auth_router(state: Arc<AuthState>) -> Router {
    Router::new()
        // Basic auth
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/verify-email", get(verify_email))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
        // OAuth
        .route("/oauth/:provider", get(oauth_redirect))
        .route("/callback/:provider", get(oauth_callback))
        .route("/providers", get(get_providers))
        // User info
        .route("/me", get(get_current_user))
        .with_state(state)
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn error_response(status: StatusCode, message: &str) -> impl IntoResponse {
    (status, Json(ErrorResponse { error: message.to_string() }))
}

/// POST /auth/register - Register new user
async fn register(
    State(state): State<Arc<AuthState>>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    // Validate email format
    if !req.email.contains('@') || req.email.len() < 5 {
        return error_response(StatusCode::BAD_REQUEST, "Invalid email format").into_response();
    }

    // Validate username
    if req.username.len() < 3 || req.username.len() > 30 {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Username must be 3-30 characters",
        )
        .into_response();
    }

    // Validate password
    if let Err(e) = validate_password(&req.password) {
        return error_response(StatusCode::BAD_REQUEST, e).into_response();
    }

    // Check if email already exists
    match state.db.find_user_by_email(&req.email) {
        Ok(Some(_)) => {
            return error_response(StatusCode::CONFLICT, "Email already registered").into_response()
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Database error")
                .into_response();
        }
    }

    // Hash password
    let password_hash = match hash_password(&req.password) {
        Ok(hash) => hash,
        Err(e) => {
            log::error!("Password hashing error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
                .into_response();
        }
    };

    // Create user
    let now = chrono::Utc::now().to_rfc3339();
    let user = User {
        id: uuid::Uuid::new_v4().to_string(),
        email: req.email.clone(),
        username: req.username.clone(),
        password_hash: Some(password_hash),
        email_verified: false,
        created_at: now.clone(),
        updated_at: now,
    };

    if let Err(e) = state.db.create_user(&user) {
        log::error!("Failed to create user: {}", e);
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
            .into_response();
    }

    // Create verification token
    let token = generate_token();
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
    let verification_token = VerificationToken {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: user.id.clone(),
        token: token.clone(),
        token_type: TokenType::EmailVerification,
        expires_at,
        used: false,
    };

    if let Err(e) = state.db.create_verification_token(&verification_token) {
        log::error!("Failed to create verification token: {}", e);
    } else {
        // Send verification email
        if let Err(e) = state
            .email
            .send_verification_email(&user.email, &user.username, &token)
            .await
        {
            log::error!("Failed to send verification email: {}", e);
        }
    }

    // Generate JWT (user can login but some features may require verified email)
    let jwt_token = match state.jwt.create_token(&user.id, &user.email, &user.username) {
        Ok(token) => token,
        Err(e) => {
            log::error!("JWT creation error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
                .into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(AuthResponse {
            token: jwt_token,
            user,
        }),
    )
        .into_response()
}

/// POST /auth/login - Login with email/password
async fn login(
    State(state): State<Arc<AuthState>>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    // Find user
    let user = match state.db.find_user_by_email(&req.email) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return error_response(StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
                .into_response();
        }
    };

    // Check password
    let password_hash = match &user.password_hash {
        Some(hash) => hash,
        None => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "Account uses OAuth login only",
            )
            .into_response()
        }
    };

    match verify_password(&req.password, password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return error_response(StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
        }
        Err(e) => {
            log::error!("Password verification error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
                .into_response();
        }
    }

    // Generate JWT
    let token = match state.jwt.create_token(&user.id, &user.email, &user.username) {
        Ok(token) => token,
        Err(e) => {
            log::error!("JWT creation error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
                .into_response();
        }
    };

    Json(AuthResponse { token, user }).into_response()
}

/// GET /auth/verify-email?token=xxx - Verify email
async fn verify_email(
    State(state): State<Arc<AuthState>>,
    Query(req): Query<VerifyEmailRequest>,
) -> impl IntoResponse {
    // Find token
    let verification_token = match state.db.find_valid_token(&req.token) {
        Ok(Some(token)) if token.token_type == TokenType::EmailVerification => token,
        Ok(_) => {
            return error_response(StatusCode::BAD_REQUEST, "Invalid or expired token")
                .into_response()
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
                .into_response();
        }
    };

    // Mark email as verified
    if let Err(e) = state.db.verify_user_email(&verification_token.user_id) {
        log::error!("Failed to verify email: {}", e);
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
            .into_response();
    }

    // Mark token as used
    let _ = state.db.mark_token_used(&verification_token.id);

    Json(MessageResponse {
        message: "Email verified successfully".to_string(),
    })
    .into_response()
}

/// POST /auth/forgot-password - Request password reset
async fn forgot_password(
    State(state): State<Arc<AuthState>>,
    Json(req): Json<ForgotPasswordRequest>,
) -> impl IntoResponse {
    // Always return success to prevent email enumeration
    let success_response = Json(MessageResponse {
        message: "If an account exists with this email, a reset link has been sent".to_string(),
    });

    // Find user
    let user = match state.db.find_user_by_email(&req.email) {
        Ok(Some(user)) => user,
        Ok(None) => return success_response.into_response(),
        Err(e) => {
            log::error!("Database error: {}", e);
            return success_response.into_response();
        }
    };

    // Create reset token
    let token = generate_token();
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let reset_token = VerificationToken {
        id: uuid::Uuid::new_v4().to_string(),
        user_id: user.id.clone(),
        token: token.clone(),
        token_type: TokenType::PasswordReset,
        expires_at,
        used: false,
    };

    if let Err(e) = state.db.create_verification_token(&reset_token) {
        log::error!("Failed to create reset token: {}", e);
        return success_response.into_response();
    }

    // Send reset email
    if let Err(e) = state
        .email
        .send_password_reset_email(&user.email, &user.username, &token)
        .await
    {
        log::error!("Failed to send reset email: {}", e);
    }

    success_response.into_response()
}

/// POST /auth/reset-password - Reset password with token
async fn reset_password(
    State(state): State<Arc<AuthState>>,
    Json(req): Json<ResetPasswordRequest>,
) -> impl IntoResponse {
    // Validate new password
    if let Err(e) = validate_password(&req.new_password) {
        return error_response(StatusCode::BAD_REQUEST, e).into_response();
    }

    // Find and validate token
    let reset_token = match state.db.find_valid_token(&req.token) {
        Ok(Some(token)) if token.token_type == TokenType::PasswordReset => token,
        Ok(_) => {
            return error_response(StatusCode::BAD_REQUEST, "Invalid or expired token")
                .into_response()
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
                .into_response();
        }
    };

    // Hash new password
    let password_hash = match hash_password(&req.new_password) {
        Ok(hash) => hash,
        Err(e) => {
            log::error!("Password hashing error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
                .into_response();
        }
    };

    // Update password
    if let Err(e) = state.db.update_password(&reset_token.user_id, &password_hash) {
        log::error!("Failed to update password: {}", e);
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Reset failed").into_response();
    }

    // Mark token as used
    let _ = state.db.mark_token_used(&reset_token.id);

    Json(MessageResponse {
        message: "Password reset successfully".to_string(),
    })
    .into_response()
}

/// GET /auth/oauth/:provider - Redirect to OAuth provider
async fn oauth_redirect(
    State(state): State<Arc<AuthState>>,
    Path(provider): Path<String>,
) -> impl IntoResponse {
    let provider = match OAuthProvider::from_str(&provider) {
        Some(p) => p,
        None => {
            return error_response(StatusCode::BAD_REQUEST, "Unknown OAuth provider")
                .into_response()
        }
    };

    match state.oauth.get_auth_url(provider) {
        Ok((url, _csrf_token)) => {
            // In production, store csrf_token in session/cookie for validation
            axum::response::Redirect::temporary(&url).into_response()
        }
        Err(e) => {
            log::error!("OAuth error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, &e).into_response()
        }
    }
}

/// GET /auth/callback/:provider - OAuth callback
async fn oauth_callback(
    State(state): State<Arc<AuthState>>,
    Path(provider): Path<String>,
    Query(callback): Query<OAuthCallback>,
) -> impl IntoResponse {
    let provider = match OAuthProvider::from_str(&provider) {
        Some(p) => p,
        None => {
            return error_response(StatusCode::BAD_REQUEST, "Unknown OAuth provider")
                .into_response()
        }
    };

    // Exchange code for user info
    let user_info = match state.oauth.exchange_code(provider, &callback.code).await {
        Ok(info) => info,
        Err(e) => {
            log::error!("OAuth exchange error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "OAuth login failed")
                .into_response();
        }
    };

    // Check if OAuth account already linked
    let user = match state
        .db
        .find_oauth_account(provider, &user_info.provider_user_id)
    {
        Ok(Some(oauth_account)) => {
            // Existing OAuth account - get user
            match state.db.find_user_by_id(&oauth_account.user_id) {
                Ok(Some(user)) => user,
                Ok(None) => {
                    return error_response(StatusCode::INTERNAL_SERVER_ERROR, "User not found")
                        .into_response()
                }
                Err(e) => {
                    log::error!("Database error: {}", e);
                    return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
                        .into_response();
                }
            }
        }
        Ok(None) => {
            // New OAuth account - check if email exists
            match state.db.find_user_by_email(&user_info.email) {
                Ok(Some(existing_user)) => {
                    // Link OAuth to existing user
                    let oauth_account = OAuthAccount {
                        id: uuid::Uuid::new_v4().to_string(),
                        user_id: existing_user.id.clone(),
                        provider,
                        provider_user_id: user_info.provider_user_id,
                        access_token: None,
                        refresh_token: None,
                        created_at: chrono::Utc::now().to_rfc3339(),
                    };
                    if let Err(e) = state.db.create_oauth_account(&oauth_account) {
                        log::error!("Failed to link OAuth account: {}", e);
                    }
                    existing_user
                }
                Ok(None) => {
                    // Create new user
                    let now = chrono::Utc::now().to_rfc3339();
                    let new_user = User {
                        id: uuid::Uuid::new_v4().to_string(),
                        email: user_info.email.clone(),
                        username: user_info.username.clone(),
                        password_hash: None, // OAuth-only account
                        email_verified: true, // OAuth emails are trusted
                        created_at: now.clone(),
                        updated_at: now,
                    };

                    if let Err(e) = state.db.create_user(&new_user) {
                        log::error!("Failed to create user: {}", e);
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Registration failed",
                        )
                        .into_response();
                    }

                    // Link OAuth account
                    let oauth_account = OAuthAccount {
                        id: uuid::Uuid::new_v4().to_string(),
                        user_id: new_user.id.clone(),
                        provider,
                        provider_user_id: user_info.provider_user_id,
                        access_token: None,
                        refresh_token: None,
                        created_at: chrono::Utc::now().to_rfc3339(),
                    };
                    if let Err(e) = state.db.create_oauth_account(&oauth_account) {
                        log::error!("Failed to create OAuth account: {}", e);
                    }

                    new_user
                }
                Err(e) => {
                    log::error!("Database error: {}", e);
                    return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
                        .into_response();
                }
            }
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
                .into_response();
        }
    };

    // Generate JWT
    let token = match state.jwt.create_token(&user.id, &user.email, &user.username) {
        Ok(token) => token,
        Err(e) => {
            log::error!("JWT creation error: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
                .into_response();
        }
    };

    // Redirect to frontend with token
    let redirect_url = format!(
        "{}?token={}&user_id={}&username={}",
        std::env::var("OAUTH_SUCCESS_REDIRECT").unwrap_or_else(|_| "/".to_string()),
        token,
        user.id,
        user.username
    );

    axum::response::Redirect::temporary(&redirect_url).into_response()
}

/// GET /auth/providers - List configured OAuth providers
async fn get_providers(State(state): State<Arc<AuthState>>) -> impl IntoResponse {
    #[derive(Serialize)]
    struct ProvidersResponse {
        providers: Vec<String>,
    }

    let providers = state
        .oauth
        .get_configured_providers()
        .iter()
        .map(|p| p.as_str().to_string())
        .collect();

    Json(ProvidersResponse { providers })
}

/// GET /auth/me - Get current user from JWT
async fn get_current_user(
    State(state): State<Arc<AuthState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // Extract token from Authorization header
    let token = match headers.get("Authorization") {
        Some(value) => {
            let value = value.to_str().unwrap_or("");
            if value.starts_with("Bearer ") {
                &value[7..]
            } else {
                return error_response(StatusCode::UNAUTHORIZED, "Invalid authorization header")
                    .into_response();
            }
        }
        None => {
            return error_response(StatusCode::UNAUTHORIZED, "Missing authorization header")
                .into_response()
        }
    };

    // Verify token
    let claims = match state.jwt.verify_token(token) {
        Ok(data) => data.claims,
        Err(_) => {
            return error_response(StatusCode::UNAUTHORIZED, "Invalid or expired token")
                .into_response()
        }
    };

    // Get user from database
    match state.db.find_user_by_id(&claims.sub) {
        Ok(Some(user)) => Json(user).into_response(),
        Ok(None) => {
            error_response(StatusCode::NOT_FOUND, "User not found").into_response()
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get user").into_response()
        }
    }
}

/// Generate a random token
fn generate_token() -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
    URL_SAFE_NO_PAD.encode(bytes)
}
