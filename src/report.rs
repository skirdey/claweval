use crate::checks::CheckOutcome;
use crate::stats::Rate;
use crate::types::{BackendType, StepKind};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct BackendInfo {
    pub backend_type: BackendType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuiteReport {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scoring_class: Option<String>,
    pub backend: BackendInfo,
    pub started_at_unix_ms: u128,
    pub duration_ms: u128,
    pub overall: OverallSummary,
    pub episodes: Vec<EpisodeReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallSummary {
    pub total_runs: u32,
    pub passed_runs: u32,
    pub pass_rate: Rate,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpisodeReport {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub repeats: u32,
    pub summary: EpisodeSummary,
    pub runs: Vec<EpisodeRunReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpisodeSummary {
    pub total_runs: u32,
    pub passed_runs: u32,
    pub pass_rate: Rate,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpisodeRunReport {
    pub run_index: u32,
    pub session_id: String,
    pub pass: bool,
    pub duration_ms: u128,
    pub steps: Vec<StepReport>,
    pub checks: Vec<CheckOutcome>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepReport {
    pub index: usize,
    pub kind: StepKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_text: Option<String>,
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_kind_details: Option<StepKindDetails>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepKindDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_attempts: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_satisfied: Option<bool>,
}

pub fn dur_ms(d: Duration) -> u128 {
    d.as_millis()
}
