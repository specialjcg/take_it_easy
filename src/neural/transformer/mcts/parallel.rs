use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use lru::LruCache;
use rayon::prelude::*;
use tch::Tensor;

use crate::game::game_state::GameState;
use crate::mcts::mcts_node::MCTSNode;
use super::super::{TransformerModel, TransformerError};

#[derive(Debug)]
pub enum ParallelMCTSError {
    TransformerError(TransformerError),
    BatchError(String),
    CacheError(String),
}

pub struct CacheConfig {
    pub capacity: usize,
    pub min_visits: usize,  // Nombre minimum de visites avant mise en cache
    pub temperature: f32,   // Température pour l'adaptation dynamique
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            capacity: 10000,
            min_visits: 5,
            temperature: 1.0,
        }
    }
}

pub struct ParallelTransformerMCTS {
    model: Arc<TransformerModel>,
    cache: Arc<Mutex<LruCache<String, (Vec<f32>, f32)>>>,
    batch_size: usize,
    config: CacheConfig,
    stats: Arc<Mutex<MCTSStats>>,
}

#[derive(Debug, Default)]
pub struct MCTSStats {
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub batch_sizes: Vec<usize>,
    pub prediction_times: Vec<f32>,
}

impl ParallelTransformerMCTS {
    pub fn new(model: TransformerModel, config: CacheConfig) -> Self {
        Self {
            model: Arc::new(model),
            cache: Arc::new(Mutex::new(LruCache::new(config.capacity))),
            batch_size: 16,  // Taille de batch initiale
            config,
            stats: Arc::new(Mutex::new(MCTSStats::default())),
        }
    }

