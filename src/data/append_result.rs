use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use chrono::Utc;
use futures_util::stream::SplitSink;
use futures_util::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};

pub fn append_to_results_file(file_path: &str, avg_score: f64) {
    let timestamp = Utc::now().to_rfc3339();
    let result_line = format!("{},{:.2}\n", timestamp, avg_score);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .expect("Unable to open results file");
    let mut writer = BufWriter::new(file);
    writer
        .write_all(result_line.as_bytes())
        .expect("Unable to write to results file");
}
