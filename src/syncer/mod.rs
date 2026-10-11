mod dispatcher;

use crate::config::Config;
use dispatcher::{Dispatcher, Frame, PendingMap};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::{
    connect_async, tungstenite::client::IntoClientRequest, tungstenite::protocol::Message,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run(token: CancellationToken, cfg: Config) {
    tracing::info!("Background WebSocket service started...");

    let connect_url = format!(
        "{}?client_id={}&client_secret={}",
        cfg.web_ws_url, cfg.web_client_id, cfg.web_client_secret
    );

    let pending_requests: PendingMap = Arc::new(Mutex::new(HashMap::new()));

    let mut retry_delay = Duration::from_secs(1);
    let max_retry_delay = Duration::from_secs(30);

    loop {
        if token.is_cancelled() {
            break;
        }

        tracing::info!(connect_url = %connect_url, "Connecting to WebSocket server");

        let request = match connect_url.clone().into_client_request() {
            Ok(req) => req,
            Err(e) => {
                tracing::error!(error = %e, "Failed to build request");
                tokio::select! {
                    _ = token.cancelled() => break,
                    _ = tokio::time::sleep(retry_delay) => continue,
                }
            }
        };

        let stream = tokio::select! {
            _ = token.cancelled() => {
                tracing::info!("Connection process globally cancelled.");
                break;
            }
            res = connect_async(request) => {
                match res {
                    Ok((stream, _res)) => {
                        tracing::info!("Connected successfully!");
                        stream
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            retry_sec = retry_delay.as_secs(),
                            "Connection failed, applying exponential backoff"
                        );

                        let current_delay = retry_delay;
                        retry_delay = std::cmp::min(retry_delay * 2, max_retry_delay);

                        tokio::select! {
                            _ = token.cancelled() => break,
                            _ = tokio::time::sleep(current_delay) => continue,
                        }
                    }
                }
            }
        };

        let connected_instant = tokio::time::Instant::now();

        let sess_token = token.child_token();
        let sess_writer = sess_token.clone();
        let sess_reader = sess_token.clone();
        let sess_heartbeat = sess_token.clone();
        let sess_rpc = sess_token.clone();
        let sess_dispatcher = sess_token.clone();

        let _guard = sess_token.drop_guard();

        let (write, read) = stream.split();
        let (tx, rx) = mpsc::channel::<Frame>(100);

        let dispatcher = Dispatcher::new(sess_dispatcher, pending_requests.clone(), tx.clone());

        let mut writer_task = tokio::spawn(run_writer(sess_writer, rx, write));
        let mut reader_task = tokio::spawn(run_reader(sess_reader, read, dispatcher));
        let heartbeat_task = tokio::spawn(run_heartbeat(sess_heartbeat, tx.clone()));
        let rpc_task = tokio::spawn(run_rpc(sess_rpc, tx.clone(), pending_requests.clone()));

        let connection_broken = tokio::select! {
            _ = token.cancelled() => {
                false
            }
            _ = &mut writer_task => {
                tracing::warn!("Writer task exited, marking connection as broken");
               true
            }
            _ = &mut reader_task => {
                tracing::warn!("Reader task exited, marking connection as broken");
                true
            }
        };

        drop(_guard);

        let _ = tokio::join!(writer_task, reader_task, heartbeat_task, rpc_task);

        if token.is_cancelled() {
            break;
        }

        if connection_broken {
            let connection_duration = connected_instant.elapsed();
            if connection_duration < Duration::from_secs(10) {
                retry_delay = std::cmp::min(retry_delay * 2, max_retry_delay);
                tracing::warn!(
                    duration_sec = connection_duration.as_secs(),
                    retry_sec = retry_delay.as_secs(),
                    "Connection unstable or dropped quickly, increasing backoff delay"
                );
            } else {
                tracing::info!(
                    duration_sec = connection_duration.as_secs(),
                    "Connection was stable, resetting retry delay to 1s"
                );
                retry_delay = Duration::from_secs(1);
            }

            let current_delay = retry_delay;
            tokio::select! {
                _ = token.cancelled() => break,
                _ = tokio::time::sleep(current_delay) => {},
            }
        }
    }

    tracing::info!("Background service has completely and safely exited.");
}

async fn run_writer<W>(sess_token: CancellationToken, mut rx: mpsc::Receiver<Frame>, mut write: W)
where
    W: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin + Send,
{
    loop {
        tokio::select! {
            _ = sess_token.cancelled() => break,
            msg = rx.recv() => {
                let frame = match msg {
                    Some(f) => f,
                    None => break,
                };

                let frame_str = match serde_json::to_string(&frame) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(error = ?e, "Failed to serialize WebSocket frame");
                        continue;
                    }
                };

                if let Err(e) = write.send(Message::Text(frame_str.into())).await {
                    tracing::error!(error = ?e, "Failed to send WebSocket message, closing writer task");
                    break;
                }
            }
        }
    }
}

async fn run_reader<R>(sess_token: CancellationToken, mut read: R, dispatcher: Dispatcher)
where
    R: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin
        + Send,
{
    loop {
        tokio::select! {
            _ = sess_token.cancelled() => break,
            msg = read.next() => {
                match msg {
                    Some(Ok(msg)) if msg.is_text() || msg.is_binary() => {
                        let text = msg.to_string();
                        if let Ok(frame) = serde_json::from_str::<Frame>(&text) {
                            dispatcher.dispatch(frame).await;
                        }
                    }
                    Some(Ok(msg)) if msg.is_close() => {
                        tracing::info!("Server actively closed the connection");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::error!(error = ?e, "WebSocket read error");
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
        }
    }
}

async fn run_heartbeat(sess_token: CancellationToken, tx: mpsc::Sender<Frame>) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = sess_token.cancelled() => break,
            _ = interval.tick() => {
                let heartbeat_frame = Frame {
                    action: "heartbeat".to_string(),
                    req_id: None,
                    code: None,
                    message: None,
                    payload: None,
                };

                if tx.send(heartbeat_frame).await.is_err() {
                    break;
                }
                tracing::debug!("Heartbeat ping sent");
            }
        }
    }
}

async fn run_rpc(
    sess_token: CancellationToken,
    tx: mpsc::Sender<Frame>,
    pending_requests: PendingMap,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            _ = sess_token.cancelled() => break,
            _ = interval.tick() => {
                let req_id = Uuid::new_v4().to_string();
                let (oneshot_tx, oneshot_rx) = oneshot::channel();

                {
                    let mut map = pending_requests.lock().await;
                    map.insert(req_id.clone(), oneshot_tx);
                }

                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let frame = Frame {
                    action: "report_metrics".to_string(),
                    req_id: Some(req_id.clone()),
                    code: None,
                    message: None,
                    payload: Some(serde_json::json!({
                        "status": "running",
                        "timestamp": timestamp
                    })),
                };

                if tx.send(frame).await.is_err() {
                    break;
                }

                tracing::info!(req_id = %req_id, "Sending RPC request");

                tokio::select! {
                    res = oneshot_rx => {
                        match res {
                            Ok(ack_frame) => {
                                if ack_frame.code.as_deref() == Some("Ok") {
                                    tracing::info!(req_id = %req_id, "RPC succeeded");
                                } else {
                                    tracing::error!(
                                        req_id = %req_id,
                                        code = ?ack_frame.code,
                                        message = ?ack_frame.message,
                                        "RPC business failure"
                                    );
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {
                        let mut map = pending_requests.lock().await;
                        map.remove(&req_id);
                        tracing::warn!(req_id = %req_id, "RPC timeout: no response received from server within 3 seconds");
                    }
                }
            }
        }
    }
}
