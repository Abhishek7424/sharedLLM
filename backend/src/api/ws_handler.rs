use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use crate::AppState;

/// GET /ws  — upgrade to WebSocket
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.event_tx.subscribe();

    // Channel used by recv_task to forward Pong payloads to send_task
    let (pong_tx, mut pong_rx) = mpsc::channel::<Vec<u8>>(8);

    // Task: forward broadcast events → WebSocket client; also send Pongs
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Ok(event) => {
                            if let Ok(text) = serde_json::to_string(&event) {
                                if sender.send(Message::Text(text)).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
                pong_data = pong_rx.recv() => {
                    match pong_data {
                        Some(data) => {
                            if sender.send(Message::Pong(data)).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    // Task: receive messages from client and handle Ping → Pong
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(data)) => {
                    if pong_tx.send(data).await.is_err() {
                        break;
                    }
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    tracing::debug!("WebSocket client disconnected");
}
