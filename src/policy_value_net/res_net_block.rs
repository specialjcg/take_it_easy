use tch::{nn, Tensor};
use tch::nn::Path;

pub struct ResNetBlock {
    pub(crate) conv1: nn::Conv2D,
    bn1: nn::BatchNorm,
    pub(crate) conv2: nn::Conv2D,
    bn2: nn::BatchNorm,
}

impl ResNetBlock {
    pub fn new(vs: Path, in_channels: i64, out_channels: i64) -> Self {
        let conv1 = nn::conv2d(vs.clone() / "conv1", in_channels, out_channels, 3, nn::ConvConfig {
            stride: 1,
            padding: 1,
            bias: false,
            ..Default::default()
        });

        let bn1 = nn::batch_norm2d(vs.clone() / "bn1", out_channels, Default::default());

        let conv2 = nn::conv2d(vs.clone() / "conv2", out_channels, out_channels, 3, nn::ConvConfig {
            stride: 1,
            padding: 1,
            bias: false,
            ..Default::default()
        });

        let bn2 = nn::batch_norm2d(vs / "bn2", out_channels, Default::default());

        Self {
            conv1,
            bn1,
            conv2,
            bn2,
        }
    }


    pub fn forward(&self, x: &Tensor) -> Tensor {
        let residual = x.shallow_clone(); // Connexion résiduelle
        let x = x.apply(&self.conv1).apply_t(&self.bn1, true).relu();
        let x = x.apply(&self.conv2).apply_t(&self.bn2, true);
        x + residual // Ajout de la connexion résiduelle
    }
}
