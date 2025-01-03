use tch::{nn, Tensor};

/// Squeeze-and-Excitation Block
/// A lightweight attention mechanism that learns channel-wise dependencies.
pub struct SqueezeExcitation {
    fc1: nn::Linear,
    fc2: nn::Linear,
}

impl SqueezeExcitation {
    /// Constructor for the SqueezeExcitation block.
    ///
    /// # Arguments
    /// * `vs` - Variable store for model parameters.
    /// * `channels` - Number of input/output channels.
    /// * `reduction` - Reduction ratio for the intermediate channel size.
    pub fn new(vs: nn::Path, channels: i64, reduction: i64) -> Self {
        let reduced_channels = channels / reduction;

        // Clone `vs` before each usage
        let vs_fc1 = vs.clone() / "fc1";
        let vs_fc2 = vs.clone() / "fc2";

        let fc1 = nn::linear(vs_fc1, channels, reduced_channels, Default::default());
        let fc2 = nn::linear(vs_fc2, reduced_channels, channels, Default::default());

        Self { fc1, fc2 }
    }



    /// Forward pass for the Squeeze-and-Excitation block.
    ///
    /// # Arguments
    /// * `x` - Input tensor of shape [batch, channels, height, width].
    ///
    /// # Returns
    /// Tensor of the same shape as input with channel-wise recalibration.
    pub fn forward(&self, x: Tensor) -> Tensor {
        // Global Average Pooling
        let s = x.mean_dim(&[-2_i64, -1_i64][..], true, tch::Kind::Float);

        // Flatten the tensor
        let s = s.view([-1, self.fc1.ws.size()[1]]);

        // Apply fully connected layers
        let s = s.apply(&self.fc1).relu();
        let s = s.apply(&self.fc2).sigmoid();

        // Reshape `s` and perform element-wise multiplication
        x * s.view([s.size()[0], s.size()[1], 1, 1])
    }



}

/// Residual Block with Squeeze-and-Excitation
pub struct ResNetBlock {
    pub(crate) conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    pub(crate) conv2: nn::Conv2D,
    bn2: nn::BatchNorm,
    se: SqueezeExcitation, // Integrate SE attention
}

impl ResNetBlock {
    /// Constructor for the Residual Block with SE.
    ///
    /// # Arguments
    /// * `vs` - Variable store for model parameters.
    /// * `channels_in` - Number of input channels.
    /// * `channels_out` - Number of output channels.
    pub fn new(vs: &nn::Path, channels_in: i64, channels_out: i64) -> Self {
        let conv1 = nn::conv2d(vs / "conv1", channels_in, channels_out, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn1 = nn::batch_norm2d(vs / "bn1", channels_out, Default::default());
        let conv2 = nn::conv2d(vs / "conv2", channels_out, channels_out, 3, nn::ConvConfig { padding: 1, ..Default::default() });
        let bn2 = nn::batch_norm2d(vs / "bn2", channels_out, Default::default());
        let se = SqueezeExcitation::new(vs / "se", channels_out, 16);

        Self { conv1, bn1, conv2, bn2, se }
    }

    /// Forward pass for the residual block.
    ///
    /// # Arguments
    /// * `x` - Input tensor of shape [batch, channels, height, width].
    ///
    /// # Returns
    /// Tensor of the same shape as input.
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let residual = x.shallow_clone();

        let x = x.apply(&self.conv1).apply_t(&self.bn1, train).relu();
        let x = x.apply(&self.conv2).apply_t(&self.bn2, train);
        let x = self.se.forward(x); // Apply SE recalibration

        (x + residual).relu()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use tch::{nn, Device};

    #[test]
    fn test_squeeze_excitation() {
        let vs = nn::VarStore::new(Device::Cpu);
        let se = SqueezeExcitation::new(vs.root(), 64, 16);

        let input = Tensor::rand(&[1, 64, 8, 8], tch::kind::FLOAT_CPU);
        let output = se.forward(input);

        assert_eq!(output.size(), vec![1, 64, 8, 8]);
    }

    #[test]
    fn test_resnet_block_with_se() {
        let vs = nn::VarStore::new(Device::Cpu);
        let res_block = ResNetBlock::new(&vs.root(), 64, 64);

        let input = Tensor::rand(&[1, 64, 8, 8], tch::kind::FLOAT_CPU);
        let output = res_block.forward(&input, true);

        assert_eq!(output.size(), vec![1, 64, 8, 8]);
    }
}
