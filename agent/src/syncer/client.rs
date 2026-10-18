pub struct Client {
    client_id: String,
    ref_counter: AtomicU64,
}

impl Client {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            ref_counter: AtomicU64::new(1),
        }
    }

    pub fn build_join_packet(&self) -> String {
        let topic = format!("agent:{}", self.client_id);
        json!(["1", "1", topic, "phx_join", {}]).to_string()
    }

    pub fn build_message_packet(
        &self,
        action: &str,
        code: i32,
        msg: &str,
        payload: Value,
    ) -> String {
        let topic = format!("agent:{}", self.client_id);

        let current_ref = self.ref_counter.fetch_add(1, Ordering::Relaxed).to_string();

        let envelope = json!({
            "action": action,
            "code": code,
            "msg": msg,
            "payload": payload
        });

        json!(["1", current_ref, topic, "agent:message", envelope]).to_string()
    }
}
