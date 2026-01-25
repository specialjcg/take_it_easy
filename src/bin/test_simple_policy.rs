//! Test minimal : 1 linear layer policy, vérifier si optimizer fonctionne

use flexi_logger::Logger;
use tch::nn::OptimizerConfig;
use tch::{nn, Device, Tensor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger::try_with_env_or_str("info")?
        .format(flexi_logger::colored_default_format)
        .start()?;

    log::info!("🧪 Test Simple Policy - 1 Linear Layer");

    let device = Device::Cpu;
    let vs = nn::VarStore::new(device);
    let root = vs.root();

    // Simple linear: 10 → 19
    let linear = nn::linear(&root / "policy", 10, 19, Default::default());

    log::info!("✅ Linear layer created: 10 → 19");

    // Create optimizer
    let mut opt = nn::Adam::default().build(&vs, 0.01).unwrap();
    log::info!("✅ Optimizer created (LR=0.01)");

    // Training loop
    log::info!("\n🏋️ Training 20 epochs");

    for epoch in 0..20 {
        // Batch: 32 samples, 10 features
        let x = Tensor::randn([32, 10], (tch::Kind::Float, device));

        // Targets: first 16 → pos 5, last 16 → pos 12
        let mut targets = vec![5i64; 16];
        targets.extend(vec![12i64; 16]);
        let y = Tensor::from_slice(&targets).to_device(device);

        // Forward
        let logits = x.apply(&linear);

        // Loss
        let loss = logits.cross_entropy_for_logits(&y);
        let loss_val = f64::try_from(&loss)?;

        // Backward + step
        opt.backward_step(&loss);

        if epoch % 5 == 0 {
            log::info!("   Epoch {}: loss={:.6}", epoch, loss_val);
        }
    }

    log::info!("\n🎯 Si loss diminue → optimizer OK");
    log::info!("   Si loss constant → BUG OPTIMIZER/GRADIENT");

    Ok(())
}
