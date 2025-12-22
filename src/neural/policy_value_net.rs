use crate::neural::gnn::{GraphPolicyNet, GraphValueNet};
use crate::neural::manager::NNArchitecture;
use tch::{nn, Tensor};

use crate::neural::res_net_block::ResNetBlock;

/// Trait for policy evaluation to enable dependency inversion and testability
///
/// This trait provides an abstraction layer for policy networks, enabling:
/// - Mock implementations for testing without PyTorch dependencies
/// - Support for alternative neural architectures
/// - Dependency injection in MCTS and service layers
///
/// Note: Not Sync due to PyTorch's raw pointer usage. Use Mutex for thread-safety.
///
/// # Future Integration
/// This trait is part of the P1 dependency inversion refactoring.
/// It will be integrated into:
/// - `NeuralManager` for polymorphic network handling
/// - MCTS algorithm for testability
/// - Service layer for flexible AI opponents
#[allow(dead_code)]
pub trait PolicyEvaluator: Send {
    /// Evaluate the policy for a given board state
    fn forward(&self, input: &Tensor, train: bool) -> Tensor;
    /// Get the neural network architecture type
    fn arch(&self) -> &NNArchitecture;
}

/// Trait for value evaluation to enable dependency inversion and testability
///
/// This trait provides an abstraction layer for value networks, enabling:
/// - Mock implementations for testing without PyTorch dependencies
/// - Support for alternative neural architectures
/// - Dependency injection in MCTS and service layers
///
/// Note: Not Sync due to PyTorch's raw pointer usage. Use Mutex for thread-safety.
///
/// # Future Integration
/// This trait is part of the P1 dependency inversion refactoring.
/// It will be integrated into:
/// - `NeuralManager` for polymorphic network handling
/// - MCTS algorithm for testability
/// - Service layer for flexible AI opponents
#[allow(dead_code)]
pub trait ValueEvaluator: Send {
    /// Evaluate the value for a given board state
    fn forward(&self, input: &Tensor, train: bool) -> Tensor;
    /// Get the neural network architecture type
    fn arch(&self) -> &NNArchitecture;
}

pub struct PolicyNet {
    pub arch: NNArchitecture,
    net: PolicyNetImpl,
}

pub enum PolicyNetImpl {
    CNN(Box<PolicyNetCNN>),
    GNN(GraphPolicyNet),
}

impl PolicyNet {
    // Bronze GNN: Adapted for 5×5 2D spatial input (was 47×1)
    pub fn new(vs: &nn::VarStore, input_dim: (i64, i64, i64), arch: NNArchitecture) -> Self {
        match arch {
            NNArchitecture::CNN => Self {
                arch,
                net: PolicyNetImpl::CNN(Box::new(PolicyNetCNN::new(vs, input_dim))),
            },
            NNArchitecture::GNN => Self {
                arch,
                net: PolicyNetImpl::GNN(GraphPolicyNet::new(vs, input_dim.0, &[64, 64, 64], 0.1)),
            },
        }
    }

    pub fn forward(&self, input: &Tensor, train: bool) -> Tensor {
        match &self.net {
            PolicyNetImpl::CNN(net) => net.forward(input, train),
            PolicyNetImpl::GNN(net) => net.forward(input, train),
        }
    }

    pub fn save_model(&mut self, vs: &tch::nn::VarStore, path: &str) -> tch::Result<()> {
        vs.save(path)
    }
    pub fn load_model(&self, vs: &mut tch::nn::VarStore, path: &str) -> tch::Result<()> {
        vs.load(path)
    }
}

impl PolicyEvaluator for PolicyNet {
    fn forward(&self, input: &Tensor, train: bool) -> Tensor {
        self.forward(input, train)
    }

    fn arch(&self) -> &NNArchitecture {
        &self.arch
    }
}

pub struct PolicyNetCNN {
    conv1: nn::Conv2D,
    gn1: nn::GroupNorm,
    res_blocks: Vec<ResNetBlock>,
    flatten: nn::Linear,
    fc1: nn::Linear,
    policy_head: nn::Linear,
    dropout_rate: f64,
}
const INITIAL_CONV_CHANNELS: i64 = 160;
const POLICY_STAGE_CHANNELS: &[i64] = &[160, 128, 128, 96, 96, 64];

