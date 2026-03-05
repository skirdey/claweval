pub mod command;
pub mod http;
pub mod openai;
pub mod openclaw;

use crate::spec::BackendSpec;
use crate::types::BackendType;
use anyhow::Result;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SendRequest {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SendResponse {
    pub output_text: String,
    pub raw_stdout: String,
    pub raw_stderr: String,
    pub json: Option<Value>,
    pub duration: Duration,
    pub exit_code: Option<i32>,
}

pub trait AgentBackend: Send + Sync {
    fn backend_type(&self) -> BackendType;
    fn send(&self, req: SendRequest) -> Result<SendResponse>;

    /// Optional: create a fresh session id (backend may have its own format).
    fn new_session_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

pub fn build_backend(spec: &BackendSpec) -> Result<Box<dyn AgentBackend>> {
    match spec.backend_type {
        BackendType::OpenClaw => Ok(Box::new(openclaw::OpenClawBackend::from_spec(spec)?)),
        BackendType::Command => Ok(Box::new(command::CommandBackend::from_spec(spec)?)),
        BackendType::Http => Ok(Box::new(http::HttpBackend::from_spec(spec)?)),
        BackendType::OpenAI => Ok(Box::new(openai::OpenAIBackend::from_spec(spec)?)),
    }
}
