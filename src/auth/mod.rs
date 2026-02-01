//! Authentication module
//!
//! Provides complete authentication functionality:
//! - User registration with email/password
//! - Email verification
//! - Password reset via email
//! - OAuth login (Google, GitHub, Discord)
//! - JWT token-based sessions
//! - gRPC authentication middleware

pub mod database;
pub mod email;
pub mod grpc_middleware;
pub mod jwt;
pub mod models;
pub mod oauth;
pub mod password;
pub mod routes;

pub use database::AuthDatabase;
pub use grpc_middleware::{authenticate_request, try_authenticate_request};
pub use jwt::{JwtConfig, JwtManager};
pub use models::*;
pub use oauth::{OAuthConfig, OAuthManager};
pub use routes::{auth_router, AuthState};
