use crate::backend::{AgentBackend, SendRequest, SendResponse};
use crate::spec::BackendSpec;
use crate::types::BackendType;
use crate::util::{read_ureq_response, UreqRead};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::time::Instant;

pub struct HttpBackend {
    pub url: String,
    pub session_field: String,
    pub message_field: String,
    pub response_field: String,
    pub headers: HashMap<String, String>,
    /// Auth token, if acquired. Also available as {{auth_token}} variable.
    pub auth_token: Option<String>,
}

impl HttpBackend {
    pub fn from_spec(spec: &BackendSpec) -> Result<Self> {
        let url = spec
            .url
            .clone()
            .ok_or_else(|| anyhow!("http backend requires backend.url"))?;
        let mut headers = spec.headers.clone().unwrap_or_default();
        let mut auth_token = None;

        // Acquire auth token if configured.
        if let Some(auth) = &spec.auth {
            let resp = ureq::post(&auth.token_url)
                .set("Content-Type", "application/json")
                .send_string(&auth.body.to_string())
                .map_err(|e| anyhow!("auth token request to {} failed: {}", auth.token_url, e))?;

            let body = resp.into_string()
                .context("failed to read auth token response body")?;
            let json: serde_json::Value = serde_json::from_str(&body)
                .with_context(|| format!("auth token response is not valid JSON: {}", body))?;

            let token = json.pointer(&auth.token_pointer)
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!(
                    "auth token not found at pointer '{}' in response: {}",
                    auth.token_pointer,
                    body
                ))?
                .to_string();

            let header_name = auth.header_name.as_deref().unwrap_or("Authorization");
            let header_prefix = auth.header_prefix.as_deref().unwrap_or("Bearer ");
            headers.insert(
                header_name.to_string(),
                format!("{}{}", header_prefix, token),
            );
            auth_token = Some(token);
        }

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
            headers,
            auth_token,
        })
    }
}

impl AgentBackend for HttpBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Http
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

        let resp_str = match read_ureq_response("HTTP backend", &self.url, request.send_json(&body))? {
            UreqRead::Ok(u) => u.body,
            UreqRead::ErrorStatus(u) => return Err(anyhow!(
                "HTTP backend returned status {} from {}: {}",
                u.status,
                self.url,
                u.body
            )),
        };

        let duration = start.elapsed();

        let json: serde_json::Value = serde_json::from_str(&resp_str)
            .with_context(|| format!("HTTP backend response is not valid JSON: {}", resp_str))?;

        let output_text = json
            .get(&self.response_field)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        // Try to parse the agent's output_text as JSON so that json_pointer
        // checks operate on the agent's response rather than the HTTP wrapper.
        let agent_json = serde_json::from_str::<serde_json::Value>(&output_text).ok();

        Ok(SendResponse {
            output_text,
            raw_stdout: resp_str,
            raw_stderr: String::new(),
            json: agent_json.or(Some(json)),
            duration,
            exit_code: None,
        })
    }
}
