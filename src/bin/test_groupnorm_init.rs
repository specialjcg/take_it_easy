use tch::{nn, Device};

fn main() {
    let vs = nn::VarStore::new(Device::Cpu);
    let gn = nn::group_norm(&vs.root() / "test_gn", 16, 128, Default::default());
    
    println!("GroupNorm parameters after creation:");
    for (name, param) in vs.variables() {
        let mean = param.mean(tch::Kind::Float).double_value(&[]);
        let std = param.std(false).double_value(&[]);
        println!("  {}: mean={:.6}, std={:.6}, shape={:?}", name, mean, std, param.size());
    }
}
