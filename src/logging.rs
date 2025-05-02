use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming, opt_format};

pub fn setup_logging() {
    Logger::try_with_env_or_str("debug")  // Use the log level from the environment or fallback to "info"
        .unwrap()
        .log_to_file(FileSpec::default().directory("/var/log/takeiteasy/"))
        .format(opt_format)  // Optional: Custom format for logs
        .rotate(
            Criterion::Size(10 * 1024 * 1024), // Rotate logs after they reach 10 MB
            Naming::Numbers,  // Name log files with numbers (e.g., log.1, log.2)
            Cleanup::KeepLogFiles(1),  // Keep the last 7 log files
        )
        .start()
        .unwrap();
}