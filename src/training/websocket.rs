use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};

pub async fn reconnect_websocket(
    listener: &TcpListener,
) -> Option<SplitSink<WebSocketStream<tokio::net::TcpStream>, Message>> {
    match listener.accept().await {
        Ok((stream, _)) => {
            let ws_stream = accept_async(stream)
                .await
                .expect("Failed to accept WebSocket");
            let (write, _) = ws_stream.split();
            Some(write)
        }
        Err(e) => {
            log::error!("Error while reconnecting WebSocket: {:?}", e);
            None
        }
    }
}
/// Envoie un message via WebSocket avec gestion d'erreur
pub async fn send_websocket_message(
    write: &mut SplitSink<WebSocketStream<tokio::net::TcpStream>, Message>,
    message: String,
    listener: &TcpListener,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = write.send(Message::Text(message.clone())).await {
        log::error!("WebSocket error: {:?}. Attempting to reconnect...", e);

        if let Some(new_write) = reconnect_websocket(listener).await {
            *write = new_write;
            write.send(Message::Text(message)).await?;
        } else {
            return Err("Failed to reconnect WebSocket".into());
        }
    }
    Ok(())
}
