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
    Cnn(Box<PolicyNetCNN>),
    Gnn(GraphPolicyNet),
}

impl PolicyNet {
    // Bronze GNN: Adapted for 5×5 2D spatial input (was 47×1)
    pub fn new(vs: &nn::VarStore, input_dim: (i64, i64, i64), arch: NNArchitecture) -> Self {
        match arch {
            NNArchitecture::Cnn | NNArchitecture::CnnOnehot => Self {
                arch,
                net: PolicyNetImpl::Cnn(Box::new(PolicyNetCNN::new(vs, input_dim))),
            },
            NNArchitecture::Gnn => Self {
                arch,
                net: PolicyNetImpl::Gnn(GraphPolicyNet::new(vs, 8, &[64, 64, 64], 0.1)),  // 8 features per node for GNN (matches training data)
            },
        }
    }

    pub fn forward(&self, input: &Tensor, train: bool) -> Tensor {
        match &self.net {
            PolicyNetImpl::Cnn(net) => net.forward(input, train),
            PolicyNetImpl::Gnn(net) => {
                // Handle both input shapes: [batch, 8, 5, 5] from MCTS or [batch, 19, 8] from supervised
                let input_shape = input.size();
                let reshaped = if input_shape.len() == 4 {
                    // [batch, 8, 5, 5] -> [batch, 19, 8]
                    let batch_size = input_shape[0];
                    input
                        .view([batch_size, 8, 25])
                        .narrow(2, 0, 19)
                        .permute(&[0, 2, 1])
                } else {
                    // Already [batch, 19, 8] or [batch, 8, 19], check and permute if needed
                    if input_shape[1] == 8 {
                        input.permute(&[0, 2, 1])  // [batch, 8, 19] -> [batch, 19, 8]
                    } else {
                        input.shallow_clone()  // Already [batch, 19, 8]
                    }
                };
                net.forward(&reshaped, train)
            },
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
    // SPATIAL POLICY HEAD: Maintains spatial correspondence 5×5 → 19 positions
    policy_conv: nn::Conv2D,  // 1×1 conv: 64→1 channel (keeps 5×5 spatial structure)
    dropout_rate: f64,
}
// AlphaZero architecture with ResNet blocks
const INITIAL_CONV_CHANNELS: i64 = 128;
// RESTORED: AlphaZero-style architecture with 3 ResNet blocks for deeper learning
const POLICY_STAGE_CHANNELS: &[i64] = &[128, 128, 96]; // 3 ResNet blocks for pattern learning

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

        // SPATIAL POLICY HEAD: 1×1 conv to maintain spatial structure
        // No ResNet blocks - directly from conv1
        let final_channels = if POLICY_STAGE_CHANNELS.is_empty() {
            INITIAL_CONV_CHANNELS
        } else {
            *POLICY_STAGE_CHANNELS.last().unwrap()
        };
        let policy_conv = nn::conv2d(
            &p / "policy_conv",
            final_channels,  // 160 channels directly from conv1
            1,               // Output 1 channel (policy logit per spatial position)
            1,               // 1×1 kernel (no spatial mixing)
            Default::default(),
        );

        log::info!(
            "🔧 Simple CNN PolicyNet: {}×{}×{} → conv1({}) → GN → LeakyReLU → policy_conv → 1×{}×{} → 19 logits",
            channels, height, width, INITIAL_CONV_CHANNELS, height, width
        );

        initialize_weights(vs);

        Self {
            conv1,
            gn1,
            res_blocks,
            policy_conv,
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
        // AlphaZero architecture: conv1 → GroupNorm → LeakyReLU → ResNet blocks → policy_conv
        let mut h = x.apply(&self.conv1).apply_t(&self.gn1, train).leaky_relu();

        // Pass through ResNet blocks for deeper feature learning
        for res_block in &self.res_blocks {
            h = res_block.forward(&h, train);
        }

        // Spatial policy head: 1×1 conv maintains 5×5 structure
        h = h.apply(&self.policy_conv);  // → [batch, 1, 5, 5]

        // Flatten 5×5 spatial map to 25 logits
        let batch_size = h.size()[0];
        h = h.view([batch_size, 25]);  // [batch, 25]

        // Extract the 19 hexagonal positions using correct VERTICAL column mapping
        // HEX_TO_GRID_MAP: hex pos → (row, col) → flat_idx = row * 5 + col
        //
        // Layout:  Col0    Col1    Col2    Col3    Col4
        //                           7
        //           0      3      8       12      16
        //           1      4      9       13      17
        //           2      5     10       14      18
        //                  6     11       15
        //
        let hex_grid_indices: [i64; 19] = [
            5, 10, 15,         // hex 0-2   → col 0, rows 1-3
            6, 11, 16, 21,     // hex 3-6   → col 1, rows 1-4
            2, 7, 12, 17, 22,  // hex 7-11  → col 2, rows 0-4
            8, 13, 18, 23,     // hex 12-15 → col 3, rows 1-4
            9, 14, 19,         // hex 16-18 → col 4, rows 1-3
        ];

        let indices = Tensor::from_slice(&hex_grid_indices).to_device(h.device());
        h.index_select(1, &indices)  // Return [batch, 19] logits in hex order
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
                param.f_uniform_(-bound, bound)
                    .expect("Xavier initialization should not fail for conv weights");
            });
        } else if size.len() == 2 {
            // Xavier initialization for linear layers
            let fan_in = size[1] as f64;
            let fan_out = size[0] as f64;
            let bound = (6.0 / (fan_in + fan_out)).sqrt();
            tch::no_grad(|| {
                param.f_uniform_(-bound, bound)
                    .expect("Xavier initialization should not fail for conv weights");
            });
        } else if size.len() == 1 {
            // Zero initialization for biases ONLY (not GroupNorm weights!)
            if name.ends_with(".bias") {
                tch::no_grad(|| {
                    param.f_zero_()
                        .expect("Zero initialization should not fail for bias");
                });
            }
            // GroupNorm weights (.weight) are already initialized to 1.0 by PyTorch - leave them!
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
    Cnn(Box<ValueNetCNN>),
    Gnn(GraphValueNet),
}

