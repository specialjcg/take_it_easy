use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use futures_util::StreamExt;
use tch::{nn, Tensor};
use tch::nn::{Module, ModuleT};
use crate::policy_value_net::res_net_block::ResNetBlock;
use std::io::Write; // Add this import
mod res_net_block;

// pub struct PolicyValueNet<'a> {
//     vs: &'a nn::VarStore,
//     conv1: nn::Conv2D,
//     conv1_bn: nn::BatchNorm,
//     conv2: nn::Conv2D,
//     conv2_bn: nn::BatchNorm,
//     conv3: nn::Conv2D,
//     conv3_bn: nn::BatchNorm,
//     fc1: nn::Linear,
//     fc1_bn: nn::BatchNorm,
//     policy_head: nn::Linear,
//     value_head: nn::Linear,
// }
//
// impl<'a> PolicyValueNet<'a> {
//     pub fn new_with_var_store(vs: &'a nn::VarStore) -> Self {
//         let p = vs.root();
//         let conv1 = nn::conv2d(&p / "conv1", 3, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });
//         let conv1_bn = nn::batch_norm2d(&p / "conv1_bn", 64, Default::default());
//         let conv2 = nn::conv2d(&p / "conv2", 64, 128, 3, nn::ConvConfig { padding: 1, ..Default::default() });
//         let conv2_bn = nn::batch_norm2d(&p / "conv2_bn", 128, Default::default());
//         let conv3 = nn::conv2d(&p / "conv3", 128, 256, 3, nn::ConvConfig { padding: 1, ..Default::default() });
//         let conv3_bn = nn::batch_norm2d(&p / "conv3_bn", 256, Default::default());
//         let fc1 = nn::linear(&p / "fc1", 256 * 5 * 5, 512, Default::default());
//         let fc1_bn = nn::batch_norm1d(&p / "fc1_bn", 512, Default::default());
//         let policy_head = nn::linear(&p / "policy_head", 512, 19, Default::default());
//         let value_head = nn::linear(&p / "value_head", 512, 1, Default::default());
//
//         PolicyValueNet {
//             vs,
//             conv1,
//             conv1_bn,
//             conv2,
//             conv2_bn,
//             conv3,
//             conv3_bn,
//             fc1,
//             fc1_bn,
//             policy_head,
//             value_head,
//         }
//     }
//
//     pub fn forward(&self, x: &Tensor) -> (Tensor, Tensor) {
//         let x = x.view([-1, 3, 5, 5]); // Reshape input to 5x5 grid with 3 channels
//         let mut h = self.conv1.forward(&x);
//         h = self.conv1_bn.forward_t(&h, true).relu(); // Use forward_t for BatchNorm
//         h = self.conv2.forward(&h);
//         h = self.conv2_bn.forward_t(&h, true).relu(); // Use forward_t for BatchNorm
//         h = self.conv3.forward(&h);
//         h = self.conv3_bn.forward_t(&h, true).relu(); // Use forward_t for BatchNorm
//         h = h.view([-1, 256 * 5 * 5]);
//         h = self.fc1.forward(&h).relu();
//
//         let policy = self.policy_head.forward(&h).softmax(-1, tch::Kind::Float);
//         let value = self.value_head.forward(&h).tanh();
//
//         (policy, value)
//     }
//
//
//     pub fn save_weights(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
//         self.conv1.ws.set_requires_grad(false);
//         self.conv2.ws.set_requires_grad(false);
//         self.conv3.ws.set_requires_grad(false);
//         self.fc1.ws.set_requires_grad(false);
//         self.policy_head.ws.set_requires_grad(false);
//         self.value_head.ws.set_requires_grad(false);
//
//         self.conv1.ws.save(&format!("{}/conv1.pt", path))?;
//         self.conv2.ws.save(&format!("{}/conv2.pt", path))?;
//         self.conv3.ws.save(&format!("{}/conv3.pt", path))?;
//         self.fc1.ws.save(&format!("{}/fc1.pt", path))?;
//         self.policy_head.ws.save(&format!("{}/policy_head.pt", path))?;
//         self.value_head.ws.save(&format!("{}/value_head.pt", path))?;
//
//         println!("Model weights saved successfully in: {}", path);
//         Ok(())
//     }
//
//     pub fn load_weights(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
//         let conv1_path = format!("{}/conv1.pt", path);
//         let conv2_path = format!("{}/conv2.pt", path);
//         let conv3_path = format!("{}/conv3.pt", path);
//         let fc1_path = format!("{}/fc1.pt", path);
//         let policy_path = format!("{}/policy_head.pt", path);
//         let value_path = format!("{}/value_head.pt", path);
//
//         if Path::new(&conv1_path).exists()
//             && Path::new(&conv2_path).exists()
//             && Path::new(&conv3_path).exists()
//             && Path::new(&fc1_path).exists()
//             && Path::new(&policy_path).exists()
//             && Path::new(&value_path).exists()
//         {
//             self.conv1.ws = Tensor::load(&conv1_path)?.detach().shallow_clone();
//             self.conv2.ws = Tensor::load(&conv2_path)?.detach().shallow_clone();
//             self.conv3.ws = Tensor::load(&conv3_path)?.detach().shallow_clone();
//             self.fc1.ws = Tensor::load(&fc1_path)?.detach().shallow_clone();
//             self.policy_head.ws = Tensor::load(&policy_path)?.detach().shallow_clone();
//             self.value_head.ws = Tensor::load(&value_path)?.detach().shallow_clone();
//
//             println!("Model weights loaded successfully from: {}", path);
//             Ok(())
//         } else {
//             Err(format!("One or more weight files are missing in: {}", path).into())
//         }
//     }
//
//
// }
pub struct PolicyValueNet<'a> {
    vs: &'a nn::VarStore,
    conv1: nn::Conv2D,
    res_blocks: Vec<ResNetBlock>, // Residual blocks
    flatten: nn::Linear,          // Flatten layer
    policy_head: nn::Linear,
    value_head: nn::Linear,
    dropout_rate: f64,            // Dropout rate

}