    pub fn parallel_predict_batch(
        &self,
        states: Vec<&GameState>
    ) -> Result<Vec<(Vec<f32>, f32)>, ParallelMCTSError> {
        // Grouper les états en batches
        let batches: Vec<Vec<&GameState>> = states
            .chunks(self.batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // Traitement parallèle des batches
        let results: Vec<Result<Vec<(Vec<f32>, f32)>, ParallelMCTSError>> = batches
            .par_iter()
            .map(|batch| self.process_batch(batch))
            .collect();

        // Agréger les résultats
        let mut all_predictions = Vec::new();
        for result in results {
            all_predictions.extend(result?);
        }

        Ok(all_predictions)
    }

    fn process_batch(
        &self,
        states: &[&GameState]
    ) -> Result<Vec<(Vec<f32>, f32)>, ParallelMCTSError> {
        let mut predictions = Vec::new();
        let mut cache_misses = Vec::new();
        let mut cache_miss_indices = Vec::new();

        // Vérifier le cache pour chaque état
        for (idx, state) in states.iter().enumerate() {
            let state_key = self.compute_state_hash(state);
            if let Some(cached) = self.cache.lock().unwrap().get(&state_key) {
                predictions.push(cached.clone());
                self.stats.lock().unwrap().cache_hits += 1;
            } else {
                cache_misses.push(*state);
                cache_miss_indices.push(idx);
                self.stats.lock().unwrap().cache_misses += 1;
            }
        }

        // Traiter les états non cachés
        if !cache_misses.is_empty() {
            let model_predictions = self.batch_predict(&cache_misses)?;

            // Mettre à jour le cache et insérer les prédictions
            for (idx, pred) in model_predictions.into_iter().enumerate() {
                let state_key = self.compute_state_hash(cache_misses[idx]);
                self.cache.lock().unwrap().put(state_key, pred.clone());
                predictions.insert(cache_miss_indices[idx], pred);
            }
        }

        Ok(predictions)
    }

    fn batch_predict(
        &self,
        states: &[&GameState]
    ) -> Result<Vec<(Vec<f32>, f32)>, ParallelMCTSError> {
        // Encoder les états en batch
        let encoded_states = states.iter()
            .map(|state| self.encode_state(state))
            .collect::<Vec<Tensor>>();

        // Forward pass du modèle
        let batch_input = Tensor::stack(&encoded_states, 0);
        let output = self.model.forward(&batch_input)
            .map_err(ParallelMCTSError::TransformerError)?;

        // Décoder les sorties
        let mut predictions = Vec::new();
        for i in 0..states.len() {
            let slice = output.slice(0, i as i64, i as i64 + 1, 1);
            let (policy, value) = self.decode_prediction(&slice)?;
            predictions.push((policy, value));
        }

        Ok(predictions)
    }

    fn encode_state(&self, state: &GameState) -> Tensor {
        use crate::neural::transformer::game_state::GameStateFeatures;

        // Convertir l'état en features
        let features = state.to_tensor_features();

        // Créer un tenseur [seq_len=4, embedding_dim=64]
        // On reshape les 64 features en une séquence de 4 tokens de 16 dims chacun
        let embedding_dim = 64;
        let seq_len = 4;
        let features_per_token = embedding_dim / seq_len;

        let mut reshaped = vec![0.0f32; seq_len * features_per_token];
        for (i, &val) in features.iter().take(reshaped.len()).enumerate() {
            reshaped[i] = val;
        }

        Tensor::from_slice(&reshaped).view([seq_len as i64, features_per_token as i64])
    }

    fn decode_prediction(&self, output: &Tensor) -> Result<(Vec<f32>, f32), ParallelMCTSError> {
        use tch::Kind;
        use super::super::POLICY_OUTPUTS;

        // L'output doit être de forme [embedding_dim] après forward
        // On extrait la politique (19 premiers logits) et la valeur (moyenne)
        let output_flat = if output.dim() > 1 {
            output.flatten(0, -1)
        } else {
            output.shallow_clone()
        };

        // Extraire les 19 premiers éléments pour la politique
        let mut policy = Vec::with_capacity(POLICY_OUTPUTS as usize);
        for i in 0..POLICY_OUTPUTS {
            if i < output_flat.size()[0] {
                policy.push(output_flat.double_value(&[i]) as f32);
            } else {
                policy.push(0.0);
            }
        }

        // Normaliser la politique avec softmax
        let policy_sum: f32 = policy.iter().map(|&x| x.exp()).sum();
        if policy_sum > f32::EPSILON {
            for p in &mut policy {
                *p = p.exp() / policy_sum;
            }
        } else {
            // Distribution uniforme si la somme est trop petite
            let uniform = 1.0 / POLICY_OUTPUTS as f32;
            for p in &mut policy {
                *p = uniform;
            }
        }

        // La valeur est la moyenne du tenseur (ou on pourrait utiliser le dernier élément)
        let value = output_flat.mean(Kind::Float).double_value(&[]) as f32;
        let value = value.tanh(); // Normaliser entre -1 et 1

        Ok((policy, value))
    }

    fn compute_state_hash(&self, state: &GameState) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Créer un hash basé sur le plateau (pas le deck pour éviter l'ordre)
        let mut hasher = DefaultHasher::new();

        // Hash des tuiles du plateau
        for tile in &state.plateau.tiles {
            tile.0.hash(&mut hasher);
            tile.1.hash(&mut hasher);
            tile.2.hash(&mut hasher);
        }

        // Hash du nombre de tuiles restantes (approximation du deck)
        let non_zero_tiles = state.deck.tiles.iter().filter(|t| t.0 != 0 || t.1 != 0 || t.2 != 0).count();
        non_zero_tiles.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    pub fn adapt_parameters(&mut self) {
        let stats = self.stats.lock().unwrap();

        // Adapter la taille du batch en fonction des performances
        let avg_batch_size = stats.batch_sizes.iter().sum::<usize>() as f32
            / stats.batch_sizes.len() as f32;
        let avg_prediction_time = stats.prediction_times.iter().sum::<f32>()
            / stats.prediction_times.len() as f32;

        // Ajuster la taille du batch
        if avg_prediction_time < 0.1 {  // Si prédictions rapides
            self.batch_size = (self.batch_size as f32 * 1.2) as usize;
        } else if avg_prediction_time > 0.2 {  // Si prédictions lentes
            self.batch_size = (self.batch_size as f32 * 0.8) as usize;
        }

        // Borner la taille du batch
        self.batch_size = self.batch_size.clamp(4, 64);

        // Adapter la température du cache
        let hit_rate = stats.cache_hits as f32
            / (stats.cache_hits + stats.cache_misses) as f32;
        self.config.temperature = (1.0 - hit_rate).max(0.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::TransformerConfig;

    fn create_test_model() -> TransformerModel {
        let config = TransformerConfig::new(64, 2, 2).unwrap();
        TransformerModel::new(config).unwrap()
    }

    fn create_test_states(n: usize) -> Vec<GameState> {
        // TODO: Implémenter la création d'états de test
        unimplemented!()
    }

    #[test]
    fn test_parallel_mcts_creation() {
        let model = create_test_model();
        let config = CacheConfig::default();
        let parallel_mcts = ParallelTransformerMCTS::new(model, config);

        assert_eq!(parallel_mcts.batch_size, 16);
        assert_eq!(parallel_mcts.config.capacity, 10000);
    }

    #[test]
    fn test_cache_behavior() {
        let model = create_test_model();
        let config = CacheConfig::default();
        let parallel_mcts = ParallelTransformerMCTS::new(model, config);

        // TODO: Tester le comportement du cache une fois l'encodage implémenté
    }

    #[test]
    fn test_parameter_adaptation() {
        let model = create_test_model();
        let config = CacheConfig::default();
        let mut parallel_mcts = ParallelTransformerMCTS::new(model, config);

        // Simuler des statistiques
        {
            let mut stats = parallel_mcts.stats.lock().unwrap();
            stats.prediction_times.extend_from_slice(&[0.05; 10]);
            stats.batch_sizes.extend_from_slice(&[16; 10]);
            stats.cache_hits = 80;
            stats.cache_misses = 20;
        }

        parallel_mcts.adapt_parameters();

        // Vérifier l'adaptation
        assert!(parallel_mcts.batch_size > 16); // La taille du batch devrait augmenter
        assert!(parallel_mcts.config.temperature < 1.0); // La température devrait diminuer
    }
}