const INITIAL_CONV_CHANNELS_VALUE: i64 = 160;
const VALUE_STAGE_CHANNELS: &[i64] = &[160, 128, 128, 96, 96, 64];
impl PolicyNetCNN {
    // Bronze GNN: Adapted for 5×5 2D spatial input (was 47×1)
    pub fn new(vs: &nn::VarStore, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root();
        let (channels, height, width) = input_dim; // Expecting (channels, height, width)

        let conv1 = nn::conv2d(
            &p / "policy_conv1",
            channels,
            INITIAL_CONV_CHANNELS,
            3,
            nn::ConvConfig {
                padding: 1,
                ..Default::default()
            },
        );
        let gn1 = nn::group_norm(&p / "gn1", 16, INITIAL_CONV_CHANNELS, Default::default());

        let mut res_blocks = Vec::new();
        let mut in_channels = INITIAL_CONV_CHANNELS;

        for (idx, &out_channels) in POLICY_STAGE_CHANNELS.iter().enumerate() {
            let block_vs = vs.root() / format!("policy_block_{idx}");
            res_blocks.push(ResNetBlock::new_path(&block_vs, in_channels, out_channels));
            in_channels = out_channels;
        }

        // After conv operations, spatial dimensions remain 5×5 (with padding=1)
        let flatten_size = in_channels * height * width;
        log::info!(
            "🔧 PolicyNet flatten_size: {} (channels={}, height={}, width={})",
            flatten_size,
            in_channels,
            height,
            width
        );
        let flatten = nn::linear(
            &p / "policy_flatten",
            flatten_size,
            2048,
            Default::default(),
        );
        log::info!(
            "✅ PolicyNet flatten layer created with input size {}",
            flatten_size
        );
        let fc1 = nn::linear(&p / "policy_fc1", 2048, 512, Default::default());
        let policy_head = nn::linear(&p / "policy_head", 512, 19, nn::LinearConfig::default());

        initialize_weights(vs);

        Self {
            conv1,
            gn1,
            res_blocks,
            flatten,
            fc1,
            policy_head,
            dropout_rate: 0.3,
        }
    }

    #[allow(dead_code)]
    pub fn save_model(&mut self, vs: &tch::nn::VarStore, path: &str) -> tch::Result<()> {
        vs.save(path)
    }
    #[allow(dead_code)]
    pub fn load_model(&mut self, vs: &mut tch::nn::VarStore, path: &str) -> tch::Result<()> {
        vs.load(path)
    }
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut h = x.apply(&self.conv1).apply_t(&self.gn1, train).leaky_relu();

        for block in &self.res_blocks {
            h = block.forward(&h, train);
        }
        // In PolicyNet::forward and ValueNet::forward:
        let expected_flatten_size = {
            let size = h.size();
            (size[1], size[2], size[3]) // Extract dimensions as a tuple
        };
        let flattened_size =
            expected_flatten_size.0 * expected_flatten_size.1 * expected_flatten_size.2;

        h = h.view([-1, flattened_size]);

        h = h.apply(&self.flatten).relu();
        if train {
            h = h.dropout(self.dropout_rate, train);
        }

        h = h.apply(&self.fc1).relu();
        if train {
            h = h.dropout(self.dropout_rate, train);
        }

        h.apply(&self.policy_head).softmax(-1, tch::Kind::Float)
    }
}

// Initialize weights
pub fn initialize_weights(vs: &nn::VarStore) {
    for (name, mut param) in vs.variables() {
        let size = param.size();

        if size.len() == 4 {
            // Xavier/Glorot initialization for conv layers
            let fan_in = (size[1] * size[2] * size[3]) as f64;
            let fan_out = (size[0] * size[2] * size[3]) as f64;
            let bound = (6.0 / (fan_in + fan_out)).sqrt();
            tch::no_grad(|| {
                let _ = param.f_uniform_(-bound, bound).unwrap();
            });
        } else if size.len() == 2 {
            // Xavier initialization for linear layers
            let fan_in = size[1] as f64;
            let fan_out = size[0] as f64;
            let bound = (6.0 / (fan_in + fan_out)).sqrt();
            tch::no_grad(|| {
                let _ = param.f_uniform_(-bound, bound).unwrap();
            });
        } else if size.len() == 1 {
            // Zero initialization for biases
            tch::no_grad(|| {
                let _ = param.f_zero_().unwrap();
            });
        }

        // Validation after initialization
        if param.isnan().any().double_value(&[]) > 0.0 {
            log::error!("🚨 NaN detected in {} after initialization!", name);
        }
    }
}

//... other imports

pub struct ValueNet {
    #[allow(dead_code)]
    pub arch: NNArchitecture,
    net: ValueNetImpl,
}

pub enum ValueNetImpl {
    CNN(Box<ValueNetCNN>),
    GNN(GraphValueNet),
}

