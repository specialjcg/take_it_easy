//! Neural Network Manager
//!
//! Centralized management of neural networks for the Take It Easy game.
//! Handles initialization, loading, and configuration of policy and value networks.

use std::path::Path;
use tch::{nn, Device};
use tch::nn::OptimizerConfig;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};

/// Configuration for neural network initialization
#[derive(Debug, Clone)]
pub struct NeuralConfig {
    /// Input dimensions (channels, height, width)
    pub input_dim: (i64, i64, i64),
    /// Device to use for computation (CPU/GPU)
    pub device: Device,
    /// Model weights directory path
    pub model_path: String,
    /// Policy network learning rate
    pub policy_lr: f64,
    /// Value network learning rate
    pub value_lr: f64,
    /// Value network weight decay
    pub value_wd: f64,
}

impl Default for NeuralConfig {
    fn default() -> Self {
        Self {
            input_dim: (5, 47, 1),
            device: Device::Cpu,
            model_path: "model_weights".to_string(),
            policy_lr: 1e-3,
            value_lr: 2e-4,
            value_wd: 1e-6,
        }
    }
}

/// Neural Network Manager that encapsulates all network components
pub struct NeuralManager {
    config: NeuralConfig,
    vs_policy: nn::VarStore,
    vs_value: nn::VarStore,
    policy_net: PolicyNet,
    value_net: ValueNet,
    optimizer_policy: nn::Optimizer,
    optimizer_value: nn::Optimizer,
}

