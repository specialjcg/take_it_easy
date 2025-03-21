
use tch::{nn, Tensor};
use tch::nn::VarStore;
use tch::Result;

use crate::policy_value_net::res_net_block::ResNetBlock;

mod res_net_block;

pub struct PolicyNet {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    res_blocks: Vec<ResNetBlock>,
    flatten: nn::Linear,
    fc1: nn::Linear,
    policy_head: nn::Linear,
    dropout_rate: f64,
}
const NUM_RES_BLOCKS: usize = 8; // Or any number you want
const INITIAL_CONV_CHANNELS: i64 = 128;

const NUM_RES_BLOCKS_VALUE: usize = 8; // Or adjust as needed
const INITIAL_CONV_CHANNELS_VALUE: i64 = 128;
impl<'a> PolicyNet {
    // policy_value_net.rs (PolicyNet and ValueNet)
    pub fn new(vs: &nn::VarStore, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root(); // p is a Path
        let (channels, height, width) = input_dim;

        let conv1 = nn::conv2d(&p / "policy_conv1", channels, INITIAL_CONV_CHANNELS, 3, nn::ConvConfig { padding: 1,..Default::default() });
        let bn1 = nn::batch_norm2d(&p / "policy_bn1", INITIAL_CONV_CHANNELS, nn::BatchNormConfig::default());

        let mut res_blocks = Vec::new();
        let mut in_channels = INITIAL_CONV_CHANNELS;

        for _ in 0..NUM_RES_BLOCKS {
            let out_channels = 32; // Or adjust as needed (e.g., increase in stages)
            res_blocks.push(ResNetBlock::new(&vs, in_channels, out_channels)); // Use &vs here!
            in_channels = out_channels;
        }


        let flatten_size = in_channels * height * width; // Adjust if you have downsampling in ResNetBlocks
        let flatten = nn::linear(&p / "policy_flatten", flatten_size, 1024, Default::default());
        let fc1 = nn::linear(&p / "policy_fc1", 1024, 256, Default::default());
        let policy_head = nn::linear(&p / "policy_head", 256, 19, nn::LinearConfig::default());

        initialize_weights(&vs); // Use &vs here!

        Self {
            conv1,
            bn1,
            res_blocks,
            flatten,
            fc1,
            policy_head,
            dropout_rate: 0.2,
        }
    }

    pub fn save_model(&self, vs: &nn::VarStore, path: &str) -> Result<()> {
        // Save the model's state dictionary to the specified path
        vs.save(path)?;
        Ok(())
    }

    pub fn load_model(&self, vs: &mut nn::VarStore, path: &str) -> Result<()> {
        // Load the model's state dictionary from the specified path
        vs.load(path)?;
        // Recreate the model with the loaded weights
        Ok(())
    }
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut h = x.apply(&self.conv1).apply_t(&self.bn1, train).leaky_relu();

        for block in &self.res_blocks {
            h = block.forward(&h, train);
        }
        // In PolicyNet::forward and ValueNet::forward:
        let expected_flatten_size = {
            let size = h.size();
            (size[1], size[2], size[3]) // Extract dimensions as a tuple
        };
        let flattened_size = expected_flatten_size.0 * expected_flatten_size.1 * expected_flatten_size.2;

        h = h.view([-1, flattened_size]);

        h = h.apply(&self.flatten).relu();
        if train { h = h.dropout(self.dropout_rate, train); }

        h = h.apply(&self.fc1).relu();
        if train { h = h.dropout(self.dropout_rate, train); }

        h.apply(&self.policy_head).softmax(-1, tch::Kind::Float)
    }

}

// Initialize weights
pub fn initialize_weights(vs: &nn::VarStore) {
    for (name, mut param) in vs.variables() {
        let size = param.size();

        if name.contains("policy_conv1") && size.len() >= 2 {
            // Xavier initialization for policy network's conv1
            let fan_in = size[1] as f64;
            let fan_out = size[0] as f64;
            let bound = (2.0 / fan_in).sqrt(); // He initialization
            let new_data = Tensor::randn_like(&param) * bound;
            param.set_data(&new_data);
            println!("Initialized policy_conv1 with Xavier initialization: {}", name);
        } else if name.contains("value_conv1") && size.len() >= 2 {
            // Xavier initialization for value network's conv1
            let fan_in = size[1] as f64;
            let fan_out = size[0] as f64;
            let bound = (2.0 / fan_in).sqrt(); // He initialization
            let new_data = Tensor::randn_like(&param) * bound;
            param.set_data(&new_data);
            println!("Initialized value_conv1 with Xavier initialization: {}", name);
        } else if size.len() >= 2 {
            // Default Xavier initialization for other weight tensors
            let fan_in = size[1] as f64;
            let fan_out = size[0] as f64;
            let bound = (2.0 / fan_in).sqrt(); // He initialization
            let new_data = Tensor::randn_like(&param) * bound;
            param.set_data(&new_data);
            println!("Initialized weight tensor: {}", name);
        } else if size.len() == 1 {
            // Zero initialization for biases
            let new_data = Tensor::zeros_like(&param);
            param.set_data(&new_data);
            println!("Initialized bias tensor: {}", name);
        } else {
            // Handle unexpected tensor shapes
            eprintln!("Unexpected tensor shape for parameter {}: {:?}", name, size);
        }
    }
}

