use std::ops::{Mul, Sub};

use futures_util::TryFutureExt;
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
    policy_head: nn::Linear,
    dropout_rate: f64,
}

impl<'a> PolicyNet {
    pub fn new(vs: &'a nn::VarStore, num_res_blocks: usize, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root();
        let (channels, height, width) = input_dim;

        let conv1 = nn::conv2d(&p / "policy_conv1", channels, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        conv1.ws.set_requires_grad(true); // Ensure requires_grad is true
        if let Some(bias) = &conv1.bs {
            bias.set_requires_grad(true); // Ensure bias is trainable
        }

        let bn1 = nn::batch_norm2d(&p / "policy_bn1", 64, nn::BatchNormConfig::default());
        bn1.ws.as_ref().map(|w| w.set_requires_grad(true));
        bn1.bs.as_ref().map(|b| b.set_requires_grad(true));

        let mut res_blocks = Vec::new();
        for _ in 0..num_res_blocks {
            res_blocks.push(ResNetBlock::new(vs, 64, 64));
        }

        let flatten_size = 64 * height * width;
        let flatten = nn::linear(&p / "policy_flatten", flatten_size, 512, Default::default());
        flatten.ws.set_requires_grad(true);
        flatten.bs.as_ref().map(|b| b.set_requires_grad(true));

        let policy_head = nn::linear(&p / "policy_head", 512, 19, nn::LinearConfig::default());
        policy_head.ws.set_requires_grad(true);
        policy_head.bs.as_ref().map(|b| b.set_requires_grad(true));

        // Initialize weights
        initialize_weights(vs);

        Self {
            conv1,
            bn1,
            res_blocks,
            flatten,
            policy_head,
            dropout_rate: 0.2,
        }
    }

    pub fn save_model(&self, vs: &nn::VarStore, path: &str) -> Result<()> {
        // Save the model's state dictionary to the specified path
        vs.save(path)?;
        Ok(())
    }

    pub fn load_model(&self,  vs: &mut nn::VarStore, path: &str, num_res_blocks: usize, input_dim: (i64, i64, i64)) -> Result<Self> {
        // Load the model's state dictionary from the specified path
        vs.load(path)?;
        // Recreate the model with the loaded weights
        Ok(Self::new(&vs, num_res_blocks, input_dim))
    }
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut h = x.apply(&self.conv1).apply_t(&self.bn1, train).relu();

        for res_block in &self.res_blocks {
            h = res_block.forward(&h, train);
        }

        h = h.view([-1, 64 * 5 * 5]);
        h = h.apply(&self.flatten).relu();

        if train {
            h = h.dropout(self.dropout_rate, train);
        }

        let policy_logits = h.apply(&self.policy_head);
        policy_logits.softmax(-1, tch::Kind::Float)
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
            let bound = (6.0 / (fan_in + fan_out)).sqrt();
            let new_data = Tensor::randn_like(&param) * bound;
            param.set_data(&new_data);
            println!("Initialized policy_conv1 with Xavier initialization: {}", name);
        } else if name.contains("value_conv1") && size.len() >= 2 {
            // Xavier initialization for value network's conv1
            let fan_in = size[1] as f64;
            let fan_out = size[0] as f64;
            let bound = (6.0 / (fan_in + fan_out)).sqrt();
            let new_data = Tensor::randn_like(&param) * bound;
            param.set_data(&new_data);
            println!("Initialized value_conv1 with Xavier initialization: {}", name);
        } else if size.len() >= 2 {
            // Default Xavier initialization for other weight tensors
            let fan_in = size[1] as f64;
            let fan_out = size[0] as f64;
            let bound = (6.0 / (fan_in + fan_out)).sqrt();
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



pub struct ValueNet {
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    res_blocks: Vec<ResNetBlock>,
    flatten: nn::Linear,
    value_head: nn::Linear,
    dropout_rate: f64,
}

impl<'a> ValueNet {
    pub fn new(vs: &'a mut nn::VarStore, num_res_blocks: usize, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root();
        let (channels, height, width) = input_dim;

        // Define conv1 layer
        let mut conv1 = nn::conv2d(&p / "value_conv1", channels, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });

        let weight = &mut conv1.ws;
        let fan_in = weight.size()[0] as f64; // Input channels
        let bound = (2.0 / fan_in).sqrt(); // He initialization
        weight.set_data(&(Tensor::randn_like(weight) * bound));

        if let Some(ref mut bias) = conv1.bs {
            bias.set_data(&Tensor::zeros_like(bias)); // Zero-initialize bias
        }
        // Define batch norm layer
        let mut bn1 = nn::batch_norm2d(&p / "value_bn1", 64, nn::BatchNormConfig::default());
        // Define residual blocks
        let mut res_blocks = Vec::new();
        for _ in 0..num_res_blocks {
            res_blocks.push(ResNetBlock::new(vs, 64, 64));
        }

        // Define fully connected layers
        let flatten_size = 64 * height * width;
        let flatten = nn::linear(&p / "value_flatten", flatten_size, 512, Default::default());


        if let Some(weight) = &mut bn1.ws {
            let fan_in = weight.size()[0] as f64;
            let bound = (2.0 / fan_in).sqrt(); // He initialization
            weight.set_data(&(Tensor::randn_like(weight) * bound));
        }

        if let Some(bias) = &mut bn1.bs {
            bias.set_data(&Tensor::zeros_like(bias)); // Zero-initialize bias
        }


        let mut value_head = nn::linear(&p / "value_head", 512, 1, nn::LinearConfig::default());

        let weight = &mut value_head.ws; // Access the weight tensor mutably
        let fan_in = weight.size()[0] as f64;
        let bound = (2.0 / fan_in).sqrt(); // He initialization
        weight.set_data(&(Tensor::randn_like(weight) * bound));

        if let Some(bias) = &mut value_head.bs {
            bias.set_data(&Tensor::zeros_like(bias)); // Zero-initialize bias
        }



        // General initialization for other layers

        Self {
            conv1,
            bn1,
            res_blocks,
            flatten,
            value_head,
            dropout_rate: 0.2,
        }
    }

    pub fn save_model(&self, vs: &nn::VarStore, path: &str) -> Result<()> {
        // Save the model's state dictionary to the specified path
        vs.save(path)?;
        Ok(())
    }

    pub fn load_model(&self,
        vs: &mut nn::VarStore,
        path: &str,
        num_res_blocks: usize,
        input_dim: (i64, i64, i64),
    ) -> Result<Self> {
        vs.load(path)?; // Load weights into the existing VarStore
        Ok(Self::new(vs, num_res_blocks, input_dim))
    }





    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {

        let mut h = x.apply(&self.conv1).apply_t(&self.bn1, train);

        h = h.clamp(-10.0, 10.0).relu();

        for res_block in &self.res_blocks {
            h = res_block.forward(&h, train);
        }

        h = h.view([-1, 64 * 5 * 5]);
        h = h.apply(&self.flatten).relu();

        if train {
            h = h.dropout(self.dropout_rate, train);
        }

        let value = h.apply(&self.value_head).tanh();
        value
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
