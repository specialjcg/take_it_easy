use tch::{nn, Device};

fn main() {
    println!("Testing tch 0.18.0 GroupNorm initialization:");

    let vs = nn::VarStore::new(Device::Cpu);

    // Create multiple GroupNorms to see if it's consistent    let gn1 = nn::group_norm(&vs.root() / "gn1", 16, 128, Default::default());
    let _gn2 = nn::group_norm(&vs.root() / "gn2", 16, 96, Default::default());

    println!("\nGroupNorm parameters:");
    for (name, param) in vs.variables() {
        let mean = param.mean(tch::Kind::Float).double_value(&[]);
        println!("  {}: mean={:.6}, shape={:?}", name, mean, param.size());
    }

    // Create a conv too to see if it's a general issue    let _conv = nn::conv2d(&vs.root() / "conv", 9, 128, 3, Default::default());

    println!("\nAfter adding conv:");
    for (name, param) in vs.variables() {
        let mean = param.mean(tch::Kind::Float).double_value(&[]);
        println!("  {}: mean={:.6}, shape={:?}", name, mean, param.size());
    }
}