impl<'a> PolicyValueNet<'a> {
    pub fn new_with_var_store(vs: &'a nn::VarStore, num_res_blocks: usize, input_dim: (i64, i64, i64)) -> Self {
        let (channels, height, width) = input_dim;
        let p = vs.root();

        let conv1 = nn::conv2d(&p / "conv1", channels, 64, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let mut res_blocks = Vec::new();
        for i in 0..num_res_blocks {
            res_blocks.push(ResNetBlock::new(&(&p / format!("res_block_{}", i)), 64, 64));
        }

        let flatten_size = 64 * height * width; // Match dimensions based on expected input
        let flatten = nn::linear(&p / "flatten", flatten_size, 512, Default::default());
        let policy_head = nn::linear(&p / "policy_head", 512, 19, Default::default());
        let value_head = nn::linear(&p / "value_head", 512, 1, Default::default());
        let dropout_rate = 0.2;
        Self {
            vs,
            conv1,
            res_blocks,
            flatten,
            policy_head,
            value_head,
            dropout_rate,

        }
    }
    fn apply_dropout(&self, x: &Tensor, train: bool) -> Tensor {
        if train {
            // Generate a random mask and apply element-wise greater-than comparison
            let mask = Tensor::rand(&x.size(), (tch::Kind::Float, x.device())).gt(self.dropout_rate);
            // Scale the output by the dropout probability
            x * mask.to_kind(tch::Kind::Float) / (1.0 - self.dropout_rate)
        } else {
            x.shallow_clone()
        }
    }
    pub fn parameters(&self) -> Vec<Tensor> { // Ensure this is public
        let mut params = Vec::new();
        params.push(self.conv1.ws.shallow_clone());
        params.push(self.flatten.ws.shallow_clone());
        params.push(self.policy_head.ws.shallow_clone());
        params.push(self.value_head.ws.shallow_clone());
        for block in &self.res_blocks {
            params.push(block.conv1.ws.shallow_clone());
            params.push(block.conv2.ws.shallow_clone());
        }
        params
    }

    pub fn forward(&self, x: &Tensor, train: bool) -> (Tensor, Tensor) {
        let mut h = x.apply(&self.conv1).relu();

        for (i, res_block) in self.res_blocks.iter().enumerate() {
            h = res_block.forward(&h, train);
            h = self.apply_dropout(&h, train); // Apply dropout after each residual block

        }

        // Dynamically calculate flatten size
        let flatten_size = h.size()[1] * h.size()[2] * h.size()[3]; // channels * height * width
        h = h.view([-1, flatten_size]);

        h = h.apply(&self.flatten).relu();
        h = self.apply_dropout(&h, train); // Apply dropout after each residual block

        let policy = h.apply(&self.policy_head).softmax(-1, tch::Kind::Float);
        let value = h.apply(&self.value_head).tanh();

        (policy, value)
    }


    pub fn save_weights(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure the directory exists
        fs::create_dir_all(path)?;

        self.conv1.ws.set_requires_grad(false);
        for (i, block) in self.res_blocks.iter().enumerate() {
            block.conv1.ws.set_requires_grad(false);
            block.conv2.ws.set_requires_grad(false);
        }
        self.flatten.ws.set_requires_grad(false);
        self.policy_head.ws.set_requires_grad(false);
        self.value_head.ws.set_requires_grad(false);

        // Save model weights
        self.conv1.ws.save(&format!("{}/conv1.pt", path))?;
        for (i, block) in self.res_blocks.iter().enumerate() {
            block.conv1.ws.save(&format!("{}/res_block_{}_conv1.pt", path, i))?;
            block.conv2.ws.save(&format!("{}/res_block_{}_conv2.pt", path, i))?;
        }
        self.flatten.ws.save(&format!("{}/flatten.pt", path))?;
        self.policy_head.ws.save(&format!("{}/policy_head.pt", path))?;
        self.value_head.ws.save(&format!("{}/value_head.pt", path))?;

        // Save dropout rate
        let dropout_file_path = format!("{}/dropout_rate.txt", path);
        let mut file = File::create(&dropout_file_path)?;
        writeln!(file, "{}", self.dropout_rate)?;

        println!("Model weights and dropout rate saved successfully in: {}", path);
        Ok(())
    }

    pub fn load_weights(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conv1_path = format!("{}/conv1.pt", path);
        let flatten_path = format!("{}/flatten.pt", path);
        let policy_path = format!("{}/policy_head.pt", path);
        let value_path = format!("{}/value_head.pt", path);

        if Path::new(&conv1_path).exists()
            && Path::new(&flatten_path).exists()
            && Path::new(&policy_path).exists()
            && Path::new(&value_path).exists()
        {
            self.conv1.ws = Tensor::load(&conv1_path)?.detach().shallow_clone();
            self.flatten.ws = Tensor::load(&flatten_path)?.detach().shallow_clone();
            self.policy_head.ws = Tensor::load(&policy_path)?.detach().shallow_clone();
            self.value_head.ws = Tensor::load(&value_path)?.detach().shallow_clone();

            for (i, block) in self.res_blocks.iter_mut().enumerate() {
                let conv1_path = format!("{}/res_block_{}_conv1.pt", path, i);
                let conv2_path = format!("{}/res_block_{}_conv2.pt", path, i);
                if Path::new(&conv1_path).exists() && Path::new(&conv2_path).exists() {
                    block.conv1.ws = Tensor::load(&conv1_path)?.detach().shallow_clone();
                    block.conv2.ws = Tensor::load(&conv2_path)?.detach().shallow_clone();
                } else {
                    return Err(format!("Weights for ResNet block {} are missing in: {}", i, path).into());
                }
            }

            // Load dropout rate
            let dropout_file_path = format!("{}/dropout_rate.txt", path);
            if Path::new(&dropout_file_path).exists() {
                let mut file = File::open(&dropout_file_path)?;
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                self.dropout_rate = content.trim().parse::<f64>()?;
            } else {
                return Err(format!("Dropout rate file is missing in: {}", path).into());
            }

            println!("Model weights and dropout rate loaded successfully from: {}", path);
            Ok(())
        } else {
            Err(format!("One or more weight files are missing in: {}", path).into())
        }
    }
}


