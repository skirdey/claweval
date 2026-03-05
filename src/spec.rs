use crate::types::{BackendType, HttpMethod};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteSpec {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub capability_tags: Option<Vec<String>>,
    #[serde(default)]
    pub scoring_class: Option<String>,

    pub backend: BackendSpec,

    /// Optional separate backend used by the LLM judge. If absent, the agent
    /// backend is used as the judge (original behaviour).
    #[serde(default)]
    pub judge_backend: Option<BackendSpec>,

    /// Optional: multiplies per-episode repeats.
    #[serde(default)]
    pub global_repeats: Option<u32>,

    /// Background services to start before episodes and stop after completion.
    #[serde(default)]
    pub services: Option<Vec<ServiceSpec>>,

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

    pub fn apply_cli_overrides(&mut self, overrides: CliOverrides) {
        if let Some(r) = overrides.repeats {
            self.global_repeats = Some(r);
        }
        if let Some(bt) = overrides.backend_type {
            self.backend.backend_type = bt;
        }
        if let Some(profile) = overrides.profile {
            self.backend.profile = Some(profile);
        }
        if overrides.local {
            self.backend.local = Some(true);
        }
        if let Some(bin) = overrides.openclaw_bin {
            self.backend.openclaw_bin = Some(bin);
        }
    }
}

