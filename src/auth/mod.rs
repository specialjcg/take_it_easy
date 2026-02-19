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

pub use grpc_middleware::try_authenticate_request;
pub use jwt::JwtManager;
pub use routes::{auth_router, AuthState};
