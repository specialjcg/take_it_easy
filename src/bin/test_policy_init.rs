use tch::{nn, Device};
use take_it_easy::neural::policy_value_net::PolicyNet;
use take_it_easy::neural::manager::NNArchitecture;

fn main() {
    let vs = nn::VarStore::new(Device::Cpu);
    let _policy_net = PolicyNet::new(&vs, (9, 5, 5), NNArchitecture::Cnn);
    
    println!("\nPolicyNetCNN parameters after creation:");
    for (name, param) in vs.variables() {
        let mean = param.mean(tch::Kind::Float).double_value(&[]);
        let std = param.std(false).double_value(&[]);
        println!("  {}: mean={:.6}, std={:.6}, shape={:?}", name, mean, std, param.size());
    }
}
