use tch::{nn, Tensor};

/// Residual Block
pub struct ResNetBlock {
    pub(crate) conv1: nn::Conv2D,
    pub(crate) bn1: nn::BatchNorm,
    pub(crate) conv2: nn::Conv2D,
    pub(crate) bn2: nn::BatchNorm,
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
        let bn1 = nn::batch_norm2d(
            &(path / "bn1"),
            channels_out,
            nn::BatchNormConfig {
                ws_init: nn::Init::Const(1.0),
                bs_init: nn::Init::Const(0.0),
                ..Default::default()
            },
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
        let bn2 = nn::batch_norm2d(
            &(path / "bn2"),
            channels_out,
            nn::BatchNormConfig {
                ws_init: nn::Init::Const(1.0),
                bs_init: nn::Init::Const(0.0),
                ..Default::default()
            },
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
            bn1,
            conv2,
            bn2,
            downsample,
        }
    }

    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let residual = if let Some(downsample) = &self.downsample {
            x.apply(downsample)
        } else {
            x.shallow_clone()
        };

        let x = x
            .apply(&self.conv1)
            .apply_t(&self.bn1, train)
            .clamp(-1e3, 1e3) // Ensure this isn't in-place
            .relu(); // Ensure this isn't in-place
        let x = x
            .apply(&self.conv2)
            .apply_t(&self.bn2, train)
            .clamp(-1e3, 1e3); // Ensure this isn't in-place

        (x + residual).relu() // Safe addition and relu
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
