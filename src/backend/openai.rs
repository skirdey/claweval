use crate::backend::{AgentBackend, SendRequest, SendResponse};
use crate::spec::BackendSpec;
use crate::types::BackendType;
use crate::util::{read_ureq_response, UreqRead};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

pub struct OpenAIBackend {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    /// Per-session conversation history (user/assistant turns).
    history: Mutex<HashMap<String, Vec<serde_json::Value>>>,
}

impl OpenAIBackend {
    pub fn from_spec(spec: &BackendSpec) -> Result<Self> {
        let base_url = spec
            .base_url
            .clone()
            .ok_or_else(|| anyhow!("openai backend requires backend.base_url"))?;
        let model = spec
            .model
            .clone()
            .ok_or_else(|| anyhow!("openai backend requires backend.model"))?;
        let api_key = spec
            .api_key
            .clone()
            .unwrap_or_else(|| "none".to_string());
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            api_key,
            history: Mutex::new(HashMap::new()),
        })
    }

    fn chat_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url)
    }
}

impl AgentBackend for OpenAIBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::OpenAI
    }

    fn send(&self, req: SendRequest) -> Result<SendResponse> {
        let start = Instant::now();

        // Build messages list: fetch existing history, append user turn.
        let messages: Vec<serde_json::Value> = {
            let mut guard = self.history.lock().unwrap();
            let session_msgs = guard
                .entry(req.session_id.clone())
                .or_insert_with(Vec::new);
            session_msgs.push(serde_json::json!({
                "role": "user",
                "content": req.message,
            }));
            session_msgs.clone()
        };

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });

        let url = self.chat_url();
        let resp = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_json(&body);
        let resp_str = match read_ureq_response("OpenAI backend", &url, resp)? {
            UreqRead::Ok(u) => u.body,
            UreqRead::ErrorStatus(u) => {
                return Err(anyhow!(
                    "OpenAI backend returned status {} from {}: {}",
                    u.status,
                    url,
                    u.body
                ))
            }
        };

        let duration = start.elapsed();

        let json: serde_json::Value = serde_json::from_str(&resp_str)
            .with_context(|| format!("OpenAI response is not valid JSON: {}", resp_str))?;

        let content = json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string();

        // Append assistant response to session history.
        {
            let mut guard = self.history.lock().unwrap();
            if let Some(session_msgs) = guard.get_mut(&req.session_id) {
                session_msgs.push(serde_json::json!({
                    "role": "assistant",
                    "content": content,
                }));
            }
        }

        Ok(SendResponse {
            output_text: content,
            raw_stdout: resp_str,
            raw_stderr: String::new(),
            json: Some(json),
            duration,
            exit_code: None,
        })
    }
}
