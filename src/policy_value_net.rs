use std::fs;
use futures_util::StreamExt;
use tch::{nn, Tensor};
use crate::policy_value_net::res_net_block::ResNetBlock;

mod res_net_block;
pub struct PolicyNet<'a> {
    vs: &'a nn::VarStore,
    conv1: nn::Conv2D,
    res_blocks: Vec<ResNetBlock>, // Residual blocks for shared features
    flatten: nn::Linear,          // Shared flattening layer
    policy_head: nn::Linear,      // Policy-specific fully connected layer
    dropout_rate: f64,            // Dropout rate for regularization
}

impl<'a> PolicyNet<'a> {
    pub fn new(vs: &'a nn::VarStore, num_res_blocks: usize, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root();
        let (channels, height, width) = input_dim;

        let conv1 = nn::conv2d(&p / "conv1", channels, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let mut res_blocks = Vec::new();
        for i in 0..num_res_blocks {
            res_blocks.push(ResNetBlock::new(&(&p / format!("res_block_{}", i)), 64, 64));
        }

        let flatten_size = 64 * height * width;
        let flatten = nn::linear(&p / "flatten", flatten_size, 512, Default::default());
        let policy_head = nn::linear(&p / "policy_head", 512, 19, Default::default());
        let dropout_rate = 0.2;

        Self {
            vs,
            conv1,
            res_blocks,
            flatten,
            policy_head,
            dropout_rate,
        }
    }
    pub fn parameters(&self) -> Vec<Tensor> {
        let mut params = Vec::new();
        params.push(self.conv1.ws.shallow_clone());
        params.push(self.flatten.ws.shallow_clone());
        params.push(self.policy_head.ws.shallow_clone());
        for block in &self.res_blocks {
            params.push(block.conv1.ws.shallow_clone());
            params.push(block.conv2.ws.shallow_clone());
        }
        params
    }
    pub fn load_weights(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.conv1.ws = Tensor::load(&format!("{}/policy_conv1.pt", path))?;
        for (i, block) in self.res_blocks.iter_mut().enumerate() {
            let conv1_path = format!("{}/policy_res_block_{}_conv1.pt", path, i);
            let conv2_path = format!("{}/policy_res_block_{}_conv2.pt", path, i);


            block.conv1.ws = Tensor::load(&conv1_path)?;
            block.conv2.ws = Tensor::load(&conv2_path)?;
        }


        self.flatten.ws = Tensor::load(&format!("{}/policy_flatten.pt", path))?;
        self.policy_head.ws = Tensor::load(&format!("{}/policy_head.pt", path))?;

        Ok(())
    }
    pub fn save_weights(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(path)?;

        self.conv1.ws.save(&format!("{}/policy_conv1.pt", path))?;
        for (i, block) in self.res_blocks.iter().enumerate() {
            block.conv1.ws.save(&format!("{}/policy_res_block_{}_conv1.pt", path, i))?;
            block.conv2.ws.save(&format!("{}/policy_res_block_{}_conv2.pt", path, i))?;
        }
        self.flatten.ws.save(&format!("{}/policy_flatten.pt", path))?;
        self.policy_head.ws.save(&format!("{}/policy_head.pt", path))?;

        Ok(())
    }

    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut h = x.apply(&self.conv1).relu();

        for res_block in &self.res_blocks {
            h = res_block.forward(&h, train);
        }

        h = h.view([-1, 64 * 5 * 5]);
        h = h.apply(&self.flatten).relu();

        let policy_logits = h.apply(&self.policy_head);
        policy_logits.softmax(-1, tch::Kind::Float)
    }
}

pub struct ValueNet<'a> {
    vs: &'a nn::VarStore,
    conv1: nn::Conv2D,
    res_blocks: Vec<ResNetBlock>,
    flatten: nn::Linear,
    value_head: nn::Linear,
}

impl<'a> ValueNet<'a> {
    pub fn new(vs: &'a nn::VarStore, num_res_blocks: usize, input_dim: (i64, i64, i64)) -> Self {
        let p = vs.root();
        let (channels, height, width) = input_dim;

        let conv1 = nn::conv2d(&p / "conv1", channels, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let mut res_blocks = Vec::new();
        for i in 0..num_res_blocks {
            res_blocks.push(ResNetBlock::new(&(&p / format!("res_block_{}", i)), 64, 64));
        }

        let flatten_size = 64 * height * width;
        let flatten = nn::linear(&p / "flatten", flatten_size, 512, Default::default());
        let value_head = nn::linear(&p / "value_head", 512, 1, Default::default());

        Self {
            vs,
            conv1,
            res_blocks,
            flatten,
            value_head,
        }
    }
    pub fn load_weights(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.conv1.ws = Tensor::load(&format!("{}/value_conv1.pt", path))?;
        for (i, block) in self.res_blocks.iter_mut().enumerate() {
            block.conv1.ws = Tensor::load(&format!("{}/value_res_block_{}_conv1.pt", path,i))?;
            block.conv2.ws = Tensor::load(&format!("{}/value_res_block_{}_conv2.pt", path,i))?;
        }
        self.flatten.ws = Tensor::load(&format!("{}/value_flatten.pt", path))?;
        self.value_head.ws = Tensor::load(&format!("{}/value_head.pt", path))?;

        Ok(())
    }
    pub fn save_weights(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(path)?;

        self.conv1.ws.save(&format!("{}/value_conv1.pt", path))?;
        for (i, block) in self.res_blocks.iter().enumerate() {
            block.conv1.ws.save(&format!("{}/value_res_block_{}_conv1.pt", path, i))?;
            block.conv2.ws.save(&format!("{}/value_res_block_{}_conv2.pt", path, i))?;
        }
        self.flatten.ws.save(&format!("{}/value_flatten.pt", path))?;
        self.value_head.ws.save(&format!("{}/value_head.pt", path))?;

        Ok(())
    }
    pub fn parameters(&self) -> Vec<Tensor> {
        let mut params = Vec::new();
        params.push(self.conv1.ws.shallow_clone());
        params.push(self.flatten.ws.shallow_clone());
        params.push(self.value_head.ws.shallow_clone());
        for block in &self.res_blocks {
            params.push(block.conv1.ws.shallow_clone());
            params.push(block.conv2.ws.shallow_clone());
        }
        params
    }
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let mut h = x.apply(&self.conv1).relu();

        for res_block in &self.res_blocks {
            h = res_block.forward(&h, train);
        }

        h = h.view([-1, 64 * 5 * 5]);
        h = h.apply(&self.flatten).relu();

        h.apply(&self.value_head).tanh()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tch::{Device, nn};

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
