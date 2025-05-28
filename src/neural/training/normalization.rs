//! Normalisation robuste des données pour l'entraînement

use tch::{Tensor, IndexOp};

/// Applique une normalisation robuste basée sur la médiane (MAD)
pub fn robust_state_normalization(state: &Tensor) -> Tensor {
    let clamped = clamp_extreme_values(state);
    let median = calculate_median(&clamped);
    let mad = calculate_mad(&clamped, median);

    normalize_with_mad(&clamped, median, mad)
}

/// Clamp les valeurs extrêmes pour éviter les outliers
fn clamp_extreme_values(state: &Tensor) -> Tensor {
    state.clamp(-10.0, 10.0)
}

/// Calcule la médiane d'un tensor
fn calculate_median(tensor: &Tensor) -> f64 {
    let flattened = tensor.view(-1);
    let sorted = flattened.sort(0, false).0;
    let median_idx = sorted.size()[0] / 2;
    sorted.i(median_idx).double_value(&[])
}

/// Calcule la Median Absolute Deviation (MAD)
fn calculate_mad(tensor: &Tensor, median: f64) -> f64 {
    let flattened = tensor.view(-1);
    let deviations = (flattened - median).abs();
    let sorted_dev = deviations.sort(0, false).0;
    let median_idx = sorted_dev.size()[0] / 2;
    sorted_dev.i(median_idx).double_value(&[]) * 1.4826
}

/// Normalise avec MAD au lieu de l'écart-type standard
fn normalize_with_mad(tensor: &Tensor, median: f64, mad: f64) -> Tensor {
    let normalized = if mad > 1e-6 {
        (tensor - median) / mad.max(1e-6)
    } else {
        tensor - median
    };

    // Clamp final pour éviter les valeurs extrêmes
    normalized.clamp(-3.0, 3.0)
}

/// Version simple de normalisation z-score
#[allow(dead_code)]
pub fn simple_normalization(tensor: &Tensor) -> Tensor {
    let mean = tensor.mean(tch::Kind::Float);
    let std = tensor.std(false).clamp_min(1e-8);
    (tensor - mean) / std
}

/// Normalisation min-max
#[allow(dead_code)]
pub fn min_max_normalization(tensor: &Tensor) -> Tensor {
    let min_val = tensor.min();
    let max_val = tensor.max();
    let range = max_val - &min_val;

    if range.double_value(&[]) > 1e-8 {
        (tensor - min_val) / range
    } else {
        tensor.shallow_clone()
    }
}