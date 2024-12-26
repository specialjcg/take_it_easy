use std::fs;
use std::path::Path;
use tch::{nn, Tensor};
use tch::nn::Module;

pub struct PolicyValueNet<'a> {
    vs: &'a nn::VarStore,
    input_layer: nn::Linear,
    policy_head: nn::Linear,
    value_head: nn::Linear,
}


impl<'a> PolicyValueNet<'a> {    pub fn new_with_var_store(
        input_size: i64,
        hidden_size: i64,
        vs: &'a nn::VarStore,
    ) -> Self {
        let input_layer = nn::linear(&vs.root(), input_size, hidden_size, Default::default());
        let policy_head = nn::linear(&vs.root(), hidden_size, 19, Default::default());
        let value_head = nn::linear(&vs.root(), hidden_size, 1, Default::default());

        PolicyValueNet {
            vs: vs.clone(),
            input_layer,
            policy_head,
            value_head,
        }
    }

    pub fn save_weights(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.input_layer.ws.set_requires_grad(false);
        self.policy_head.ws.set_requires_grad(false);
        self.value_head.ws.set_requires_grad(false);

        self.input_layer.ws.save(&format!("{}/input_layer.pt", path))?;
        self.policy_head.ws.save(&format!("{}/policy_head.pt", path))?;
        self.value_head.ws.save(&format!("{}/value_head.pt", path))?;
        println!("Model weights saved successfully in: {}", path);
        Ok(())
    }


    pub fn load_weights(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let input_path = format!("{}/input_layer.pt", path);
        let policy_path = format!("{}/policy_head.pt", path);
        let value_path = format!("{}/value_head.pt", path);

        if Path::new(&input_path).exists()
            && Path::new(&policy_path).exists()
            && Path::new(&value_path).exists()
        {
            self.input_layer.ws.set_requires_grad(false);
            self.policy_head.ws.set_requires_grad(false);
            self.value_head.ws.set_requires_grad(false);

            self.input_layer
                .ws
                .copy_(&Tensor::load(&input_path)?.detach());
            self.policy_head
                .ws
                .copy_(&Tensor::load(&policy_path)?.detach());
            self.value_head
                .ws
                .copy_(&Tensor::load(&value_path)?.detach());

            println!("Model weights loaded successfully from: {}", path);
            Ok(())
        } else {
            Err(format!("One or more weight files are missing in: {}", path).into())
        }
    }




    pub fn forward(&self, x: &Tensor) -> (Tensor, Tensor) {
        let hidden = x.apply(&self.input_layer).relu();
        let policy = self.policy_head.forward(&hidden).softmax(-1, tch::Kind::Float);
        let value = self.value_head.forward(&hidden).tanh();
        (policy, value)
    }
}