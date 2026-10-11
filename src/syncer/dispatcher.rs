use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Frame {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Heartbeat,
    Ack,
    TrafficReport,
    NodeStats,
    UserUpsert,
    UserDelete,
    ConfigSync,
    NodeCommand,
}

pub type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Frame>>>>;

pub struct Dispatcher {
    pub token: CancellationToken,
    pub pending_map: PendingMap,
    pub tx: mpsc::Sender<Frame>,
}

impl Dispatcher {
    pub fn new(token: CancellationToken, pending_map: PendingMap, tx: mpsc::Sender<Frame>) -> Self {
        Self {
            token,
            pending_map,
            tx,
        }
    }

    pub async fn dispatch(&self, frame: Frame) {
        if frame.action == "ack" {
            if let Some(req_id) = &frame.req_id {
                let mut map = self.pending_map.lock().await;
                if let Some(sender) = map.remove(req_id) {
                    let _ = sender.send(frame);
                }
            }
            return;
        }

        match frame.action.as_str() {
            "server_command" => {
                self.handle_server_command(frame).await;
            }
            other => {
                tracing::info!(action = %other, "Received other server action");
            }
        }
    }

    async fn handle_server_command(&self, frame: Frame) {
        tracing::info!(
            payload = ?frame.payload,
            "Received server command"
        );
        let ack_frame = Frame {
            action: "ack".to_string(),
            req_id: frame.req_id,
            code: Some("Ok".to_string()),
            message: None,
            payload: Some(serde_json::json!({ "code": 200 })),
        };
        let _ = self.tx.send(ack_frame).await;
    }
}
