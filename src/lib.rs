//! # Take It Easy Game Library
//!
//! A comprehensive library for the "Take It Easy" board game implementation with AI capabilities.
//!
//! ## Features
//!
//! - **Game Engine**: Complete game logic and rules implementation
//! - **AI Engine**: Monte Carlo Tree Search (MCTS) with neural network integration
//! - **Server Components**: Web UI and gRPC servers for multiplayer gameplay
//! - **Training System**: Neural network training for AI improvement
//! - **Scoring System**: Advanced scoring algorithms and strategies
//!
//! ## Usage
//!
//! ```rust
//! use take_it_easy::{
//!     game::GameEngine,
//!     servers::{WebUiServer, WebUiConfig},
//!     mcts::MctsEngine,
//! };
//! ```

// ============================================================================
// PUBLIC API MODULES
// ============================================================================

/// Core game logic and rules
pub mod game;

/// Monte Carlo Tree Search AI engine
pub mod mcts;

/// Neural network components for AI training
pub mod neural;

/// Scoring algorithms and strategies
pub mod scoring;

/// Server components (Web UI, gRPC)
pub mod servers;

/// Training system for neural networks
pub mod training;

/// Utility functions and helpers
pub mod utils;

// ============================================================================
// INTERNAL MODULES (not exposed publicly)
// ============================================================================

mod generated;
mod services;
mod data;
mod strategy;
mod logging;

// ============================================================================
// PUBLIC API RE-EXPORTS
// ============================================================================

/// Main game engine facade
pub use game::*;

/// MCTS AI engine exports
pub use mcts::*;

/// Server configuration and implementations
pub use servers::{WebUiServer, WebUiConfig, GrpcServer, GrpcConfig};

/// Neural network components
pub use neural::*;


/// Training capabilities
pub use training::*;

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Main error type for the Take It Easy library
#[derive(Debug, thiserror::Error)]
pub enum TakeItEasyError {
    #[error("Game error: {0}")]
    Game(String),

    #[error("AI error: {0}")]
    Ai(String),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Training error: {0}")]
    Training(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, TakeItEasyError>;

// ============================================================================
// LIBRARY VERSION INFO
// ============================================================================

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Library description
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");