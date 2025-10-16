use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ProfilingStats {
    pub total_time: Duration,
    pub call_count: usize,
    pub avg_memory_usage: f64,
    pub peak_memory_usage: f64,
}

pub struct TransformerProfiler {
    stats: Arc<Mutex<HashMap<String, ProfilingStats>>>,
    start_times: HashMap<String, Instant>,
    current_memory: HashMap<String, Vec<f64>>,
}

impl TransformerProfiler {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(HashMap::new())),
            start_times: HashMap::new(),
            current_memory: HashMap::new(),
        }
    }

    pub fn start_operation(&mut self, operation: &str) {
        self.start_times
            .insert(operation.to_string(), Instant::now());
        self.current_memory
            .entry(operation.to_string())
            .or_insert_with(Vec::new);
    }

    pub fn end_operation(&mut self, operation: &str, memory_usage: f64) {
        if let Some(start_time) = self.start_times.remove(operation) {
            let duration = start_time.elapsed();

            // Enregistrer l'utilisation mémoire
            if let Some(memory_samples) = self.current_memory.get_mut(operation) {
                memory_samples.push(memory_usage);
            }

            // Mettre à jour les statistiques
            let mut stats = self.stats.lock().unwrap();
            let entry = stats
                .entry(operation.to_string())
                .or_insert(ProfilingStats {
                    total_time: Duration::from_secs(0),
                    call_count: 0,
                    avg_memory_usage: 0.0,
                    peak_memory_usage: 0.0,
                });

            entry.total_time += duration;
            entry.call_count += 1;

            // Mise à jour des statistiques mémoire
            if let Some(samples) = self.current_memory.get(operation) {
                entry.avg_memory_usage = samples.iter().sum::<f64>() / samples.len() as f64;
                entry.peak_memory_usage = samples.iter().fold(0.0, |a, &b| a.max(b));
            }
        }
    }

    pub fn get_operation_stats(&self, operation: &str) -> Option<ProfilingStats> {
        self.stats.lock().unwrap().get(operation).cloned()
    }

    pub fn get_all_stats(&self) -> HashMap<String, ProfilingStats> {
        self.stats.lock().unwrap().clone()
    }

    pub fn print_report(&self) {
        println!("\nTransformer Performance Report:");
        println!("{:-<60}", "");

        let stats = self.stats.lock().unwrap();
        for (op, stat) in stats.iter() {
            println!("Operation: {}", op);
            println!("  Total Time: {:?}", stat.total_time);
            println!("  Calls: {}", stat.call_count);
            println!("  Avg Time: {:?}", stat.total_time / stat.call_count as u32);
            println!(
                "  Avg Memory: {:.2} MB",
                stat.avg_memory_usage / 1024.0 / 1024.0
            );
            println!(
                "  Peak Memory: {:.2} MB",
                stat.peak_memory_usage / 1024.0 / 1024.0
            );
            println!("{:-<60}", "");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    #[test]
    fn test_basic_profiling() {
        let mut profiler = TransformerProfiler::new();

        profiler.start_operation("test_op");
        thread::sleep(Duration::from_millis(10));
        profiler.end_operation("test_op", 1000.0);

        let stats = profiler.get_operation_stats("test_op").unwrap();
        assert_eq!(stats.call_count, 1);
        assert!(stats.total_time >= Duration::from_millis(10));
    }

    #[test]
    fn test_multiple_operations() {
        let mut profiler = TransformerProfiler::new();

        // Premier appel
        profiler.start_operation("op1");
        thread::sleep(Duration::from_millis(10));
        profiler.end_operation("op1", 1000.0);

        // Deuxième appel
        profiler.start_operation("op1");
        thread::sleep(Duration::from_millis(10));
        profiler.end_operation("op1", 2000.0);

        let stats = profiler.get_operation_stats("op1").unwrap();
        assert_eq!(stats.call_count, 2);
        assert!(stats.peak_memory_usage == 2000.0);
        assert!(stats.avg_memory_usage == 1500.0);
    }

    #[test]
    fn test_profiling_macro() {
        let mut profiler = TransformerProfiler::new();

        profiler.start_operation("macro_test");
        let result = {
            thread::sleep(Duration::from_millis(10));
            42
        };
        profiler.end_operation("macro_test", 1000.0);

        assert_eq!(result, 42);
        let stats = profiler.get_operation_stats("macro_test").unwrap();
        assert_eq!(stats.call_count, 1);
    }
}
