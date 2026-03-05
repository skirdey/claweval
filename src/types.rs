use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// BackendType — replaces stringly-typed backend dispatch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    #[serde(rename = "openclaw")]
    OpenClaw,
    #[serde(rename = "command")]
    Command,
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "openai")]
    OpenAI,
}

impl BackendType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenClaw => "openclaw",
            Self::Command => "command",
            Self::Http => "http",
            Self::OpenAI => "openai",
        }
    }
}

impl fmt::Display for BackendType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for BackendType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "openclaw" => Ok(Self::OpenClaw),
            "command" => Ok(Self::Command),
            "http" => Ok(Self::Http),
            "openai" => Ok(Self::OpenAI),
            other => Err(format!(
                "unknown backend type '{}'. supported: openclaw|command|http|openai",
                other
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// HttpMethod — replaces stringly-typed HTTP method
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
    #[serde(rename = "PUT")]
    Put,
    #[serde(rename = "DELETE")]
    Delete,
    #[serde(rename = "PATCH")]
    Patch,
    #[serde(rename = "HEAD")]
    Head,
    #[serde(rename = "OPTIONS")]
    Options,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl Default for HttpMethod {
    fn default() -> Self {
        Self::Get
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// StepKind — replaces stringly-typed step kind on StepOutcome / StepReport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "sleep")]
    Sleep,
    #[serde(rename = "note")]
    Note,
    #[serde(rename = "exec")]
    Exec,
    #[serde(rename = "http_probe")]
    HttpProbe,
    #[serde(rename = "poll")]
    Poll,
    #[serde(rename = "set_var")]
    SetVar,
    #[serde(rename = "webhook_listen")]
    WebhookListen,
    #[serde(rename = "sse_subscribe")]
    SseSubscribe,
    #[serde(rename = "parallel")]
    Parallel,
}

impl StepKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Sleep => "sleep",
            Self::Note => "note",
            Self::Exec => "exec",
            Self::HttpProbe => "http_probe",
            Self::Poll => "poll",
            Self::SetVar => "set_var",
            Self::WebhookListen => "webhook_listen",
            Self::SseSubscribe => "sse_subscribe",
            Self::Parallel => "parallel",
        }
    }
}

impl fmt::Display for StepKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// CheckType — replaces stringly-typed check_type on CheckOutcome
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckType {
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "not_contains")]
    NotContains,
    #[serde(rename = "not_empty")]
    NotEmpty,
    #[serde(rename = "regex")]
    Regex,
    #[serde(rename = "equals_trim")]
    EqualsTrim,
    #[serde(rename = "json_pointer_equals")]
    JsonPointerEquals,
    #[serde(rename = "json_schema")]
    JsonSchema,
    #[serde(rename = "latency_under_ms")]
    LatencyUnderMs,
    #[serde(rename = "llm_judge")]
    LlmJudge,
    #[serde(rename = "eventually_contains")]
    EventuallyContains,
    #[serde(rename = "status_code_equals")]
    StatusCodeEquals,
    #[serde(rename = "json_pointer_exists")]
    JsonPointerExists,
    #[serde(rename = "within_time_window_ms")]
    WithinTimeWindowMs,
    #[serde(rename = "json_pointer_contains")]
    JsonPointerContains,
    #[serde(rename = "json_array_length")]
    JsonArrayLength,
    #[serde(rename = "webhook_received")]
    WebhookReceived,
    #[serde(rename = "sse_event_received")]
    SseEventReceived,
    #[serde(rename = "step_order")]
    StepOrder,
}

impl CheckType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::NotContains => "not_contains",
            Self::NotEmpty => "not_empty",
            Self::Regex => "regex",
            Self::EqualsTrim => "equals_trim",
            Self::JsonPointerEquals => "json_pointer_equals",
            Self::JsonSchema => "json_schema",
            Self::LatencyUnderMs => "latency_under_ms",
            Self::LlmJudge => "llm_judge",
            Self::EventuallyContains => "eventually_contains",
            Self::StatusCodeEquals => "status_code_equals",
            Self::JsonPointerExists => "json_pointer_exists",
            Self::WithinTimeWindowMs => "within_time_window_ms",
            Self::JsonPointerContains => "json_pointer_contains",
            Self::JsonArrayLength => "json_array_length",
            Self::WebhookReceived => "webhook_received",
            Self::SseEventReceived => "sse_event_received",
            Self::StepOrder => "step_order",
        }
    }
}

impl fmt::Display for CheckType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// JsonPointer — newtype for RFC 6901 JSON Pointer strings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonPointer(pub String);

impl JsonPointer {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JsonPointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_type_roundtrip() {
        let bt = BackendType::Http;
        let json = serde_json::to_string(&bt).unwrap();
        assert_eq!(json, "\"http\"");
        let parsed: BackendType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, BackendType::Http);
    }

    #[test]
    fn backend_type_from_str() {
        assert_eq!("openclaw".parse::<BackendType>().unwrap(), BackendType::OpenClaw);
        assert_eq!("openai".parse::<BackendType>().unwrap(), BackendType::OpenAI);
        assert!("invalid".parse::<BackendType>().is_err());
    }

    #[test]
    fn http_method_default_is_get() {
        assert_eq!(HttpMethod::default(), HttpMethod::Get);
        assert_eq!(HttpMethod::Get.as_str(), "GET");
    }

    #[test]
    fn http_method_roundtrip() {
        let m = HttpMethod::Post;
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, "\"POST\"");
        let parsed: HttpMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, HttpMethod::Post);
    }

    #[test]
    fn json_pointer_transparent_serde() {
        let ptr = JsonPointer("/foo/bar".to_string());
        let json = serde_json::to_string(&ptr).unwrap();
        assert_eq!(json, "\"/foo/bar\"");
        let parsed: JsonPointer = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_str(), "/foo/bar");
    }
}
