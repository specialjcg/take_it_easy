pub mod manager;
pub mod policy_value_net;
pub mod res_net_block;
pub mod tensor_conversion;
pub mod training;

// Re-export key components for convenience
pub use manager::{NeuralConfig, NeuralManager};
