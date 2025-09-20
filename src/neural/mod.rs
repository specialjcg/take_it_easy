pub mod res_net_block;
pub mod policy_value_net;
pub mod tensor_conversion;
pub mod training;
pub mod manager;

// Re-export key components for convenience
pub use manager::{NeuralManager, NeuralConfig};