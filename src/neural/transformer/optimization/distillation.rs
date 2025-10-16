use tch::{Tensor, Kind, Device, nn};
use std::result::Result;
use super::super::{TransformerModel, TransformerError};

#[derive(Debug)]
pub enum DistillationError {
    TeacherError(TransformerError),
    StudentError(TransformerError),
    OptimizationError(String),
}

pub struct DistillationConfig {
    pub temperature: f64,
    pub alpha: f64,      // Poids entre distillation et tâche originale
    pub batch_size: i64,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            temperature: 2.0,
            alpha: 0.5,
            batch_size: 32,
        }
    }
}

pub struct KnowledgeDistiller {
    teacher: TransformerModel,
    student: TransformerModel,
    config: DistillationConfig,
}

impl KnowledgeDistiller {
    pub fn new(
        teacher: TransformerModel,
        student: TransformerModel,
        config: DistillationConfig,
    ) -> Self {
        Self {
            teacher,
            student,
            config,
        }
    }

    pub fn distill_batch(
        &self,
        input: &Tensor,
        target: &Tensor,
    ) -> Result<(Tensor, Tensor), DistillationError> {
        // Obtenir les prédictions du teacher
        let teacher_logits = self.teacher.forward(input)
            .map_err(DistillationError::TeacherError)?;

        // Softmax avec température pour le teacher
        let soft_targets = (teacher_logits / self.config.temperature)
            .softmax(-1, Kind::Float);

        // Forward pass du student
        let student_logits = self.student.forward(input)
            .map_err(DistillationError::StudentError)?;

        // Calcul des pertes
        let distillation_loss = compute_distillation_loss(
            &student_logits,
            &soft_targets,
            self.config.temperature,
        );

        let task_loss = compute_task_loss(&student_logits, target);

        // Combinaison des pertes
        let total_loss = self.config.alpha * distillation_loss +
                        (1.0 - self.config.alpha) * task_loss;

        Ok((total_loss, student_logits))
    }
}

fn compute_distillation_loss(
    student_logits: &Tensor,
    soft_targets: &Tensor,
    temperature: f64,
) -> Tensor {
    let log_probs = (student_logits / temperature).log_softmax(-1, Kind::Float);
    -(soft_targets * log_probs).sum_dim_intlist(&[-1], false, Kind::Float).mean()
}

fn compute_task_loss(logits: &Tensor, targets: &Tensor) -> Tensor {
    logits.cross_entropy_loss(targets, None, tch::Reduction::Mean, -1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::TransformerConfig;

    fn create_test_models() -> (TransformerModel, TransformerModel) {
        let teacher_config = TransformerConfig::new(64, 4, 4).unwrap();
        let student_config = TransformerConfig::new(64, 2, 2).unwrap();

        let teacher = TransformerModel::new(teacher_config).unwrap();
        let student = TransformerModel::new(student_config).unwrap();

        (teacher, student)
    }

    #[test]
    fn test_distillation_setup() {
        let (teacher, student) = create_test_models();
        let config = DistillationConfig::default();
        let distiller = KnowledgeDistiller::new(teacher, student, config);

        let input = Tensor::rand(&[16, 4, 64], (Kind::Float, Device::Cpu));
        let target = Tensor::rand(&[16, 19], (Kind::Float, Device::Cpu));

        let result = distiller.distill_batch(&input, &target);
        assert!(result.is_ok());
    }

    #[test]
    fn test_loss_computation() {
        let (teacher, student) = create_test_models();
        let config = DistillationConfig::default();
        let distiller = KnowledgeDistiller::new(teacher, student, config);

        let input = Tensor::rand(&[8, 4, 64], (Kind::Float, Device::Cpu));
        let target = Tensor::rand(&[8, 19], (Kind::Float, Device::Cpu));

        let (loss, _) = distiller.distill_batch(&input, &target).unwrap();
        assert!(loss.dim() == 0); // La perte devrait être un scalaire
        assert!(loss.double_value(&[]) > 0.0); // La perte devrait être positive
    }
}
