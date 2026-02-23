use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteSpec {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,

    pub backend: BackendSpec,

    /// Optional separate backend used by the LLM judge. If absent, the agent
    /// backend is used as the judge (original behaviour).
    #[serde(default)]
    pub judge_backend: Option<BackendSpec>,

    /// Optional: multiplies per-episode repeats.
    #[serde(default)]
    pub global_repeats: Option<u32>,

    pub episodes: Vec<EpisodeSpec>,
}

impl SuiteSpec {
    pub fn from_path(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read suite file: {}", path.display()))?;
        let spec: SuiteSpec = serde_json::from_str(&data)
            .with_context(|| format!("failed to parse suite json: {}", path.display()))?;
        Ok(spec)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSpec {
    /// openclaw | command | http | openai
    #[serde(rename = "type")]
    pub backend_type: String,

    // --- openclaw backend ---

    /// OpenClaw binary path (default: openclaw)
    #[serde(default)]
    pub openclaw_bin: Option<String>,

    /// If true, pass --local to OpenClaw
    #[serde(default)]
    pub local: Option<bool>,

    /// OpenClaw profile name (isolates state)
    #[serde(default)]
    pub profile: Option<String>,

    /// Extra global args to place before subcommand (OpenClaw)
    #[serde(default)]
    pub global_args: Option<Vec<String>>,

    /// If true, request JSON output from OpenClaw (recommended)
    #[serde(default)]
    pub json: Option<bool>,

    // --- command backend ---

    /// Generic command backend: executable
    #[serde(default)]
    pub command: Option<String>,

    /// Generic command backend: args with placeholders {session} and {message}
    #[serde(default)]
    pub args: Option<Vec<String>>,

    /// Generic command backend: env vars
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,

    // --- http backend ---

    /// HTTP endpoint URL (e.g. "http://localhost:8080/chat")
    #[serde(default)]
    pub url: Option<String>,

    /// JSON field name for the session id in the request body (default: "session_id")
    #[serde(default)]
    pub session_field: Option<String>,

    /// JSON field name for the message in the request body (default: "message")
    #[serde(default)]
    pub message_field: Option<String>,

    /// JSON field name for the response text in the response body (default: "response")
    #[serde(default)]
    pub response_field: Option<String>,

    /// Extra HTTP headers
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,

    // --- openai backend ---

    /// Base URL for OpenAI-compatible API (e.g. "http://localhost:11434")
    #[serde(default)]
    pub base_url: Option<String>,

    /// Model name (e.g. "llama3.2", "claude-haiku-4-5-20251001")
    #[serde(default)]
    pub model: Option<String>,

    /// API key (default: "none")
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeSpec {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub repeats: Option<u32>,
    pub steps: Vec<StepSpec>,
    #[serde(default)]
    pub checks: Vec<CheckSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepSpec {
    /// Send a user message to the agent.
    #[serde(rename = "user")]
    User {
        input: String,
        #[serde(default)]
        name: Option<String>,
    },

    /// Sleep/wait for a duration (milliseconds).
    #[serde(rename = "sleep")]
    Sleep {
        ms: u64,
        #[serde(default)]
        name: Option<String>,
    },

    /// A no-op marker step.
    #[serde(rename = "note")]
    Note {
        text: String,
        #[serde(default)]
        name: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CheckSpec {
    /// Assert the output contains substring.
    #[serde(rename = "contains")]
    Contains {
        #[serde(default)]
        step: Option<usize>,
        text: String,
        #[serde(default)]
        case_sensitive: Option<bool>,
    },

    /// Assert the output does NOT contain substring.
    #[serde(rename = "not_contains")]
    NotContains {
        #[serde(default)]
        step: Option<usize>,
        text: String,
        #[serde(default)]
        case_sensitive: Option<bool>,
    },

    /// Assert the output is non-empty after trimming.
    #[serde(rename = "not_empty")]
    NotEmpty {
        #[serde(default)]
        step: Option<usize>,
    },

    /// Assert the output matches a regex.
    #[serde(rename = "regex")]
    Regex {
        #[serde(default)]
        step: Option<usize>,
        pattern: String,
    },

    /// Assert trimmed output equals text.
    #[serde(rename = "equals_trim")]
    EqualsTrim {
        #[serde(default)]
        step: Option<usize>,
        text: String,
        #[serde(default)]
        case_sensitive: Option<bool>,
    },

    /// Assert JSON pointer value equals expected JSON (requires backend JSON output).
    #[serde(rename = "json_pointer_equals")]
    JsonPointerEquals {
        #[serde(default)]
        step: Option<usize>,
        pointer: String,
        expected: serde_json::Value,
    },

    /// Assert step output parses as JSON and validates against inline JSON Schema (draft-7).
    #[serde(rename = "json_schema")]
    JsonSchema {
        #[serde(default)]
        step: Option<usize>,
        schema: serde_json::Value,
    },

    /// Assert step duration is <= threshold.
    #[serde(rename = "latency_under_ms")]
    LatencyUnderMs {
        #[serde(default)]
        step: Option<usize>,
        max_ms: u128,
    },

    /// LLM-as-judge rubric. Requires --enable-llm-judge in runner.
    /// The judge should respond with either:
    /// - JSON: {"pass": true/false, "score": 0.0-1.0, "notes": "..."}
    /// - or a single token PASS / FAIL
    #[serde(rename = "llm_judge")]
    LlmJudge {
        #[serde(default)]
        step: Option<usize>,
        rubric: String,
        #[serde(default)]
        reference: Option<String>,
        #[serde(default)]
        min_score: Option<f64>,
    },
}
