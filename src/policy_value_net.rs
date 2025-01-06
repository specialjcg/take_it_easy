use std::fs;
use std::ops::{Mul, Sub};
use futures_util::TryFutureExt;
use tch::{Kind, nn, Tensor};
use tch::nn::init::FanInOut;
use tch::nn::init::FanInOut::FanIn;
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









// fn initialize_weights(vs: &nn::VarStore) {
//     for (name, mut tensor) in vs.variables() {
//         *tensor = tensor.detach().clone(&Default::default());
//         println!("Initializing tensor: {}", name);
//
//         if tensor.requires_grad() {
//             let new_tensor = if name.contains("bn") {
//                 if name.ends_with("weight") {
//                     println!("Initializing BatchNorm weight: {}", name);
//                     Tensor::ones_like(&tensor).detach() // Detach from computation graph
//                 } else if name.ends_with("bias") {
//                     println!("Initializing BatchNorm bias: {}", name);
//                     Tensor::zeros_like(&tensor).detach()
//                 } else {
//                     continue;
//                 }
//             } else if tensor.size().len() >= 2 {
//                 println!("Initializing weight with Kaiming Uniform: {}", name);
//                 let fan_in = tensor.size()[1];
//                 let gain = (6.0 / fan_in as f64).sqrt();
//                 Tensor::rand_like(&tensor).mul(gain).sub(gain / 2.0).detach()
//             } else {
//                 println!("Initializing bias: {}", name);
//                 Tensor::zeros_like(&tensor).detach()
//             };
//
//             // Use f_copy_ to replace the tensor's data
//             tensor.f_copy_(&new_tensor).unwrap_or_else(|err| {
//                 panic!("Failed to initialize tensor {}: {:?}", name, err)
//             });
//         } else {
//             println!("Skipping tensor without gradients: {}", name);
//         }
//     }
// }

























pub struct ValueNet<'a> {
    vs: &'a nn::VarStore,
    conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    res_blocks: Vec<ResNetBlock>,
    flatten: nn::Linear,
    value_head: nn::Linear,
    dropout_rate: f64,
}

impl<'a> ValueNet<'a> {
    pub fn new(vs: &'a nn::VarStore, num_res_blocks: usize, input_dim: (i64, i64, i64)) -> Self {
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
            vs,
            conv1,
            bn1,
            res_blocks,
            flatten,
            value_head,
            dropout_rate: 0.2,
        }
    }


    pub fn load_weights(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Load initial convolutional layer weights
        self.conv1.ws = Tensor::load(&format!("{}/value_conv1.pt", path))?;
        self.conv1.bs = Some(Tensor::load(&format!("{}/value_conv1_bias.pt", path))?);
        self.bn1.running_mean = Tensor::load(&format!("{}/value_bn1_mean.pt", path))?;
        self.bn1.running_var = Tensor::load(&format!("{}/value_bn1_var.pt", path))?;
        self.bn1.ws = Some(Tensor::load(&format!("{}/value_bn1_weights.pt", path))?);
        self.bn1.bs = Some(Tensor::load(&format!("{}/value_bn1_biases.pt", path))?);

        // Load weights for ResNet blocks
        for (i, block) in self.res_blocks.iter_mut().enumerate() {
            block.conv1.ws = Tensor::load(&format!("{}/value_res_block_{}_conv1.pt", path, i))?;
            block.conv2.ws = Tensor::load(&format!("{}/value_res_block_{}_conv2.pt", path, i))?;
            block.bn1.running_mean = Tensor::load(&format!("{}/value_res_block_{}_bn1_mean.pt", path, i))?;
            block.bn1.running_var = Tensor::load(&format!("{}/value_res_block_{}_bn1_var.pt", path, i))?;
            block.bn1.ws = Some(Tensor::load(&format!("{}/value_res_block_{}_bn1_weights.pt", path, i))?);
            block.bn1.bs = Some(Tensor::load(&format!("{}/value_res_block_{}_bn1_biases.pt", path, i))?);
            block.bn2.running_mean = Tensor::load(&format!("{}/value_res_block_{}_bn2_mean.pt", path, i))?;
            block.bn2.running_var = Tensor::load(&format!("{}/value_res_block_{}_bn2_var.pt", path, i))?;
            block.bn2.ws = Some(Tensor::load(&format!("{}/value_res_block_{}_bn2_weights.pt", path, i))?);
            block.bn2.bs = Some(Tensor::load(&format!("{}/value_res_block_{}_bn2_biases.pt", path, i))?);


        }

        // Load weights for fully connected layers
        self.flatten.ws = Tensor::load(&format!("{}/value_flatten.pt", path))?;
        self.value_head.ws = Tensor::load(&format!("{}/value_head.pt", path))?;

        Ok(())
    }

    pub fn save_weights(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(path)?;

        // Save initial convolutional layer weights
        self.conv1.ws.save(&format!("{}/value_conv1.pt", path))?;
        if let Some(bs) = &self.conv1.bs {
            bs.save(&format!("{}/value_conv1_bias.pt", path))?;
        }
        self.bn1.running_mean.save(&format!("{}/value_bn1_mean.pt", path))?;
        self.bn1.running_var.save(&format!("{}/value_bn1_var.pt", path))?;
        if let Some(ws) = &self.bn1.ws {
            ws.save(&format!("{}/value_bn1_weights.pt", path))?;
        }
        if let Some(bs) = &self.bn1.bs {
            bs.save(&format!("{}/value_bn1_biases.pt", path))?;
        }

        // Save weights for ResNet blocks
        for (i, block) in self.res_blocks.iter().enumerate() {
            block.conv1.ws.save(&format!("{}/value_res_block_{}_conv1.pt", path, i))?;
            block.conv2.ws.save(&format!("{}/value_res_block_{}_conv2.pt", path, i))?;
            block.bn1.running_mean.save(&format!("{}/value_res_block_{}_bn1_mean.pt", path, i))?;
            block.bn1.running_var.save(&format!("{}/value_res_block_{}_bn1_var.pt", path, i))?;
            if let Some(ws) = &block.bn1.ws {
                ws.save(&format!("{}/value_res_block_{}_bn1_weights.pt", path, i))?;
            }
            if let Some(bs) = &block.bn1.bs {
                bs.save(&format!("{}/value_res_block_{}_bn1_biases.pt", path, i))?;
            }
            block.bn2.running_mean.save(&format!("{}/value_res_block_{}_bn2_mean.pt", path, i))?;
            block.bn2.running_var.save(&format!("{}/value_res_block_{}_bn2_var.pt", path, i))?;
            if let Some(ws) = &block.bn2.ws {
                ws.save(&format!("{}/value_res_block_{}_bn2_weights.pt", path, i))?;
            }
            if let Some(bs) = &block.bn2.bs {
                bs.save(&format!("{}/value_res_block_{}_bn2_biases.pt", path, i))?;
            }


        }

        // Save weights for fully connected layers
        self.flatten.ws.save(&format!("{}/value_flatten.pt", path))?;
        self.value_head.ws.save(&format!("{}/value_head.pt", path))?;

        Ok(())
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
