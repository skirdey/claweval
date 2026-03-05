use crate::backend::{AgentBackend, SendRequest, SendResponse};
use crate::spec::BackendSpec;
use crate::types::BackendType;
use anyhow::{anyhow, Context, Result};
use crate::util::parse_embedded_json;
use serde_json::Value;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct OpenClawBackend {
    pub openclaw_bin: String,
    pub local: bool,
    pub profile: Option<String>,
    pub global_args: Vec<String>,
    pub json: bool,
}

impl OpenClawBackend {
    pub fn from_spec(spec: &BackendSpec) -> Result<Self> {
        Ok(Self {
            openclaw_bin: spec.openclaw_bin.clone().unwrap_or_else(|| "openclaw".to_string()),
            local: spec.local.unwrap_or(false),
            profile: spec.profile.clone(),
            global_args: spec.global_args.clone().unwrap_or_default(),
            json: spec.json.unwrap_or(true),
        })
    }

    fn parse_json(stdout: &str) -> Option<Value> {
        parse_embedded_json(stdout)
    }

    fn extract_text(v: &Value) -> Option<String> {
        // Common shapes we might see. OpenClaw's exact JSON schema can vary by version.
        // We avoid hard-coding and use a tolerant heuristic.
        if let Some(s) = v.as_str() {
            return Some(s.to_string());
        }

        if let Some(obj) = v.as_object() {
            let keys = [
                "assistant",
                "output",
                "content",
                "text",
                "response",
                "message",
                "result",
            ];
            for k in keys {
                if let Some(val) = obj.get(k) {
                    if let Some(s) = val.as_str() {
                        return Some(s.to_string());
                    }
                    // nested common pattern: { assistant: { content: "..." } }
                    if let Some(nobj) = val.as_object() {
                        for nk in ["content", "text", "message"] {
                            if let Some(s) = nobj.get(nk).and_then(|x| x.as_str()) {
                                return Some(s.to_string());
                            }
                        }
                    }
                }
            }
        }

        if let Some(arr) = v.as_array() {
            // Try last element
            if let Some(last) = arr.last() {
                return Self::extract_text(last);
            }
        }

        None
    }
}

impl AgentBackend for OpenClawBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::OpenClaw
    }

    fn send(&self, req: SendRequest) -> Result<SendResponse> {
        let start = Instant::now();

        let mut cmd = Command::new(&self.openclaw_bin);

        // Global flags
        if let Some(profile) = &self.profile {
            cmd.arg("--profile").arg(profile);
        }
        for a in &self.global_args {
            cmd.arg(a);
        }

        // Subcommand
        cmd.arg("agent");

        // Agent flags
        if self.local {
            cmd.arg("--local");
        }
        if self.json {
            cmd.arg("--json");
        }
        cmd.arg("--session-id").arg(&req.session_id);
        cmd.arg(&req.message);

        let output = cmd
            .output()
            .with_context(|| format!("failed to spawn openclaw command: {:?}", cmd))?;

        let duration = start.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();

        if !output.status.success() {
            return Err(anyhow!(
                "openclaw exited with {:?}. stderr: {}",
                exit_code,
                stderr
            ));
        }

        let json = if self.json {
            Self::parse_json(&stdout)
        } else {
            None
        };
        let output_text = if let Some(v) = &json {
            Self::extract_text(v).unwrap_or_else(|| stdout.trim().to_string())
        } else {
            stdout.trim().to_string()
        };

        Ok(SendResponse {
            output_text,
            raw_stdout: stdout,
            raw_stderr: stderr,
            json,
            duration,
            exit_code,
        })
    }

    fn new_session_id(&self) -> String {
        // OpenClaw supports arbitrary session-id strings; we use UUIDs.
        uuid::Uuid::new_v4().to_string()
    }
}