impl NeuralManager {
    /// Create a new neural network manager with default configuration
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_config(NeuralConfig::default())
    }

    /// Create a new neural network manager with custom configuration
    pub fn with_config(config: NeuralConfig) -> Result<Self, Box<dyn std::error::Error>> {
        log::info!("🧠 Initializing neural network manager...");
        log::debug!("Neural config: input_dim={:?}, device={:?}", config.input_dim, config.device);

        // Initialize VarStores
        let mut vs_policy = nn::VarStore::new(config.device);
        let mut vs_value = nn::VarStore::new(config.device);

        // Create networks
        let mut policy_net = PolicyNet::new(&vs_policy, config.input_dim);
        let mut value_net = ValueNet::new(&vs_value, config.input_dim);

        // Load weights if model directory exists
        if Path::new(&config.model_path).exists() {
            log::info!("📂 Loading neural network weights from {}", config.model_path);

            let policy_path = format!("{}/policy/policy.params", config.model_path);
            if let Err(e) = policy_net.load_model(&mut vs_policy, &policy_path) {
                log::warn!("⚠️ Failed to load PolicyNet from {}: {:?}", policy_path, e);
            } else {
                log::info!("✅ PolicyNet loaded successfully");
            }

            let value_path = format!("{}/value/value.params", config.model_path);
            if let Err(e) = value_net.load_model(&mut vs_value, &value_path) {
                log::warn!("⚠️ Failed to load ValueNet from {}: {:?}", value_path, e);
            } else {
                log::info!("✅ ValueNet loaded successfully");
            }
        } else {
            log::info!("📁 Model directory {} not found, using fresh networks", config.model_path);
        }

        // Create optimizers
        let optimizer_policy = nn::Adam::default()
            .build(&vs_policy, config.policy_lr)?;

        let optimizer_value = nn::Adam {
            wd: config.value_wd,
            ..Default::default()
        }
        .build(&vs_value, config.value_lr)?;

        log::info!("✅ Neural network manager initialized successfully");

        Ok(Self {
            config,
            vs_policy,
            vs_value,
            policy_net,
            value_net,
            optimizer_policy,
            optimizer_value,
        })
    }

    /// Get a reference to the neural configuration
    pub fn config(&self) -> &NeuralConfig {
        &self.config
    }

    /// Get a reference to the policy network
    pub fn policy_net(&self) -> &PolicyNet {
        &self.policy_net
    }

    /// Get a mutable reference to the policy network
    pub fn policy_net_mut(&mut self) -> &mut PolicyNet {
        &mut self.policy_net
    }

    /// Get a reference to the value network
    pub fn value_net(&self) -> &ValueNet {
        &self.value_net
    }

    /// Get a mutable reference to the value network
    pub fn value_net_mut(&mut self) -> &mut ValueNet {
        &mut self.value_net
    }

    /// Get a reference to the policy VarStore
    pub fn policy_varstore(&self) -> &nn::VarStore {
        &self.vs_policy
    }

    /// Get a mutable reference to the policy VarStore
    pub fn policy_varstore_mut(&mut self) -> &mut nn::VarStore {
        &mut self.vs_policy
    }

    /// Get a reference to the value VarStore
    pub fn value_varstore(&self) -> &nn::VarStore {
        &self.vs_value
    }

    /// Get a mutable reference to the value VarStore
    pub fn value_varstore_mut(&mut self) -> &mut nn::VarStore {
        &mut self.vs_value
    }

    /// Get a reference to the policy optimizer
    pub fn policy_optimizer(&self) -> &nn::Optimizer {
        &self.optimizer_policy
    }

    /// Get a mutable reference to the policy optimizer
    pub fn policy_optimizer_mut(&mut self) -> &mut nn::Optimizer {
        &mut self.optimizer_policy
    }

    /// Get a reference to the value optimizer
    pub fn value_optimizer(&self) -> &nn::Optimizer {
        &self.optimizer_value
    }

    /// Get a mutable reference to the value optimizer
    pub fn value_optimizer_mut(&mut self) -> &mut nn::Optimizer {
        &mut self.optimizer_value
    }

    /// Take ownership of the networks for transfer to other contexts
    /// This consumes the manager and returns the components
    pub fn into_components(self) -> NeuralComponents {
        NeuralComponents {
            config: self.config,
            vs_policy: self.vs_policy,
            vs_value: self.vs_value,
            policy_net: self.policy_net,
            value_net: self.value_net,
            optimizer_policy: self.optimizer_policy,
            optimizer_value: self.optimizer_value,
        }
    }

    /// Save the current model weights to disk
    pub fn save_models(&self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("💾 Saving neural network models to {}", self.config.model_path);

        // Create directories if they don't exist
        std::fs::create_dir_all(format!("{}/policy", self.config.model_path))?;
        std::fs::create_dir_all(format!("{}/value", self.config.model_path))?;

        // Save policy network
        let policy_path = format!("{}/policy/policy.params", self.config.model_path);
        self.vs_policy.save(&policy_path)?;
        log::info!("✅ PolicyNet saved to {}", policy_path);

        // Save value network
        let value_path = format!("{}/value/value.params", self.config.model_path);
        self.vs_value.save(&value_path)?;
        log::info!("✅ ValueNet saved to {}", value_path);

        Ok(())
    }

    /// Get summary information about the neural networks
    pub fn summary(&self) -> NeuralSummary {
        NeuralSummary {
            input_dim: self.config.input_dim,
            device: format!("{:?}", self.config.device),
            model_path: self.config.model_path.clone(),
            policy_lr: self.config.policy_lr,
            value_lr: self.config.value_lr,
            policy_params: self.vs_policy.variables().len(),
            value_params: self.vs_value.variables().len(),
        }
    }
}

/// Components extracted from NeuralManager for ownership transfer
pub struct NeuralComponents {
    pub config: NeuralConfig,
    pub vs_policy: nn::VarStore,
    pub vs_value: nn::VarStore,
    pub policy_net: PolicyNet,
    pub value_net: ValueNet,
    pub optimizer_policy: nn::Optimizer,
    pub optimizer_value: nn::Optimizer,
}

/// Summary information about neural networks
#[derive(Debug)]
pub struct NeuralSummary {
    pub input_dim: (i64, i64, i64),
    pub device: String,
    pub model_path: String,
    pub policy_lr: f64,
    pub value_lr: f64,
    pub policy_params: usize,
    pub value_params: usize,
}

