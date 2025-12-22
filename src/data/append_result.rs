use chrono::Utc;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};

pub fn append_to_results_file(file_path: &str, avg_score: f64) -> std::io::Result<()> {
    let timestamp = Utc::now().to_rfc3339();
    let result_line = format!("{},{:.2}\n", timestamp, avg_score);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(result_line.as_bytes())?;
    Ok(())
}
