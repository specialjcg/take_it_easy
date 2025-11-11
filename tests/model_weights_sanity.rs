use std::path::Path;

use take_it_easy::neural::manager::NNArchitecture;
use take_it_easy::neural::policy_value_net::{PolicyNet, ValueNet};
use tch::{nn, Device};

const EXPECTED_DIM: (i64, i64, i64) = (8, 5, 5);

fn load_policy() -> Result<(), String> {
    let path = Path::new("../model_weights/cnn/policy/policy.params");
    if !path.exists() {
        return Ok(()); // nothing to check
    }

    let mut vs = nn::VarStore::new(Device::Cpu);
    let policy_net = PolicyNet::new(&vs, EXPECTED_DIM, NNArchitecture::CNN);
    policy_net
        .load_model(&mut vs, path.to_str().unwrap())
        .map_err(|err| format!("policy weights mismatch: {:?}", err))
}

fn load_value() -> Result<(), String> {
    let path = Path::new("../model_weights/cnn/value/value.params");
    if !path.exists() {
        return Ok(()); // nothing to check
    }

    let mut vs = nn::VarStore::new(Device::Cpu);
    let value_net = ValueNet::new(&vs, EXPECTED_DIM, NNArchitecture::CNN);
    value_net
        .load_model(&mut vs, path.to_str().unwrap())
        .map_err(|err| format!("value weights mismatch: {:?}", err))
}

#[test]
fn model_weights_match_expected_channels() {
    load_policy().unwrap();
    load_value().unwrap();
}
