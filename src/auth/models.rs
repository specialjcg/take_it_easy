//! Authentication data models

use serde::{Deserialize, Serialize};

/// User account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub email_verified: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// OAuth account linked to a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthAccount {
    pub id: String,
    pub user_id: String,
    pub provider: OAuthProvider,
    pub provider_user_id: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub created_at: String,
}

/// Supported OAuth providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Google,
    Github,
    Discord,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::Github => "github",
            OAuthProvider::Discord => "discord",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "google" => Some(OAuthProvider::Google),
            "github" => Some(OAuthProvider::Github),
            "discord" => Some(OAuthProvider::Discord),
            _ => None,
        }
    }
}

/// Verification token (email verification, password reset)
#[derive(Debug, Clone)]
pub struct VerificationToken {
    pub id: String,
    pub user_id: String,
    pub token: String,
    pub token_type: TokenType,
    pub expires_at: String,
    pub used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    EmailVerification,
    PasswordReset,
}

impl TokenType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenType::EmailVerification => "email_verification",
            TokenType::PasswordReset => "password_reset",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "email_verification" => Some(TokenType::EmailVerification),
            "password_reset" => Some(TokenType::PasswordReset),
            _ => None,
        }
    }
}

/// JWT claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // user_id
    pub email: String,
    pub username: String,
    pub exp: usize,         // expiration timestamp
    pub iat: usize,         // issued at timestamp
}

/// API request/response types
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

/// OAuth callback data
#[derive(Debug, Deserialize)]
pub struct OAuthCallback {
    pub code: String,
    pub state: Option<String>,
}

/// User info from OAuth provider
#[derive(Debug, Clone)]
pub struct OAuthUserInfo {
    pub provider: OAuthProvider,
    pub provider_user_id: String,
    pub email: String,
    pub username: String,
}