impl ValueNet {
    // Bronze GNN: Adapted for 5×5 2D spatial input (was 47×1)
    pub fn new(vs: &nn::VarStore, input_dim: (i64, i64, i64), arch: NNArchitecture) -> Self {
        match arch {
            NNArchitecture::Cnn | NNArchitecture::CnnOnehot => Self {
                arch,
                net: ValueNetImpl::Cnn(Box::new(ValueNetCNN::new(vs, input_dim))),
            },
            NNArchitecture::Gnn => Self {
                arch,
                net: ValueNetImpl::Gnn(GraphValueNet::new(vs, 8, &[64, 64, 64], 0.1)),  // 8 features per node for GNN (matches training data)
            },
        }
    }

    pub fn forward(&self, input: &Tensor, train: bool) -> Tensor {
        match &self.net {
            ValueNetImpl::Cnn(net) => net.forward(input, train),
            ValueNetImpl::Gnn(net) => {
                // Handle both input shapes: [batch, 8, 5, 5] from MCTS or [batch, 19, 8] from supervised
                let input_shape = input.size();
                let reshaped = if input_shape.len() == 4 {
                    // [batch, 8, 5, 5] -> [batch, 19, 8]
                    let batch_size = input_shape[0];
                    input
                        .view([batch_size, 8, 25])
                        .narrow(2, 0, 19)
                        .permute(&[0, 2, 1])
                } else {
                    // Already [batch, 19, 8] or [batch, 8, 19], check and permute if needed
                    if input_shape[1] == 8 {
                        input.permute(&[0, 2, 1])  // [batch, 8, 19] -> [batch, 19, 8]
                    } else {
                        input.shallow_clone()  // Already [batch, 19, 8]
                    }
                };
                net.forward(&reshaped, train)
            },
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
        let flatten = nn::linear(&p / "value_flatten", flatten_size, 2048, Default::default());
        let fc1 = nn::linear(&p / "value_fc1", 2048, 512, Default::default());
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
        tensor.f_uniform_(-bound, bound)
            .expect("Kaiming initialization should not fail");
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
        let policy_net = PolicyNet::new(&vs, input_dim, NNArchitecture::Cnn);

        // Assert that the PolicyNet was created correctly
        assert_eq!(policy_net.arch, NNArchitecture::Cnn);
        // Internal fields are now private within the enum variant
    }

    #[test]
    fn test_policy_net_forward() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let policy_net = PolicyNet::new(&vs, input_dim, NNArchitecture::Cnn);

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
        let value_net = ValueNet::new(&vs, input_dim, NNArchitecture::Cnn);

        // Create a dummy input tensor
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = value_net.forward(&input, true);

        // Assert that the output has the expected shape
        assert_eq!(output.size(), vec![1, 1]); // Assuming 1 output value
        assert_eq!(value_net.arch, NNArchitecture::Cnn);
    }

    #[test]
    fn test_policy_net_forward_cnn() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let net = PolicyNet::new(&vs, input_dim, NNArchitecture::Cnn);
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let out = net.forward(&input, false);
        assert_eq!(out.size(), vec![1, 19]);
    }

    #[test]
    fn test_policy_net_forward_gnn() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5); // 8 features, 19 nœuds attendus côté GNN
        let net = PolicyNet::new(&vs, input_dim, NNArchitecture::Gnn);
        // GNN expects [batch, nodes, features] = [1, 19, 8]
        let input = Tensor::rand(&[1, 19, 8], (tch::Kind::Float, Device::Cpu));
        let out = net.forward(&input, false);
        assert_eq!(out.size()[0], 1);
    }

    #[test]
    fn test_policy_evaluator_trait() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let policy_net = PolicyNet::new(&vs, input_dim, NNArchitecture::Cnn);

        // Use the trait interface
        let evaluator: &dyn PolicyEvaluator = &policy_net;
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = evaluator.forward(&input, false);

        assert_eq!(output.size(), vec![1, 19]);
        assert_eq!(*evaluator.arch(), NNArchitecture::Cnn);
    }

    #[test]
    fn test_value_evaluator_trait() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (8, 5, 5);
        let value_net = ValueNet::new(&vs, input_dim, NNArchitecture::Cnn);

        // Use the trait interface
        let evaluator: &dyn ValueEvaluator = &value_net;
        let input = Tensor::rand(&[1, 8, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = evaluator.forward(&input, false);

        assert_eq!(output.size(), vec![1, 1]);
        assert_eq!(*evaluator.arch(), NNArchitecture::Cnn);
    }
}