impl ValueNet {
    // Bronze GNN: Adapted for 5×5 2D spatial input (was 47×1)
    pub fn new(vs: &nn::VarStore, input_dim: (i64, i64, i64), arch: NNArchitecture) -> Self {
        match arch {
            NNArchitecture::CNN => Self {
                arch,
                net: ValueNetImpl::CNN(Box::new(ValueNetCNN::new(vs, input_dim))),
            },
            NNArchitecture::GNN => Self {
                arch,
                net: ValueNetImpl::GNN(GraphValueNet::new(vs, input_dim.0, &[64, 64, 64], 0.1)),
            },
        }
    }

    pub fn forward(&self, input: &Tensor, train: bool) -> Tensor {
        match &self.net {
            ValueNetImpl::CNN(net) => net.forward(input, train),
            ValueNetImpl::GNN(net) => net.forward(input, train),
        }
    }

    pub fn save_model(&mut self, vs: &tch::nn::VarStore, path: &str) -> tch::Result<()> {
        vs.save(path)
    }
    pub fn load_model(&self, vs: &mut tch::nn::VarStore, path: &str) -> tch::Result<()> {
        vs.load(path)
    }
}

impl ValueEvaluator for ValueNet {
    fn forward(&self, input: &Tensor, train: bool) -> Tensor {
        self.forward(input, train)
    }

    fn arch(&self) -> &NNArchitecture {
        &self.arch
    }
}

// Renommer l’implémentation CNN existante en PolicyNetCNN/ValueNetCNN
pub struct ValueNetCNN {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    res_blocks: Vec<ResNetBlock>,
    flatten: nn::Linear,
    fc1: nn::Linear, // Added FC layer
    value_head: nn::Linear,
    dropout_rate: f64,
}

impl ValueNetCNN {
    // Bronze GNN: Adapted for 5×5 2D spatial input (was 47×1)
    pub fn new(vs: &nn::VarStore, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root();
        let (channels, height, width) = input_dim; // Expecting (channels, height, width)

        let conv1 = nn::conv2d(
            &p / "value_conv1",
            channels,
            INITIAL_CONV_CHANNELS_VALUE,
            3,
            nn::ConvConfig {
                padding: 1,
                ..Default::default()
            },
        );

        let bn1 = nn::batch_norm2d(
            &p / "value_bn1",
            INITIAL_CONV_CHANNELS_VALUE,
            nn::BatchNormConfig {
                affine: true,
                ..Default::default()
            },
        );

        let mut res_blocks = Vec::new();
        let mut in_channels = INITIAL_CONV_CHANNELS_VALUE;

        for (idx, &out_channels) in VALUE_STAGE_CHANNELS.iter().enumerate() {
            let block_vs = vs.root() / format!("value_block_{idx}");
            res_blocks.push(ResNetBlock::new_path(&block_vs, in_channels, out_channels));
            in_channels = out_channels;
        }

        let flatten_size = in_channels * height * width; // Adjust if you have downsampling
        let flatten = nn::linear(&p / "value_flatten", flatten_size, 2048, Default::default()); // Increased size
        let fc1 = nn::linear(&p / "value_fc1", 2048, 512, Default::default()); // Added FC layer
        let value_head = nn::linear(&p / "value_head", 512, 1, nn::LinearConfig::default());

        initialize_weights(vs); // Use &vs here!

        Self {
            conv1,
            bn1,
            res_blocks,
            flatten,
            fc1,
            value_head,
            dropout_rate: 0.3,
        }
    }
    #[allow(dead_code)]
    pub fn save_model(&mut self, vs: &tch::nn::VarStore, path: &str) -> tch::Result<()> {
        vs.save(path)
    }
    #[allow(dead_code)]
    pub fn load_model(&mut self, vs: &mut tch::nn::VarStore, path: &str) -> tch::Result<()> {
        vs.load(path)
    }
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        // Input validation and normalization
        if x.isnan().any().double_value(&[]) > 0.0 || x.isinf().any().double_value(&[]) > 0.0 {
            log::error!("⚠️ Invalid input to ValueNet");
            return Tensor::zeros([1, 1], (tch::Kind::Float, tch::Device::Cpu));
        }

        // More robust normalization
        let x_mean = x.mean(tch::Kind::Float);
        let x_std = x.std(false).clamp_min(1e-6);
        let x = (x - x_mean) / x_std;

        // Forward pass with LeakyReLU for better gradient flow
        let mut h = x.apply(&self.conv1).apply_t(&self.bn1, train);

