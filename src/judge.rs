use crate::backend::{AgentBackend, SendRequest};
use crate::util::parse_embedded_json;
use anyhow::{anyhow, Result};
use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize)]
pub struct JudgeResult {
    pub pass: bool,
    pub score: f64,
    pub notes: String,
}

fn parse_judge_json(text: &str) -> Option<JudgeResult> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }

    parse_embedded_json(s).and_then(|v| from_value(&v))
}

fn from_value(v: &Value) -> Option<JudgeResult> {
    let obj = v.as_object()?;
    let pass = obj.get("pass")?.as_bool()?;
    let score = obj
        .get("score")
        .and_then(|x| x.as_f64())
        .unwrap_or(if pass { 1.0 } else { 0.0 });
    let notes = obj
        .get("notes")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    Some(JudgeResult { pass, score, notes })
}

fn parse_pass_fail(text: &str) -> Option<JudgeResult> {
    let s = text.trim().to_uppercase();
    if s == "PASS" {
        return Some(JudgeResult {
            pass: true,
            score: 1.0,
            notes: "PASS token".to_string(),
        });
    }
    if s == "FAIL" {
        return Some(JudgeResult {
            pass: false,
            score: 0.0,
            notes: "FAIL token".to_string(),
        });
    }
    None
}

pub fn run_llm_judge(
    backend: &dyn AgentBackend,
    rubric: &str,
    candidate: &str,
    reference: Option<&str>,
) -> Result<JudgeResult> {
    let session_id = backend.new_session_id();

    let mut prompt = String::new();
    prompt.push_str("You are a strict evaluation judge for an assistant.\n");
    prompt.push_str("Return ONLY valid JSON with keys: pass (boolean), score (0..1), notes (string).\n");
    prompt.push_str("Do not include any additional keys. Do not wrap in markdown.\n\n");

    prompt.push_str("RUBRIC:\n");
    prompt.push_str(rubric);
    prompt.push_str("\n\nCANDIDATE_RESPONSE:\n");
    prompt.push_str(candidate);
    prompt.push_str("\n\n");

    if let Some(r) = reference {
        prompt.push_str("REFERENCE (if any):\n");
        prompt.push_str(r);
        prompt.push_str("\n\n");
    }

    prompt.push_str("Now output the JSON judgement.");

    let resp = backend.send(SendRequest {
        session_id,
        message: prompt,
    })?;

    // Prefer parsing judge JSON from response.json if it is exactly our expected shape.
    if let Some(v) = resp.json.as_ref() {
        if let Some(j) = from_value(v) {
            return Ok(j);
        }
    }

    if let Some(j) = parse_judge_json(&resp.output_text) {
        return Ok(j);
    }
    if let Some(j) = parse_pass_fail(&resp.output_text) {
        return Ok(j);
    }

    Err(anyhow!(
        "judge returned unparseable output. raw: {}",
        resp.output_text
    ))
}
