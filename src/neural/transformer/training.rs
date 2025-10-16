use super::{TransformerError, TransformerModel, POLICY_OUTPUTS};
use crate::game::create_deck::create_deck;
use crate::game::game_state::GameState;
use crate::game::get_legal_moves::get_legal_moves;
use crate::game::plateau::create_plateau_empty;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::remove_tile_from_deck::replace_tile_in_deck;
use crate::game::tile::Tile;
use crate::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use crate::neural::manager::{NeuralConfig, NeuralManager};
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::neural::transformer::mcts_integration::ParallelTransformerMCTS;
use crate::neural::transformer::TransformerConfig;
use crate::scoring::scoring::result;
use rand::seq::SliceRandom;
use rand::{rng, rngs::StdRng, Rng, SeedableRng};
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::result::Result;
use tch::nn::OptimizerConfig;
use tch::no_grad;
use tch::Reduction;
use tch::{nn, Device, IndexOp, Kind, Tensor};

const MAX_SCORE: f64 = 200.0;

#[derive(Debug)]
pub enum TrainingError {
    TransformerError(TransformerError),
    OptimizationError(String),
    DataError(String),
}

impl From<TransformerError> for TrainingError {
    fn from(err: TransformerError) -> Self {
        TrainingError::TransformerError(err)
    }
}

pub struct TrainingConfig {
    pub batch_size: i64,
    pub learning_rate: f64,
    pub num_epochs: i64,
    pub gradient_clip: f64,
    pub eval_games: usize,
    pub eval_interval: i64,
    pub baseline_simulations: usize,
    pub baseline_model_path: String,
    pub label_smoothing: f64,
    pub history_path: Option<String>,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            batch_size: 16,
            learning_rate: 5e-4,
            num_epochs: 100,
            gradient_clip: 1.0,
            eval_games: 20,
            eval_interval: 5,
            baseline_simulations: 150,
            baseline_model_path: "model_weights".to_string(),
            label_smoothing: 0.1,
            history_path: Some("transformer_training_history.csv".to_string()),
        }
    }
}

pub struct TransformerSample {
    pub state: Tensor,
    pub policy_raw: Tensor,
    pub policy_boosted: Tensor,
    pub value: Tensor,
    pub boost_intensity: f32,
}

pub struct TransformerTrainer {
    model: TransformerModel,
    optimizer: nn::Optimizer,
    config: TrainingConfig,
    device: Device,
    vs: nn::VarStore,
}

impl TransformerTrainer {
    pub fn new(model_config: TransformerConfig, config: TrainingConfig, device: Device) -> Self {
        let vs = nn::VarStore::new(device);
        let model = TransformerModel::new(model_config, &vs.root()).unwrap();
        let optimizer = nn::Adam::default()
            .build(&vs, config.learning_rate)
            .unwrap();
        Self {
            model,
            optimizer,
            config,
            device,
            vs,
        }
    }