impl std::fmt::Display for NeuralSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,
            "Neural Networks Summary:\n\
             📐 Input Dimensions: {:?}\n\
             💻 Device: {}\n\
             📂 Model Path: {}\n\
             🎯 Policy LR: {:.2e}, Value LR: {:.2e}\n\
             🔢 Policy Params: {}, Value Params: {}",
            self.input_dim, self.device, self.model_path,
            self.policy_lr, self.value_lr,
            self.policy_params, self.value_params
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neural_config_default() {
        let config = NeuralConfig::default();
        assert_eq!(config.input_dim, (5, 47, 1));
        assert_eq!(config.device, Device::Cpu);
        assert_eq!(config.model_path, "model_weights");
        assert_eq!(config.policy_lr, 1e-3);
        assert_eq!(config.value_lr, 2e-4);
        assert_eq!(config.value_wd, 1e-6);
    }

    #[test]
    fn test_neural_config_custom() {
        let config = NeuralConfig {
            input_dim: (3, 64, 64),
            device: Device::Cpu,
            model_path: "test_models".to_string(),
            policy_lr: 1e-4,
            value_lr: 1e-5,
            value_wd: 1e-7,
        };

        assert_eq!(config.input_dim, (3, 64, 64));
        assert_eq!(config.model_path, "test_models");
        assert_eq!(config.policy_lr, 1e-4);
    }

    #[test]
    fn test_neural_manager_creation() {
        let manager = NeuralManager::new();
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.config().input_dim, (5, 47, 1));
        assert_eq!(manager.config().model_path, "model_weights");
    }

    #[test]
    fn test_neural_manager_with_custom_config() {
        let config = NeuralConfig {
            input_dim: (1, 32, 32),
            device: Device::Cpu,
            model_path: "test_path".to_string(),
            policy_lr: 5e-4,
            value_lr: 1e-4,
            value_wd: 5e-7,
        };

        let manager = NeuralManager::with_config(config);
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.config().input_dim, (1, 32, 32));
        assert_eq!(manager.config().policy_lr, 5e-4);
        assert_eq!(manager.config().model_path, "test_path");
    }

    #[test]
    fn test_neural_summary_display() {
        let summary = NeuralSummary {
            input_dim: (5, 47, 1),
            device: "Cpu".to_string(),
            model_path: "model_weights".to_string(),
            policy_lr: 1e-3,
            value_lr: 2e-4,
            policy_params: 1000,
            value_params: 500,
        };

        let display = format!("{}", summary);
        assert!(display.contains("Neural Networks Summary"));
        assert!(display.contains("Input Dimensions: (5, 47, 1)"));
        assert!(display.contains("Device: Cpu"));
        assert!(display.contains("Policy Params: 1000"));
    }

    #[test]
    fn test_neural_manager_accessors() {
        let manager = NeuralManager::new().unwrap();

        // Test immutable accessors
        let _policy_net = manager.policy_net();
        let _value_net = manager.value_net();
        let _policy_vs = manager.policy_varstore();
        let _value_vs = manager.value_varstore();
        let _policy_opt = manager.policy_optimizer();
        let _value_opt = manager.value_optimizer();
        let _config = manager.config();

        // Test summary
        let summary = manager.summary();
        assert_eq!(summary.input_dim, (5, 47, 1));
        assert_eq!(summary.model_path, "model_weights");
    }

    #[test]
    fn test_neural_manager_mutable_accessors() {
        let mut manager = NeuralManager::new().unwrap();

        // Test mutable accessors
        let _policy_net = manager.policy_net_mut();
        let _value_net = manager.value_net_mut();
        let _policy_vs = manager.policy_varstore_mut();
        let _value_vs = manager.value_varstore_mut();
        let _policy_opt = manager.policy_optimizer_mut();
        let _value_opt = manager.value_optimizer_mut();
    }

    #[test]
    fn test_neural_components_extraction() {
        let manager = NeuralManager::new().unwrap();
        let components = manager.into_components();

        assert_eq!(components.config.input_dim, (5, 47, 1));
        assert_eq!(components.config.model_path, "model_weights");
        // Components should be properly moved
    }
}