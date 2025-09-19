// Modules for server components
pub mod web_ui;
pub mod grpc;

// Re-export public APIs
pub use web_ui::{WebUiServer, WebUiConfig};
pub use grpc::{GrpcServer, GrpcConfig};