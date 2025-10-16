// Tests d'évaluation pour le Transformer

use take_it_easy::neural::transformer::evaluation::TransformerEvaluator;
use take_it_easy::neural::transformer::{TransformerConfig, TransformerModel};

struct DummyGameState;

impl take_it_easy::neural::transformer::evaluation::GameStateEval for DummyGameState {
    fn to_feature_vector(&self) -> Vec<f32> {
        // Retourne un vecteur correspondant à 19 positions * 3 valeurs (qui seront padées à 64)
        vec![0.0; 19 * 3]
    }
    fn is_valid_move(&self, _pos: usize) -> bool {
        true
    }
}

#[test]
fn test_transformer_evaluator_analyze_patterns() {
    // Crée un modèle Transformer minimal avec la config par défaut
    let config = TransformerConfig::default();
    let vs = tch::nn::VarStore::new(tch::Device::Cpu);
    let model = TransformerModel::new(config, &vs.root())
        .expect("La création du modèle Transformer doit réussir");
    let evaluator = TransformerEvaluator::new(model);
    // On crée un batch de 1, séquence de 1, embedding_dim = 64
    let states = vec![DummyGameState];
    let result = evaluator.analyze_patterns(&states);
    if let Err(e) = &result {
        eprintln!("Erreur retournée par analyze_patterns : {:?}", e);
    }
    assert!(result.is_ok(), "L'analyse des patterns doit réussir");
    let _patterns = result.unwrap();
    // Vérifie que les patterns et l'importance sont bien présents
    // (ici, on ne vérifie que la structure, pas la valeur)
    // À adapter selon la structure réelle de PatternAnalysis
}