    /// Entraîne le modèle sur des données d'entrée et des labels cibles (dummy minimal)
    pub fn train(&mut self, data: Vec<TransformerSample>) -> Result<(), TrainingError> {
        let mut best_eval_score: Option<f64> = None;
        let mut dataset = data;
        let mut rng = rng();
        let mut history_writer = self.prepare_history_writer()?;

        if dataset.is_empty() {
            log::warn!("[Transformer] Aucun échantillon pour l'entraînement — abandon.");
            return Ok(());
        }

        log::info!(
            "[Transformer] Démarrage entraînement — samples: {}, epochs: {}, batch: {}, lr: {:.2e}, smooth: {:.2}, eval_every: {}",
            dataset.len(),
            self.config.num_epochs,
            self.config.batch_size,
            self.config.learning_rate,
            self.config.label_smoothing,
            self.config.eval_interval
        );

        for epoch in 0..self.config.num_epochs {
            dataset.shuffle(&mut rng);
            let mut total_loss = 0.0;
            let mut total_samples = 0usize;

            let batch_size = self.config.batch_size.max(1) as usize;

            for batch in dataset.chunks(batch_size) {
                let mut inputs = Vec::with_capacity(batch.len());
                let mut policy_targets = Vec::with_capacity(batch.len());
                let mut value_targets = Vec::with_capacity(batch.len());
                let mut boosted_targets = Vec::with_capacity(batch.len());
                let mut boost_tracking = Vec::with_capacity(batch.len());

                for sample in batch {
                    let prepared_input =
                        match Self::prepare_input_tensor(&sample.state, self.model.config()) {
                            Some(tensor) => tensor.to_device(self.device).to_kind(Kind::Float),
                            None => {
                                log::warn!(
                                    "[TransformerTrainer] Entrée ignorée (shape {:?})",
                                    sample.state.size()
                                );
                                continue;
                            }
                        };

                    let policy_target = match Self::policy_target_distribution(
                        &sample.policy_raw,
                        self.device,
                        self.config.label_smoothing,
                    ) {
                        Some(tensor) => tensor,
                        None => {
                            log::warn!(
                                "[TransformerTrainer] Politique cible invalide (tensor {:?})",
                                sample.policy_raw.size()
                            );
                            continue;
                        }
                    };

                    let value_target =
                        match Self::value_target_tensor(&sample.value, self.device, Kind::Float) {
                            Some(tensor) => tensor,
                            None => {
                                log::warn!(
                                    "[TransformerTrainer] Valeur cible invalide (tensor {:?})",
                                    sample.value.size()
                                );
                                continue;
                            }
                        };

                    let boosted_target = sample
                        .policy_boosted
                        .to_device(self.device)
                        .to_kind(Kind::Float)
                        .view([1, policy_target.size()[1]]);

                    inputs.push(prepared_input);
                    policy_targets.push(policy_target);
                    value_targets.push(value_target);
                    boosted_targets.push(boosted_target);
                    boost_tracking.push(sample.boost_intensity);
                }

                if inputs.is_empty() {
                    continue;
                }

                let mut input_batch = Tensor::stack(&inputs, 0);
                input_batch = Self::normalize_batch(input_batch);

                let policy_target_batch = Tensor::stack(&policy_targets, 0);
                let boosted_target_batch = Tensor::stack(&boosted_targets, 0);
                let value_target_batch = Tensor::stack(&value_targets, 0);

                // Créer un target de boost binaire : 1 si la position a été boostée, 0 sinon
                // On détecte les positions boostées en comparant policy_boosted vs policy_raw
                let boost_target_batch = (&boosted_target_batch - &policy_target_batch)
                    .abs()
                    .gt(0.01) // Si différence > 1%, c'est une position boostée
                    .to_kind(Kind::Float);

                let (policy_logits, value_pred, boost_logits) = self
                    .model
                    .infer_with_boost(&input_batch)
                    .map_err(|e| TrainingError::OptimizationError(e.to_string()))?;

                let log_probs = policy_logits.log_softmax(-1, Kind::Float);
                let policy_loss = -(policy_target_batch.shallow_clone() * log_probs)
                    .sum_dim_intlist(&[-1i64][..], false, Kind::Float)
                    .mean(Kind::Float);

                let value_loss = value_pred
                    .view([-1])
                    .mse_loss(&value_target_batch.view([-1]), Reduction::Mean);

                // Loss de prédiction de boost (BCE loss)
                let boost_probs = boost_logits.sigmoid();
                let boost_loss = boost_probs
                    .binary_cross_entropy::<Tensor>(&boost_target_batch, None, Reduction::Mean);

                // Combiner les trois losses (pondérer boost loss à 0.3 pour ne pas dominer)
                let loss: Tensor = policy_loss + value_loss + boost_loss * 0.3;

                if log::log_enabled!(log::Level::Trace) {
                    let kl = Self::kl_divergence(&policy_target_batch, &boosted_target_batch);
                    let avg_boost: f32 = if boost_tracking.is_empty() {
                        0.0
                    } else {
                        boost_tracking.iter().sum::<f32>() / boost_tracking.len() as f32
                    };
                    log::trace!(
                        "[TransformerTrainer] batch KL(raw||boosted)={:.6} avg_boost={:.2}",
                        kl,
                        avg_boost
                    );
                }

                self.optimizer.zero_grad();
                loss.backward();

                if self
                    .vs
                    .trainable_variables()
                    .into_iter()
                    .any(|tensor| tensor.grad().defined())
                {
                    self.optimizer.clip_grad_norm(self.config.gradient_clip);
                }

                self.optimizer.step();

                total_loss += loss.double_value(&[]) * inputs.len() as f64;
                total_samples += inputs.len();
            }

            if total_samples == 0 {
                log::warn!(
                    "[TransformerTrainer] Époque {} ignorée — aucun batch valable.",
                    epoch + 1
                );
                continue;
            }

            let average_loss = total_loss / total_samples as f64;
            println!("Epoch {}: loss = {:.4}", epoch + 1, average_loss);

            let mut transformer_avg = None;
            let mut baseline_avg = None;

            if self.config.eval_interval > 0
                && self.config.eval_games > 0
                && ((epoch + 1) % self.config.eval_interval == 0)
            {
                transformer_avg = self.evaluate_transformer_games(self.config.eval_games);

                if let Some(avg_score) = transformer_avg {
                    log::info!(
                        "[Transformer] Époque {} — score moyen ({:?} parties) : {:.2}",
                        epoch + 1,
                        self.config.eval_games,
                        avg_score
                    );
                    if best_eval_score.map_or(true, |best| avg_score > best) {
                        best_eval_score = Some(avg_score);
                    }
                } else {
                    log::warn!(
                        "[Transformer] Époque {} — évaluation indisponible (aucune partie jouée)",
                        epoch + 1
                    );
                }

                baseline_avg = self.evaluate_against_baseline(self.config.eval_games);
                if let Some((transformer_score, baseline_score)) = baseline_avg {
                    log::info!(
                        "[Transformer] Benchmark MCTS vs Transformer — {:.2} / {:.2}",
                        transformer_score,
                        baseline_score
                    );
                }
            }

            if let Some(writer) = history_writer.as_mut() {
                let transformer_score = transformer_avg
                    .map(|v| format!("{:.2}", v))
                    .unwrap_or_else(|| "".to_string());
                let baseline_scores = baseline_avg
                    .map(|(t, b)| format!("{:.2},{:.2}", t, b))
                    .unwrap_or_else(|| ",".to_string());
                writeln!(
                    writer,
                    "{},{:.6},{},{}",
                    epoch + 1,
                    average_loss,
                    transformer_score,
                    baseline_scores
                )
                .map_err(|e| TrainingError::DataError(e.to_string()))?;
            }
        }

        if let Some(writer) = history_writer.as_mut() {
            writer
                .flush()
                .map_err(|e| TrainingError::DataError(e.to_string()))?;
        }

        if let Some(best) = best_eval_score {
            log::info!(
                "[Transformer] Meilleur score moyen observé pendant l'entraînement : {:.2}",
                best
            );
        }

        log::info!(
            "[Transformer] Entraînement terminé — epochs: {}, best_eval: {}, historique: {}",
            self.config.num_epochs,
            best_eval_score
                .map(|v| format!("{:.2}", v))
                .unwrap_or_else(|| "n/a".to_string()),
            self.config.history_path.as_deref().unwrap_or("<désactivé>")
        );

        Ok(())
    }

