// Test d'apprentissage minimal pour le Transformer
use take_it_easy::neural::transformer::training::{TrainingConfig, TransformerSample, TransformerTrainer};
use take_it_easy::neural::transformer::TransformerConfig;
use tch::{Device, Kind, Tensor};

#[test]
fn test_transformer_minimal_learning() {
    // Config et modèle minimal
    let config = TransformerConfig::default();
    let device = Device::Cpu;
    let mut trainer = TransformerTrainer::new(
        config.clone(),
        TrainingConfig {
            num_epochs: 3,
            batch_size: 1,
            eval_games: 0, // Désactiver l'évaluation pour le test minimal
            eval_interval: 0,
            ..Default::default()
        },
        device,
    );

    // Données factices : lot de 1 exemple, séquence courte et embedding conforme à la config
    let seq_len = 4;
    let state = Tensor::randn(&[1, seq_len, config.embedding_dim()], (Kind::Float, device));

    // Cibles : distributions de politique et valeur
    let policy_raw = Tensor::ones(&[19], (Kind::Float, device)) / 19.0; // Distribution uniforme
    let policy_boosted = policy_raw.shallow_clone(); // Même chose pour ce test
    let value = Tensor::zeros(&[1], (Kind::Float, device));

    let sample = TransformerSample {
        state,
        policy_raw,
        policy_boosted,
        value,
        boost_intensity: 0.0,
    };

    let data = vec![sample];

    // Apprentissage minimal (doit s'exécuter sans panic)
    let result = trainer.train(data);
    assert!(
        result.is_ok(),
        "L'apprentissage minimal doit réussir : {:?}",
        result.err()
    );
}
