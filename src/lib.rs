use thiserror::Error;

pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Debug, Error)]
pub enum TakeItEasyError {
    #[error("Game error: {0}")]
    Game(String),
    #[error("AI error: {0}")]
    Ai(String),
    #[error("Server error: {0}")]
    Server(String),
}

pub type Result<T> = std::result::Result<T, TakeItEasyError>;

pub mod data;
pub mod game;
pub mod generated;
pub mod mcts;
pub mod neural;
pub mod scoring;
pub mod servers;
pub mod services;
pub mod strategy;
pub mod training;
pub mod utils;

pub use servers::{GrpcConfig, GrpcServer, WebUiConfig, WebUiServer};