        // Use LeakyReLU instead of ReLU for better gradient flow
        h = h.leaky_relu_();

        // ResNet blocks
        for block in &self.res_blocks {
            h = block.forward(&h, train);

            // Check for NaN/Inf after each block
            if h.isnan().any().double_value(&[]) > 0.0 || h.isinf().any().double_value(&[]) > 0.0 {
                log::error!("⚠️ Invalid values detected in ResNet block");
                return Tensor::zeros([1, 1], (tch::Kind::Float, tch::Device::Cpu));
            }
        }

        // Flatten and fully connected layers
        let flattened_size = {
            let size = h.size();
            size[1] * size[2] * size[3]
        };

        h = h
            .view([-1, flattened_size])
            .apply(&self.flatten)
            .leaky_relu_();

        if train {
            h = h.dropout(self.dropout_rate, train);
        }

        h = h.apply(&self.fc1).leaky_relu_();

        if train {
            h = h.dropout(self.dropout_rate, train);
        }

        // Final value prediction with tanh activation for bounded output
        let output = h.apply(&self.value_head).tanh() * 2.0; // Scale to [-2, 2] range

        // Final validation
        if output.isnan().any().double_value(&[]) > 0.0
            || output.isinf().any().double_value(&[]) > 0.0
        {
            log::error!("⚠️ Invalid output from ValueNet");
            return Tensor::zeros([1, 1], (tch::Kind::Float, tch::Device::Cpu));
        }

        output
    }
}
#[allow(dead_code)]
fn kaiming_uniform(tensor: &mut Tensor, fan_in: f64) {
    let bound = (6.0f64).sqrt() / fan_in.sqrt();
    tch::no_grad(|| {
        let _ = tensor.f_uniform_(-bound, bound).unwrap();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tch::{nn, Device, Tensor};

    #[test]
    fn test_policy_net_creation() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5); // Enhanced feature stack
        let policy_net = PolicyNet::new(&vs, input_dim, NNArchitecture::CNN);

        // Assert that the PolicyNet was created correctly
        assert_eq!(policy_net.arch, NNArchitecture::CNN);
        // Internal fields are now private within the enum variant
    }

    #[test]
    fn test_policy_net_forward() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let policy_net = PolicyNet::new(&vs, input_dim, NNArchitecture::CNN);

        // Create a dummy input tensor
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = policy_net.forward(&input, true);

        // Assert that the output has the expected shape
        assert_eq!(output.size(), vec![1, 19]); // Assuming 19 is the number of actions
    }

    #[test]
    fn test_value_net_creation_and_forward() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let value_net = ValueNet::new(&vs, input_dim, NNArchitecture::CNN);

        // Create a dummy input tensor
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = value_net.forward(&input, true);

        // Assert that the output has the expected shape
        assert_eq!(output.size(), vec![1, 1]); // Assuming 1 output value
        assert_eq!(value_net.arch, NNArchitecture::CNN);
    }

    #[test]
    fn test_policy_net_forward_cnn() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let net = PolicyNet::new(&vs, input_dim, NNArchitecture::CNN);
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let out = net.forward(&input, false);
        assert_eq!(out.size(), vec![1, 19]);
    }

    #[test]
    fn test_policy_net_forward_gnn() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5); // 8 features, 19 nœuds attendus côté GNN
        let net = PolicyNet::new(&vs, input_dim, NNArchitecture::GNN);
        // GNN expects [batch, nodes, features] = [1, 19, 8]
        let input = Tensor::rand(&[1, 19, 8], (tch::Kind::Float, Device::Cpu));
        let out = net.forward(&input, false);
        assert_eq!(out.size()[0], 1);
    }

    #[test]
    fn test_policy_evaluator_trait() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let policy_net = PolicyNet::new(&vs, input_dim, NNArchitecture::CNN);

        // Use the trait interface
        let evaluator: &dyn PolicyEvaluator = &policy_net;
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = evaluator.forward(&input, false);

        assert_eq!(output.size(), vec![1, 19]);
        assert_eq!(*evaluator.arch(), NNArchitecture::CNN);
    }

    #[test]
    fn test_value_evaluator_trait() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let value_net = ValueNet::new(&vs, input_dim, NNArchitecture::CNN);

        // Use the trait interface
        let evaluator: &dyn ValueEvaluator = &value_net;
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = evaluator.forward(&input, false);

        assert_eq!(output.size(), vec![1, 1]);
        assert_eq!(*evaluator.arch(), NNArchitecture::CNN);
    }
}
