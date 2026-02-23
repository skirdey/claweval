use crate::backend::{AgentBackend, SendRequest, SendResponse};
use crate::spec::BackendSpec;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::time::Instant;

pub struct HttpBackend {
    pub url: String,
    pub session_field: String,
    pub message_field: String,
    pub response_field: String,
    pub headers: HashMap<String, String>,
}

impl HttpBackend {
    pub fn from_spec(spec: &BackendSpec) -> Result<Self> {
        let url = spec
            .url
            .clone()
            .ok_or_else(|| anyhow!("http backend requires backend.url"))?;
        Ok(Self {
            url,
            session_field: spec
                .session_field
                .clone()
                .unwrap_or_else(|| "session_id".to_string()),
            message_field: spec
                .message_field
                .clone()
                .unwrap_or_else(|| "message".to_string()),
            response_field: spec
                .response_field
                .clone()
                .unwrap_or_else(|| "response".to_string()),
            headers: spec.headers.clone().unwrap_or_default(),
        })
    }
}

impl AgentBackend for HttpBackend {
    fn name(&self) -> &str {
        "http"
    }

    fn send(&self, req: SendRequest) -> Result<SendResponse> {
        let start = Instant::now();

        let body = serde_json::json!({
            self.session_field.as_str(): req.session_id,
            self.message_field.as_str(): req.message,
        });

        let mut request = ureq::post(&self.url);
        for (k, v) in &self.headers {
            request = request.set(k, v);
        }

        let resp_str = match request.send_json(&body) {
            Ok(resp) => resp
                .into_string()
                .with_context(|| "failed to read HTTP response body")?,
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                return Err(anyhow!(
                    "HTTP backend returned status {} from {}: {}",
                    code,
                    self.url,
                    body
                ));
            }
            Err(e) => {
                return Err(anyhow!("HTTP request to {} failed: {}", self.url, e));
            }
        };

        let duration = start.elapsed();

        let json: serde_json::Value = serde_json::from_str(&resp_str)
            .with_context(|| format!("HTTP backend response is not valid JSON: {}", resp_str))?;

        let output_text = json
            .get(&self.response_field)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        Ok(SendResponse {
            output_text,
            raw_stdout: resp_str,
            raw_stderr: String::new(),
            json: Some(json),
            duration,
            exit_code: None,
        })
    }
}
