use tch::{nn, Tensor, nn::Module, nn::OptimizerConfig};

pub struct PolicyValueNet {
    pub(crate) vs: nn::VarStore,
    input_layer: nn::Linear,    // New input layer
    policy_head: nn::Linear,    // Policy output layer
    value_head: nn::Linear,     // Value output layer
}

impl PolicyValueNet {
    pub fn new(input_size: i64, hidden_size: i64) -> Self {
        let vs = nn::VarStore::new(tch::Device::Cpu);

        // Add a new input transformation layer to match hidden_size
        let input_layer = nn::linear(&vs.root(), input_size, hidden_size, Default::default());
        let policy_head = nn::linear(&vs.root(), hidden_size, 19, Default::default());
        let value_head = nn::linear(&vs.root(), hidden_size, 1, Default::default());

        PolicyValueNet {
            vs,
            input_layer,
            policy_head,
            value_head,
        }
    }

    pub fn forward(&self, x: &Tensor) -> (Tensor, Tensor) {
        // Apply the input layer first
        let hidden = x.apply(&self.input_layer).relu();

        // Policy head outputs probabilities over 19 actions
        let policy = self.policy_head.forward(&hidden).softmax(-1, tch::Kind::Float);

        // Value head outputs a scalar value between -1 and 1
        let value = self.value_head.forward(&hidden).tanh();

        (policy, value)
    }
}