pub struct CliOverrides {
    pub repeats: Option<u32>,
    pub backend_type: Option<BackendType>,
    pub openclaw_bin: Option<String>,
    pub local: bool,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSpec {
    #[serde(rename = "type")]
    pub backend_type: BackendType,

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

    /// HTTP auth: acquire a token at construction time.
    #[serde(default)]
    pub auth: Option<HttpAuthSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpAuthSpec {
    /// POST endpoint to acquire a token.
    pub token_url: String,
    /// JSON body to send with the token request.
    pub body: serde_json::Value,
    /// JSON pointer to extract the token from the response.
    pub token_pointer: String,
    /// Header name to inject the token (default: "Authorization").
    #[serde(default)]
    pub header_name: Option<String>,
    /// Prefix before the token value (default: "Bearer ").
    #[serde(default)]
    pub header_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    /// Wait for this text in stdout before considering the service ready.
    #[serde(default)]
    pub ready_text: Option<String>,
    /// Max time to wait for ready_text (default: 10000ms).
    #[serde(default)]
    pub ready_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeSpec {
    pub id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub repeats: Option<u32>,
    /// Steps run before each repeat (failures abort the run).
    #[serde(default)]
    pub setup: Option<Vec<StepSpec>>,
    /// Steps run after each repeat (best-effort, failures logged).
    #[serde(default)]
    pub teardown: Option<Vec<StepSpec>>,
    /// Pre-seeded variables available as {{var_name}} from the start.
    #[serde(default)]
    pub vars: Option<HashMap<String, String>>,
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

    /// Run an external command and capture output.
    #[serde(rename = "exec")]
    Exec {
        command: String,
        #[serde(default)]
        args: Option<Vec<String>>,
        #[serde(default)]
        env: Option<HashMap<String, String>>,
        #[serde(default)]
        name: Option<String>,
    },

    /// Probe an HTTP endpoint and capture status/body.
    #[serde(rename = "http_probe")]
    HttpProbe {
        url: String,
        #[serde(default)]
        method: Option<HttpMethod>,
        #[serde(default)]
        headers: Option<HashMap<String, String>>,
        #[serde(default)]
        body: Option<serde_json::Value>,
        #[serde(default)]
        timeout_ms: Option<u64>,
        #[serde(default)]
        name: Option<String>,
    },

    /// Poll repeatedly until condition is met or timeout.
    #[serde(rename = "poll")]
    Poll {
        probe: Box<StepSpec>,
        interval_ms: u64,
        timeout_ms: u64,
        until: PollCondition,
        #[serde(default)]
        name: Option<String>,
    },

    /// Extract a value from a previous step's JSON and store as a variable.
    #[serde(rename = "set_var")]
    SetVar {
        /// Variable name to set.
        var: String,
        /// Step index to extract from (default: previous step).
        #[serde(default)]
        step: Option<usize>,
        /// JSON pointer to extract the value.
        pointer: String,
        #[serde(default)]
        name: Option<String>,
    },

    /// Listen for incoming HTTP requests (webhook callback capture).
    #[serde(rename = "webhook_listen")]
    WebhookListen {
        /// Port to listen on (0 = auto-pick free port).
        port: u16,
        /// Path filter (default: "/").
        #[serde(default)]
        path: Option<String>,
        /// How long to listen in milliseconds.
        timeout_ms: u64,
        /// Stop early after receiving this many requests.
        #[serde(default)]
        min_requests: Option<u32>,
        #[serde(default)]
        name: Option<String>,
    },

    /// Subscribe to an SSE endpoint and collect events.
    #[serde(rename = "sse_subscribe")]
    SseSubscribe {
        url: String,
        #[serde(default)]
        headers: Option<HashMap<String, String>>,
        timeout_ms: u64,
        /// Stop after N events (0 = until timeout).
        #[serde(default)]
        max_events: Option<u32>,
        /// Only collect events matching this SSE event type.
        #[serde(default)]
        event_filter: Option<String>,
        #[serde(default)]
        name: Option<String>,
    },

    /// Run multiple steps concurrently.
    #[serde(rename = "parallel")]
    Parallel {
        steps: Vec<StepSpec>,
        #[serde(default)]
        name: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PollCondition {
    #[serde(rename = "contains_text")]
    ContainsText {
        text: String,
        #[serde(default)]
        case_sensitive: Option<bool>,
    },
    #[serde(rename = "regex")]
    Regex {
        pattern: String,
    },
    #[serde(rename = "status_code")]
    StatusCode {
        code: u16,
    },
    #[serde(rename = "json_pointer_equals")]
    JsonPointerEquals {
        pointer: String,
        expected: serde_json::Value,
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
        max_ms: u64,
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

    /// Assert output eventually contains text (for polled snapshots).
    #[serde(rename = "eventually_contains")]
    EventuallyContains {
        #[serde(default)]
        step: Option<usize>,
        text: String,
        within_ms: u64,
        #[serde(default)]
        case_sensitive: Option<bool>,
    },

    /// Assert HTTP probe status code equals expected.
    #[serde(rename = "status_code_equals")]
    StatusCodeEquals {
        #[serde(default)]
        step: Option<usize>,
        code: u16,
    },

    /// Assert JSON pointer exists in parsed step JSON.
    #[serde(rename = "json_pointer_exists")]
    JsonPointerExists {
        #[serde(default)]
        step: Option<usize>,
        pointer: String,
    },

    /// Assert step runtime is within [min_ms, max_ms].
    #[serde(rename = "within_time_window_ms")]
    WithinTimeWindowMs {
        #[serde(default)]
        step: Option<usize>,
        min_ms: u64,
        max_ms: u64,
    },

    /// Assert JSON pointer's string value contains a substring.
    #[serde(rename = "json_pointer_contains")]
    JsonPointerContains {
        #[serde(default)]
        step: Option<usize>,
        pointer: String,
        text: String,
        #[serde(default)]
        case_sensitive: Option<bool>,
    },

    /// Assert JSON array at pointer has length within [min, max].
    #[serde(rename = "json_array_length")]
    JsonArrayLength {
        #[serde(default)]
        step: Option<usize>,
        pointer: String,
        #[serde(default)]
        min: Option<usize>,
        #[serde(default)]
        max: Option<usize>,
    },

    /// Assert webhook listener received requests.
    #[serde(rename = "webhook_received")]
    WebhookReceived {
        #[serde(default)]
        step: Option<usize>,
        #[serde(default)]
        min_count: Option<u32>,
        #[serde(default)]
        payload_pointer: Option<String>,
        #[serde(default)]
        payload_expected: Option<serde_json::Value>,
    },

    /// Assert SSE events were received.
    #[serde(rename = "sse_event_received")]
    SseEventReceived {
        #[serde(default)]
        step: Option<usize>,
        #[serde(default)]
        min_count: Option<u32>,
        #[serde(default)]
        data_contains: Option<String>,
        #[serde(default)]
        data_pointer: Option<String>,
        #[serde(default)]
        data_expected: Option<serde_json::Value>,
    },

    /// Assert step A finished before step B started (temporal ordering).
    #[serde(rename = "step_order")]
    StepOrder {
        before_step: usize,
        after_step: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_new_step_and_check_types() {
        let raw = r#"
        {
          "name": "t",
          "backend": {"type":"http","url":"http://localhost:5000/chat"},
          "episodes": [{
            "id":"ep",
            "steps":[
              {"type":"exec","command":"echo","args":["hi"]},
              {"type":"http_probe","url":"http://localhost:5010/health","method":"GET"},
              {
                "type":"poll",
                "probe":{"type":"http_probe","url":"http://localhost:5010/health"},
                "interval_ms":100,
                "timeout_ms":500,
                "until":{"type":"status_code","code":200}
              }
            ],
            "checks":[
              {"type":"status_code_equals","step":1,"code":200},
              {"type":"json_pointer_exists","step":1,"pointer":"/ok"},
              {"type":"within_time_window_ms","step":2,"min_ms":0,"max_ms":1000},
              {"type":"eventually_contains","step":2,"text":"ok","within_ms":1000}
            ]
          }]
        }"#;

        let s: SuiteSpec = serde_json::from_str(raw).expect("suite should parse");
        assert_eq!(s.episodes.len(), 1);
        assert_eq!(s.episodes[0].steps.len(), 3);
        assert_eq!(s.episodes[0].checks.len(), 4);
    }

    #[test]
    fn parses_advanced_step_and_check_types() {
        let raw = r#"
        {
          "name": "advanced",
          "backend": {"type":"http","url":"http://localhost:5000/chat",
                      "auth":{"token_url":"http://localhost:5000/auth","body":{"key":"val"},
                              "token_pointer":"/token"}},
          "services": [
            {"name":"mock","command":"echo","args":["ready"],"ready_text":"ready","ready_timeout_ms":5000}
          ],
          "episodes": [{
            "id":"ep_advanced",
            "vars": {"base": "http://localhost:5000"},
            "setup": [{"type":"exec","command":"echo","args":["setup"]}],
            "teardown": [{"type":"exec","command":"echo","args":["teardown"]}],
            "steps":[
              {"type":"http_probe","url":"{{base}}/api","method":"POST","body":{"msg":"hi"}},
              {"type":"set_var","var":"item_id","step":0,"pointer":"/id"},
              {"type":"webhook_listen","port":0,"path":"/callback","timeout_ms":5000,"min_requests":1},
              {"type":"sse_subscribe","url":"http://localhost:5000/events","timeout_ms":3000,
               "max_events":5,"event_filter":"message"},
              {
                "type":"parallel",
                "steps":[
                  {"type":"webhook_listen","port":9090,"timeout_ms":10000,"min_requests":1},
                  {"type":"user","input":"trigger webhook"}
                ]
              }
            ],
            "checks":[
              {"type":"json_pointer_contains","step":0,"pointer":"/message","text":"hello"},
              {"type":"json_array_length","step":0,"pointer":"/items","min":1,"max":10},
              {"type":"webhook_received","step":2,"min_count":1,
               "payload_pointer":"/status","payload_expected":"done"},
              {"type":"sse_event_received","step":3,"min_count":1,"data_contains":"update"},
              {"type":"step_order","before_step":0,"after_step":1}
            ]
          }]
        }"#;

        let s: SuiteSpec = serde_json::from_str(raw).expect("advanced suite should parse");
        assert_eq!(s.episodes.len(), 1);
        assert!(s.services.is_some());
        assert_eq!(s.services.as_ref().unwrap().len(), 1);
        assert!(s.backend.auth.is_some());

        let ep = &s.episodes[0];
        assert!(ep.setup.is_some());
        assert!(ep.teardown.is_some());
        assert!(ep.vars.is_some());
        assert_eq!(ep.steps.len(), 5);
        assert_eq!(ep.checks.len(), 5);

        // Verify set_var parses correctly
        match &ep.steps[1] {
            StepSpec::SetVar { var, step, pointer, .. } => {
                assert_eq!(var, "item_id");
                assert_eq!(*step, Some(0));
                assert_eq!(pointer, "/id");
            }
            _ => panic!("step 1 should be SetVar"),
        }

        // Verify parallel parses correctly
        match &ep.steps[4] {
            StepSpec::Parallel { steps, .. } => {
                assert_eq!(steps.len(), 2);
            }
            _ => panic!("step 4 should be Parallel"),
        }
    }

    #[test]
    fn apply_cli_overrides_mutates_spec() {
        let mut spec = SuiteSpec {
            name: "x".to_string(),
            description: None,
            capability_tags: None,
            scoring_class: None,
            backend: BackendSpec {
                backend_type: BackendType::Http,
                openclaw_bin: None,
                local: None,
                profile: None,
                global_args: None,
                json: None,
                command: None,
                args: None,
                env: None,
                url: Some("http://localhost".to_string()),
                session_field: None,
                message_field: None,
                response_field: None,
                headers: None,
                base_url: None,
                model: None,
                api_key: None,
                auth: None,
            },
            judge_backend: None,
            global_repeats: Some(1),
            services: None,
            episodes: Vec::new(),
        };

        spec.apply_cli_overrides(CliOverrides {
            repeats: Some(3),
            backend_type: Some(BackendType::OpenClaw),
            openclaw_bin: Some("/bin/echo".to_string()),
            local: true,
            profile: Some("local-test".to_string()),
        });

        assert_eq!(spec.global_repeats, Some(3));
        assert_eq!(spec.backend.backend_type, BackendType::OpenClaw);
        assert_eq!(spec.backend.openclaw_bin.as_deref(), Some("/bin/echo"));
        assert_eq!(spec.backend.local, Some(true));
        assert_eq!(spec.backend.profile.as_deref(), Some("local-test"));
    }
}
