use crate::backend::{AgentBackend, SendRequest, SendResponse};
use crate::spec::BackendSpec;
use crate::types::BackendType;
use crate::util::parse_embedded_json;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CommandBackend {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub json: bool,
}

impl CommandBackend {
    pub fn from_spec(spec: &BackendSpec) -> Result<Self> {
        let command = spec
            .command
            .clone()
            .ok_or_else(|| anyhow!("command backend requires backend.command"))?;
        let args = spec.args.clone().unwrap_or_default();
        Ok(Self {
            command,
            args,
            env: spec.env.clone().unwrap_or_default(),
            json: spec.json.unwrap_or(false),
        })
    }

    fn substitute(s: &str, session_id: &str, message: &str) -> String {
        s.replace("{session}", session_id)
            .replace("{message}", message)
    }

    fn parse_json(stdout: &str) -> Option<serde_json::Value> {
        parse_embedded_json(stdout)
    }
}

impl AgentBackend for CommandBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Command
    }

    fn send(&self, req: SendRequest) -> Result<SendResponse> {
        let start = Instant::now();

        let mut cmd = Command::new(&self.command);

        for a in &self.args {
            cmd.arg(Self::substitute(a, &req.session_id, &req.message));
        }
        for (k, v) in &self.env {
            cmd.env(k, v);
        }

        let output = cmd
            .output()
            .with_context(|| format!("failed to spawn command backend: {:?}", cmd))?;

        let duration = start.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();

        if !output.status.success() {
            return Err(anyhow!(
                "command backend exited with {:?}. stderr: {}",
                exit_code,
                stderr
            ));
        }

        let json = if self.json {
            Self::parse_json(&stdout)
        } else {
            None
        };

        Ok(SendResponse {
            output_text: stdout.trim().to_string(),
            raw_stdout: stdout,
            raw_stderr: stderr,
            json,
            duration,
            exit_code,
        })
    }

    fn new_session_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}
