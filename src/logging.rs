use flexi_logger::{Logger, opt_format};

pub fn setup_logging() {
    Logger::try_with_env_or_str("info")  // Use info level by default
        .unwrap()
        .log_to_stdout()  // Log to stdout for easier debugging
        .format(opt_format)  // Optional: Custom format for logs
        .start()
        .unwrap();
}