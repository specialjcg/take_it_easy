#[cfg(test)]
mod tests {
    use crate::neural::transformer::{TransformerConfig, TransformerModel};
    use tch::{nn, Tensor};

    fn dummy_transformer() -> TransformerModel {
        let vs = nn::VarStore::new(tch::Device::Cpu);
        let config = TransformerConfig {
            embedding_dim: 8,
            num_heads: 2,
            num_layers: 1,
            ff_dim: 32,
            dropout_rate: None,
        };
        TransformerModel::new(config, &vs.root()).unwrap()
    }

    #[test]
    fn test_forward_accepts_2d_and_3d() {
        let model = dummy_transformer();
        // 2D input
        let input_2d = Tensor::randn(&[4, 8], (tch::Kind::Float, tch::Device::Cpu));
        let out_2d = model.forward(&input_2d).unwrap();
        assert_eq!(
            out_2d.size().len(),
            3,
            "La sortie doit être 3D pour une entrée 2D"
        );
        // 3D input
        let input_3d = Tensor::randn(&[2, 4, 8], (tch::Kind::Float, tch::Device::Cpu));
        let out_3d = model.forward(&input_3d).unwrap();
        assert_eq!(
            out_3d.size().len(),
            3,
            "La sortie doit rester 3D pour une entrée 3D"
        );
    }

    #[test]
    fn test_forward_rejects_bad_shape() {
        let model = dummy_transformer();
        // 1D input
        let input_1d = Tensor::randn(&[8], (tch::Kind::Float, tch::Device::Cpu));
        assert!(
            model.forward(&input_1d).is_err(),
            "Doit échouer pour une entrée 1D"
        );
        // 4D input
        let input_4d = Tensor::randn(&[2, 2, 4, 8], (tch::Kind::Float, tch::Device::Cpu));
        assert!(
            model.forward(&input_4d).is_err(),
            "Doit échouer pour une entrée 4D"
        );
    }
}