    fn prepare_history_writer(&self) -> Result<Option<BufWriter<std::fs::File>>, TrainingError> {
        if let Some(path) = &self.config.history_path {
            let path_ref = Path::new(path);
            if let Some(parent) = path_ref.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| TrainingError::DataError(e.to_string()))?;
                }
            }
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path_ref)
                .map_err(|e| TrainingError::DataError(e.to_string()))?;
            let is_empty = file
                .metadata()
                .map_err(|e| TrainingError::DataError(e.to_string()))?
                .len()
                == 0;
            let mut writer = BufWriter::new(file);
            if is_empty {
                writeln!(
                    writer,
                    "epoch,loss,transformer_avg,transformer_vs_mcts,baseline_mcts"
                )
                .map_err(|e| TrainingError::DataError(e.to_string()))?;
            }
            Ok(Some(writer))
        } else {
            Ok(None)
        }
    }

    fn normalize_batch(tensor: Tensor) -> Tensor {
        let mean = tensor.mean_dim(&[-1i64][..], true, Kind::Float);
        let std = tensor
            .var_dim(&[-1i64][..], false, true)
            .sqrt()
            .clamp_min(1e-5);
        (tensor - &mean) / std
    }

    fn kl_divergence(raw: &Tensor, boosted: &Tensor) -> f32 {
        let raw = raw.flatten(0, -1).to_kind(Kind::Float);
        let boosted = boosted.flatten(0, -1).to_kind(Kind::Float);
        if raw.size() != boosted.size() {
            return 0.0;
        }
        let raw_clamped = (&raw + 1e-6).clamp_min(1e-6);
        let boosted_clamped = (&boosted + 1e-6).clamp_min(1e-6);
        let ratio = raw_clamped.shallow_clone() / boosted_clamped;
        let kl = (raw_clamped * ratio.log()).sum(Kind::Float);
        kl.double_value(&[]) as f32
    }

    fn policy_target_distribution(
        target_policy: &Tensor,
        device: Device,
        smoothing: f64,
    ) -> Option<Tensor> {
        if POLICY_OUTPUTS <= 1 {
            return None;
        }

        let flattened = target_policy
            .shallow_clone()
            .to_device(device)
            .to_kind(Kind::Float)
            .flatten(0, -1);
        let len = flattened.size()[0];

        if len == 0 {
            return None;
        }

        if len == 1 {
            let idx = flattened.int64_value(&[0]);
            let classes = (POLICY_OUTPUTS - 1).max(1) as f64;
            let smoothing = smoothing.clamp(0.0, 0.4);
            let off_value = if classes > 0.0 {
                smoothing / classes
            } else {
                0.0
            };
            let on_value = 1.0 - smoothing;
            let target = Tensor::full([POLICY_OUTPUTS], off_value, (Kind::Float, device));
            let _ = target.i(idx).fill_(on_value);
            return Some(target.view([1, POLICY_OUTPUTS]));
        }

        let mut distribution = flattened;
        let total = distribution.sum(Kind::Float).double_value(&[]);
        if total > f64::EPSILON {
            distribution = &distribution / total;
        } else {
            distribution = Tensor::full([len], 1.0 / len as f64, (Kind::Float, device));
        }

        let smoothing = smoothing.clamp(0.0, 0.4);
        if smoothing > 0.0 {
            let uniform = Tensor::full([len], smoothing / len as f64, (Kind::Float, device));
            distribution = distribution * (1.0 - smoothing) + uniform;
        }

        Some(distribution.view([1, len]))
    }

    fn evaluate_against_baseline(&self, num_games: usize) -> Option<(f64, f64)> {
        if num_games == 0 {
            return None;
        }

        let mut neural_config = NeuralConfig::default();
        neural_config.model_path = self.config.baseline_model_path.clone();
        let manager = match NeuralManager::with_config(neural_config) {
            Ok(manager) => manager,
            Err(err) => {
                log::warn!(
                    "[Transformer] Impossible de charger la baseline MCTS: {}",
                    err
                );
                return None;
            }
        };

        let policy_net = manager.policy_net();
        let value_net = manager.value_net();

        let evaluator = ParallelTransformerMCTS::with_device(self.model.clone(), self.device);
        let mut seed_rng = rng();
        let mut transformer_total = 0.0;
        let mut baseline_total = 0.0;
        let mut completed = 0usize;

        for _ in 0..num_games {
            let seed = seed_rng.random::<u64>();
            let mut rng_transformer = StdRng::seed_from_u64(seed);
            let mut rng_mcts = StdRng::seed_from_u64(seed);

            match Self::play_transformer_game(&evaluator, &mut rng_transformer) {
                Some(score) => {
                    let baseline_score = Self::play_mcts_game(
                        &mut rng_mcts,
                        policy_net,
                        value_net,
                        self.config.baseline_simulations,
                    );
                    transformer_total += score as f64;
                    baseline_total += baseline_score as f64;
                    completed += 1;
                }
                None => {
                    log::warn!("[TransformerEval] Partie Transformer invalide — ignorée.");
                }
            }
        }

        if completed == 0 {
            None
        } else {
            Some((
                transformer_total / completed as f64,
                baseline_total / completed as f64,
            ))
        }
    }

    fn prepare_input_tensor(input: &Tensor, config: &TransformerConfig) -> Option<Tensor> {
        let embedding_dim = config.embedding_dim();
        if embedding_dim <= 0 {
            return None;
        }

        let tensor_3d = match input.dim() {
            d if d >= 3 => {
                let sizes = input.size();
                if sizes.len() < 3 {
                    return None;
                }
                let batch = sizes[0];
                let seq = sizes[1];
                let feature = sizes[2..]
                    .iter()
                    .try_fold(1i64, |acc, &s| acc.checked_mul(s))?;
                if feature <= 0 {
                    return None;
                }
                Some(input.shallow_clone().view([batch, seq, feature]))
            }
            2 => {
                let sizes = input.size();
                let seq = sizes[0];
                let dim = sizes[1];
                Some(input.shallow_clone().view([1, seq, dim]))
            }
            1 => {
                let embedding_dim_usize: usize = embedding_dim.try_into().ok()?;
                if embedding_dim_usize == 0 {
                    return None;
                }
                let total: usize = input.numel().try_into().ok()?;
                if total % embedding_dim_usize != 0 {
                    return None;
                }
                let seq_len = (total / embedding_dim_usize) as i64;
                Some(
                    input
                        .shallow_clone()
                        .view([seq_len, embedding_dim])
                        .unsqueeze(0),
                )
            }
            _ => None,
        }?;

        let tensor = Self::adjust_embedding_dim(tensor_3d, embedding_dim)?;
        if tensor.size()[0] == 1 {
            Some(tensor.squeeze_dim(0))
        } else {
            Some(tensor)
        }
    }

    fn value_target_tensor(target_value: &Tensor, device: Device, kind: Kind) -> Option<Tensor> {
        let tensor = target_value.shallow_clone().to_device(device).to_kind(kind);
        if tensor.numel() == 0 {
            None
        } else {
            let normalized = ((tensor / MAX_SCORE).clamp(-1.0, 1.0) * 2.0 - 1.0).view([-1]);
            Some(normalized)
        }
    }

    fn adjust_embedding_dim(tensor: Tensor, embedding_dim: i64) -> Option<Tensor> {
        if embedding_dim <= 0 {
            return None;
        }
        let mut tensor = tensor;
        let sizes = tensor.size();
        if sizes.len() != 3 {
            return None;
        }
        let current_dim = sizes[2];
        if current_dim == embedding_dim {
            return Some(tensor);
        }

        if current_dim < embedding_dim {
            let pad = Tensor::zeros(
                &[sizes[0], sizes[1], embedding_dim - current_dim],
                (tensor.kind(), tensor.device()),
            );
            tensor = Tensor::cat(&[tensor, pad], 2);
            Some(tensor)
        } else {
            Some(tensor.narrow(2, 0, embedding_dim))
        }
    }

    fn evaluate_transformer_games(&self, num_games: usize) -> Option<f64> {
        if num_games == 0 {
            return None;
        }

        let evaluator = ParallelTransformerMCTS::with_device(self.model.clone(), self.device);
        no_grad(|| Self::run_transformer_evaluation(&evaluator, num_games))
    }

    fn run_transformer_evaluation(
        evaluator: &ParallelTransformerMCTS,
        num_games: usize,
    ) -> Option<f64> {
        let mut total_score = 0.0;
        let mut games_completed = 0usize;

        let mut seed_rng = rng();

        for _ in 0..num_games {
            let seed = seed_rng.random::<u64>();
            let mut game_rng = StdRng::seed_from_u64(seed);
            match Self::play_transformer_game(evaluator, &mut game_rng) {
                Some(score) => {
                    total_score += score as f64;
                    games_completed += 1;
                }
                None => log::warn!("[TransformerEval] Partie interrompue — score ignoré."),
            }
        }

        if games_completed == 0 {
            None
        } else {
            Some(total_score / games_completed as f64)
        }
    }

    fn play_transformer_game<R: Rng + ?Sized>(
        evaluator: &ParallelTransformerMCTS,
        rng: &mut R,
    ) -> Option<i32> {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();

        while !is_plateau_full(&plateau) {
            let available_tiles: Vec<(usize, Tile)> = deck
                .tiles
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, tile)| *tile != Tile(0, 0, 0))
                .collect();

            if available_tiles.is_empty() {
                break;
            }

            let choice_idx = rng.random_range(0..available_tiles.len());
            let (_, chosen_tile) = available_tiles[choice_idx];

            let legal_moves = get_legal_moves(plateau.clone());
            if legal_moves.is_empty() {
                break;
            }

            let game_state = GameState {
                plateau: plateau.clone(),
                deck: deck.clone(),
            };

            let (policy, _) = match evaluator.parallel_predict_batch(&[&game_state]) {
                Ok(predictions) => {
                    if let Some(first) = predictions.into_iter().next() {
                        first
                    } else {
                        log::warn!("[TransformerEval] Prédiction vide reçue.");
                        return None;
                    }
                }
                Err(e) => {
                    log::warn!("[TransformerEval] Erreur de prédiction: {}", e);
                    return None;
                }
            };

            let mut best_position = legal_moves[0];
            let mut best_score = f32::MIN;

            for &position in &legal_moves {
                if position >= policy.len() {
                    continue;
                }
                let score = policy[position];
                if score > best_score {
                    best_score = score;
                    best_position = position;
                }
            }

            plateau.tiles[best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
        }

        Some(result(&plateau))
    }

    fn play_mcts_game<R: Rng + ?Sized>(
        rng: &mut R,
        policy_net: &PolicyNet,
        value_net: &ValueNet,
        num_simulations: usize,
    ) -> i32 {
        let mut deck = create_deck();
        let mut plateau = create_plateau_empty();
        let total_turns = 19;
        let mut current_turn = 0;

        while !is_plateau_full(&plateau) {
            let available_tiles: Vec<(usize, Tile)> = deck
                .tiles
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, tile)| *tile != Tile(0, 0, 0))
                .collect();

            if available_tiles.is_empty() {
                break;
            }

            let choice_idx = rng.random_range(0..available_tiles.len());
            let (_, chosen_tile) = available_tiles[choice_idx];

            let mut plateau_for_search = plateau.clone();
            let mut deck_for_search = deck.clone();

            let result = mcts_find_best_position_for_tile_with_nn(
                &mut plateau_for_search,
                &mut deck_for_search,
                chosen_tile,
                policy_net,
                value_net,
                num_simulations,
                current_turn,
                total_turns,
            );

            let best_position = result.best_position;
            plateau.tiles[best_position] = chosen_tile;
            deck = replace_tile_in_deck(&deck, &chosen_tile);
            current_turn += 1;
        }

        result(&plateau)
    }

    pub fn load_weights<P: AsRef<Path>>(&mut self, path: P) -> Result<(), TrainingError> {
        let path = path.as_ref();
        if !path.exists() {
            log::info!("[Transformer] Aucun poids existant à charger ({:?})", path);
            return Ok(());
        }

        let path_str = path.to_str().ok_or_else(|| {
            TrainingError::DataError(format!("Chemin de poids invalide: {:?}", path))
        })?;

        self.model
            .load_model(path_str)
            .map_err(|e| TrainingError::OptimizationError(e.to_string()))?;

        log::info!("[Transformer] Poids chargés depuis {:?}", path);
        Ok(())
    }

    pub fn save_weights<P: AsRef<Path>>(&self, path: P) -> Result<(), TrainingError> {
        let path = path.as_ref();
        if let Err(e) = std::fs::create_dir_all(path) {
            return Err(TrainingError::DataError(format!(
                "Impossible de créer le dossier {:?}: {}",
                path, e
            )));
        }

        let path_str = path.to_str().ok_or_else(|| {
            TrainingError::DataError(format!("Chemin de sauvegarde invalide: {:?}", path))
        })?;

        self.model
            .save_model(path_str)
            .map_err(|e| TrainingError::OptimizationError(e.to_string()))?;

        log::info!("[Transformer] Poids sauvegardés dans {:?}", path);
        Ok(())
    }
}
