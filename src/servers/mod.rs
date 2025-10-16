// Modules for server components
pub mod grpc;
pub mod web_ui;

// Re-export public APIs
pub use grpc::{GrpcConfig, GrpcServer};
pub use web_ui::{WebUiConfig, WebUiServer};
