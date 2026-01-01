use tch::{nn, Tensor};

/// Residual Block with GroupNorm (more stable than BatchNorm for gradients)
pub struct ResNetBlock {
    pub(crate) conv1: nn::Conv2D,
    pub(crate) gn1: nn::GroupNorm,  // Changed from BatchNorm to GroupNorm
    pub(crate) conv2: nn::Conv2D,
    pub(crate) gn2: nn::GroupNorm,  // Changed from BatchNorm to GroupNorm
    downsample: Option<nn::Conv2D>, // Optional downsampling for skip connections
}

impl ResNetBlock {
    #[allow(dead_code)]
    pub fn new(vs: &nn::VarStore, channels_in: i64, channels_out: i64) -> Self {
        Self::new_path(&vs.root(), channels_in, channels_out)
    }

    pub fn new_path(path: &nn::Path, channels_in: i64, channels_out: i64) -> Self {
        let conv1 = nn::conv2d(
            &(path / "conv1"),
            channels_in,
            channels_out,
            3,
            nn::ConvConfig {
                padding: 1,
                ..Default::default()
            },
        );
        // GroupNorm: 16 groups, more stable than BatchNorm for gradient flow
        let gn1 = nn::group_norm(
            &(path / "gn1"),
            16,
            channels_out,
            Default::default(),
        );
        let conv2 = nn::conv2d(
            &(path / "conv2"),
            channels_out,
            channels_out,
            3,
            nn::ConvConfig {
                padding: 1,
                ..Default::default()
            },
        );
        // GroupNorm: 16 groups, more stable than BatchNorm for gradient flow
        let gn2 = nn::group_norm(
            &(path / "gn2"),
            16,
            channels_out,
            Default::default(),
        );

        // Downsample if input/output channels differ
        let downsample = if channels_in != channels_out {
            Some(nn::conv2d(
                &(path / "downsample"),
                channels_in,
                channels_out,
                1,
                Default::default(),
            ))
        } else {
            None
        };

        Self {
            conv1,
            gn1,
            conv2,
            gn2,
            downsample,
        }
    }

    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        // Standard ResNet block with skip connection
        let identity = if let Some(downsample) = &self.downsample {
            x.apply(downsample)
        } else {
            x.shallow_clone()
        };

        // First conv block
        let out = x
            .apply(&self.conv1)
            .apply_t(&self.gn1, train)
            .relu();

        // Second conv block (no activation yet)
        let out = out
            .apply(&self.conv2)
            .apply_t(&self.gn2, train);

        // Add skip connection and activate
        (out + identity).relu()
    }
}

#[cfg(test)]
mod tests {
    use tch::{nn, Device};

    use super::*;

    #[test]
    fn test_resnet_block() {
        let vs = nn::VarStore::new(Device::Cpu);
        let res_block = ResNetBlock::new(&vs, 64, 64);

        let input = Tensor::rand(&[1, 64, 8, 8], tch::kind::FLOAT_CPU);
        let output = res_block.forward(&input, true);

        assert_eq!(output.size(), vec![1, 64, 8, 8]);
    }
}
