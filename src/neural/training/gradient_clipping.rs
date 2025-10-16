//! Gestion des gradients et clipping pour stabiliser l'entraînement

use tch::nn;

/// Résultat du clipping des gradients
#[derive(Debug)]
pub struct GradientClippingResult {
    pub max_grad_value: f64,
    pub max_grad_policy: f64,
}

/// Applique un clipping amélioré des gradients pour les réseaux value et policy
pub fn enhanced_gradient_clipping(
    vs_value: &nn::VarStore,
    vs_policy: &nn::VarStore,
) -> GradientClippingResult {
    let max_grad_value = clip_value_network_gradients(vs_value);
    let max_grad_policy = clip_policy_network_gradients(vs_policy);

    log_gradient_norms(max_grad_value, max_grad_policy);

    GradientClippingResult {
        max_grad_value,
        max_grad_policy,
    }
}

/// Applique un clipping agressif pour le réseau de valeur
fn clip_value_network_gradients(vs_value: &nn::VarStore) -> f64 {
    let mut max_grad_value: f64 = 0.0;

    tch::no_grad(|| {
        for (_name, tensor) in vs_value.variables() {
            if tensor.grad().defined() {
                let grad_norm = tensor.grad().norm().double_value(&[]);
                max_grad_value = max_grad_value.max(grad_norm);

                // Clipping très agressif pour stabilité
                let _ = tensor.grad().clamp_(-0.5, 0.5);
            }
        }
    });

    max_grad_value
}

/// Applique un clipping modéré pour le réseau de policy
fn clip_policy_network_gradients(vs_policy: &nn::VarStore) -> f64 {
    let mut max_grad_policy: f64 = 0.0;

    tch::no_grad(|| {
        for (_name, tensor) in vs_policy.variables() {
            if tensor.grad().defined() {
                let grad_norm = tensor.grad().norm().double_value(&[]);
                max_grad_policy = max_grad_policy.max(grad_norm);

                // Clipping modéré
                let _ = tensor.grad().clamp_(-1.0, 1.0);
            }
        }
    });

    max_grad_policy
}

/// Log les normes de gradients si elles sont élevées
fn log_gradient_norms(max_grad_value: f64, max_grad_policy: f64) {
    if max_grad_value > 1.0 {
        log::debug!("High gradient value norm: {}", max_grad_value);
    }
    if max_grad_policy > 2.0 {
        log::debug!("High gradient policy norm: {}", max_grad_policy);
    }
}

/// Version simple du clipping des gradients
#[allow(dead_code)]
pub fn simple_gradient_clipping(vs: &nn::VarStore, max_norm: f64) -> f64 {
    let mut max_grad: f64 = 0.0;

    tch::no_grad(|| {
        for (_name, tensor) in vs.variables() {
            if tensor.grad().defined() {
                let grad_norm = tensor.grad().norm().double_value(&[]);
                max_grad = max_grad.max(grad_norm);

                let _ = tensor.grad().clamp_(-max_norm, max_norm);
            }
        }
    });

    max_grad
}