//... other imports


pub struct ValueNet {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    res_blocks: Vec<ResNetBlock>,
    flatten: nn::Linear,
    fc1: nn::Linear, // Added FC layer
    value_head: nn::Linear,
    dropout_rate: f64,
}

impl ValueNet {
    pub fn new(vs: &VarStore, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root();
        let (channels, height, width) = input_dim;

        let conv1 = nn::conv2d(&p / "value_conv1", channels, INITIAL_CONV_CHANNELS_VALUE, 3, nn::ConvConfig { padding: 1,..Default::default() });
        let bn1 = nn::batch_norm2d(&p / "value_bn1", INITIAL_CONV_CHANNELS_VALUE, nn::BatchNormConfig::default());

        let mut res_blocks = Vec::new();
        let mut in_channels = INITIAL_CONV_CHANNELS_VALUE;

        for _ in 0..NUM_RES_BLOCKS_VALUE {
            let out_channels = 128; // Or adjust as needed
            res_blocks.push(ResNetBlock::new(&vs, in_channels, out_channels)); // Use &vs here!
            in_channels = out_channels;
        }

        let flatten_size = in_channels * height * width; // Adjust if you have downsampling
        let flatten = nn::linear(&p / "value_flatten", flatten_size, 1024, Default::default()); // Increased size
        let fc1 = nn::linear(&p / "value_fc1", 1024, 256, Default::default()); // Added FC layer
        let value_head = nn::linear(&p / "value_head", 256, 1, nn::LinearConfig::default());

        initialize_weights(&vs); // Use &vs here!

        Self {
            conv1,
            bn1,
            res_blocks,
            flatten,
            fc1,
            value_head,
            dropout_rate: 0.2,
        }
    }
    pub fn save_model(&self, vs: &nn::VarStore, path: &str) -> Result<()> {
        // Save the model's state dictionary to the specified path
        vs.save(path)?;
        Ok(())
    }

    pub fn load_model(&self, vs: &mut nn::VarStore, path: &str) -> Result<()> {
        // Load the model's state dictionary from the specified path
        vs.load(path)?;
        // Recreate the model with the loaded weights
        Ok(())
    }
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut h = x.apply(&self.conv1).apply_t(&self.bn1, train).leaky_relu();

        for block in &self.res_blocks {
            h = block.forward(&h, train);
        }

        // In PolicyNet::forward and ValueNet::forward:
        let expected_flatten_size = {
            let size = h.size();
            (size[1], size[2], size[3]) // Extract dimensions as a tuple
        };
        let flattened_size = expected_flatten_size.0 * expected_flatten_size.1 * expected_flatten_size.2;

        h = h.view([-1, flattened_size]);

        h = h.apply(&self.flatten).relu();
        if train { h = h.dropout(self.dropout_rate, train); }

        h = h.apply(&self.fc1).relu(); // FC layer
        if train { h = h.dropout(self.dropout_rate, train); }

        h.apply(&self.value_head).sigmoid()
    }
}


#[cfg(test)]
mod tests {
    use tch::{Device, nn};

    use super::*;

    #[test]
    fn test_policy_net_creation() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (3, 5, 5); // Example dimensions: channels, height, width
        let policy_net = PolicyNet::new(&vs, 2, input_dim);

        // Assert that the PolicyNet was created correctly
        assert_eq!(policy_net.res_blocks.len(), 2);
        assert_eq!(policy_net.dropout_rate, 0.2);
    }

    #[test]
    fn test_policy_net_forward() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (3, 5, 5);
        let policy_net = PolicyNet::new(&vs, 2, input_dim);

        // Create a dummy input tensor
        let input = Tensor::rand(&[1, 3, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = policy_net.forward(&input, true);

        // Assert that the output has the expected shape
        assert_eq!(output.size(), vec![1, 19]); // Assuming 19 is the number of actions
    }

    #[test]
    fn test_policy_net_save_and_load_weights() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (3, 5, 5);
        let mut policy_net = PolicyNet::new(&vs, 2, input_dim);

        // Path for saving weights
        let path = "./test_weights";

        // Save weights
        assert!(policy_net.save_weights(path).is_ok());

        // Load weights
        assert!(policy_net.load_weights(path).is_ok());
    }

    #[test]
    fn test_value_net_creation_and_forward() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (3, 5, 5);
        let value_net = ValueNet::new(&vs, 2, input_dim);

        // Create a dummy input tensor
        let input = Tensor::rand(&[1, 3, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = value_net.forward(&input, true);

        // Assert that the output has the expected shape
        assert_eq!(output.size(), vec![1, 1]); // Assuming 1 output value
    }
}
