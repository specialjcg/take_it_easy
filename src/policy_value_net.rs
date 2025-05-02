
use tch::{nn, Tensor};
use tch::nn::VarStore;
use tch::Result;

use crate::policy_value_net::res_net_block::ResNetBlock;

mod res_net_block;

pub struct PolicyNet {
    conv1: nn::Conv2D,
    gn1: nn::GroupNorm,
    res_blocks: Vec<ResNetBlock>,
    flatten: nn::Linear,
    fc1: nn::Linear,
    policy_head: nn::Linear,
    dropout_rate: f64,
}
const NUM_RES_BLOCKS: usize = 4; // Or any number you want
const INITIAL_CONV_CHANNELS: i64 = 128;

const NUM_RES_BLOCKS_VALUE: usize = 4; // Or adjust as needed
const INITIAL_CONV_CHANNELS_VALUE: i64 = 128;
impl<'a> PolicyNet {
    // policy_value_net.rs (PolicyNet and ValueNet)
    pub fn new(vs: &nn::VarStore, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root(); // p is a Path
        let (channels, height, width) = input_dim;

        let conv1 = nn::conv2d(&p / "policy_conv1", channels, INITIAL_CONV_CHANNELS, 3, nn::ConvConfig { padding: 1,..Default::default() });
        let gn1 = nn::group_norm(&p / "gn1", 16, 128, Default::default());

        let mut res_blocks = Vec::new();
        let mut in_channels = INITIAL_CONV_CHANNELS;

        for _ in 0..NUM_RES_BLOCKS {
            let out_channels = 32; // Or adjust as needed (e.g., increase in stages)
            res_blocks.push(ResNetBlock::new(&vs, in_channels, out_channels)); // Use &vs here!
            in_channels = out_channels;
        }


        let flatten_size = in_channels * height * width; // Adjust if you have downsampling in ResNetBlocks
        let flatten = nn::linear(&p / "policy_flatten", flatten_size, 2048, Default::default());
        let fc1 = nn::linear(&p / "policy_fc1", 2048, 512, Default::default());
        let policy_head = nn::linear(&p / "policy_head", 512, 19, nn::LinearConfig::default());

        initialize_weights(&vs); // Use &vs here!

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
        let mut h = x.apply(&self.conv1).apply_t(&self.gn1, train).leaky_relu();

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
        if name.contains("conv1") && size.len() == 4 {
            let fan_in = (size[1] * size[2] * size[3]) as f64;
            kaiming_uniform(&mut param, fan_in);
            // log::info!("Initialized {} with Kaiming uniform", name);
        } else if size.len() == 2 {
            let fan_in = size[1] as f64;
            let bound = (1.0 / fan_in).sqrt();
            tch::no_grad(|| {
                param.f_uniform_(-bound, bound).unwrap();
            });
            // log::info!("Initialized Linear {} with Xavier uniform", name);
        } else if size.len() == 1 {
            tch::no_grad(|| {
                param.f_zero_().unwrap();
            });
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

        let bn1 = nn::batch_norm2d(&p / "value_bn1", INITIAL_CONV_CHANNELS_VALUE, nn::BatchNormConfig { affine: true, ..Default::default() });

        let mut res_blocks = Vec::new();
        let mut in_channels = INITIAL_CONV_CHANNELS_VALUE;

        for _ in 0..NUM_RES_BLOCKS_VALUE {
            let out_channels = 128; // Or adjust as needed
            res_blocks.push(ResNetBlock::new(&vs, in_channels, out_channels)); // Use &vs here!
            in_channels = out_channels;
        }

        let flatten_size = in_channels * height * width; // Adjust if you have downsampling
        let flatten = nn::linear(&p / "value_flatten", flatten_size, 2048, Default::default()); // Increased size
        let fc1 = nn::linear(&p / "value_fc1", 2048, 512, Default::default()); // Added FC layer
        let value_head = nn::linear(&p / "value_head", 512, 1, nn::LinearConfig::default());

        initialize_weights(&vs); // Use &vs here!

        Self {
            conv1,
            bn1,
            res_blocks,
            flatten,
            fc1,
            value_head,
            dropout_rate: 0.0,
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
        // 🔍 Log des statistiques des entrées
        let input_mean = x.mean(tch::Kind::Float).double_value(&[]);
        let input_std = x.std(false).double_value(&[]);
        // log::debug!("[ValueNet Input] Mean: {:.4}, Std: {:.4}", input_mean, input_std);

        // 📏 Recentrer les entrées
        let x = (x - x.mean(tch::Kind::Float)) / (x.std(false).clamp_min(1e-6));

        // 🚫 Test avec LeakyReLU(alpha=0.2)
        let mut h = x.apply(&self.conv1);
        let conv1_mean = h.mean(tch::Kind::Float).double_value(&[]);
        let conv1_std = h.std(false).double_value(&[]);
        // log::debug!("[ValueNet Conv1 Output Before Activation] Mean: {:.4}, Std: {:.4}", conv1_mean, conv1_std);

        // Pour un leaky_relu(0.2)
        // Leaky ReLU custom avec pente 0.2
        h = h.maximum(&(h.shallow_clone() * 0.2));

        let post_relu_mean = h.mean(tch::Kind::Float).double_value(&[]);
        // log::debug!("[ValueNet Conv1 Output After LeakyReLU] Mean: {:.4}", post_relu_mean);

        for block in &self.res_blocks {
            h = block.forward(&h, train);
        }

        let flattened_size = {
            let size = h.size();
            size[1] * size[2] * size[3]
        };

        h = h.view([-1, flattened_size])
            .apply(&self.flatten).relu();
        if train { h = h.dropout(self.dropout_rate, train); }

        h = h.apply(&self.fc1).relu();
        if train { h = h.dropout(self.dropout_rate, train); }

        h.apply(&self.value_head)

    }



}
fn kaiming_uniform(tensor: &mut Tensor, fan_in: f64) {
    let bound = (6.0f64).sqrt() / fan_in.sqrt();
    tch::no_grad(|| {
        tensor.f_uniform_(-bound, bound).unwrap();
    });
}



#[cfg(test)]
mod tests {
    use tch::{Device, nn};

    use super::*;

    #[test]
    fn test_policy_net_creation() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (3, 5, 5); // Example dimensions: channels, height, width
        let policy_net = PolicyNet::new(&vs,  input_dim);

        // Assert that the PolicyNet was created correctly
        assert_eq!(policy_net.res_blocks.len(), 4);
        assert_eq!(policy_net.dropout_rate, 0.3);
    }

    #[test]
    fn test_policy_net_forward() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (3, 5, 5);
        let policy_net = PolicyNet::new(&vs,  input_dim);

        // Create a dummy input tensor
        let input = Tensor::rand(&[1, 3, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = policy_net.forward(&input, true);

        // Assert that the output has the expected shape
        assert_eq!(output.size(), vec![1, 19]); // Assuming 19 is the number of actions
    }


    #[test]
    fn test_value_net_creation_and_forward() {
        let vs = nn::VarStore::new(Device::Cpu);
        let input_dim = (3, 5, 5);
        let value_net = ValueNet::new(&vs,  input_dim);

        // Create a dummy input tensor
        let input = Tensor::rand(&[1, 3, 5, 5], (tch::Kind::Float, Device::Cpu));
        let output = value_net.forward(&input, true);

        // Assert that the output has the expected shape
        assert_eq!(output.size(), vec![1, 1]); // Assuming 1 output value
    }
}
