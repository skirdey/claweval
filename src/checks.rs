use crate::backend::AgentBackend;
use crate::judge;
use crate::jsonschema;
use crate::runner::StepOutcome;
use crate::spec::CheckSpec;
use anyhow::{anyhow, Result};
use regex::Regex;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckOutcome {
    pub check_type: String,
    pub step: Option<usize>,
    pub pass: bool,
    pub score: f64,
    pub details: String,
}

fn resolve_step_index(requested: Option<usize>, steps: &[StepOutcome]) -> Result<usize> {
    if steps.is_empty() {
        return Err(anyhow!("no steps were executed"));
    }
    let idx = requested.unwrap_or_else(|| steps.len() - 1);
    if idx >= steps.len() {
        return Err(anyhow!(
            "check references step {}, but only {} steps exist",
            idx,
            steps.len()
        ));
    }
    Ok(idx)
}

fn step_output_text(steps: &[StepOutcome], idx: usize) -> String {
    steps
        .get(idx)
        .and_then(|s| s.response.as_ref())
        .map(|r| r.output_text.clone())
        .unwrap_or_default()
}

fn step_json<'a>(steps: &'a [StepOutcome], idx: usize) -> Option<&'a serde_json::Value> {
    steps
        .get(idx)
        .and_then(|s| s.response.as_ref())
        .and_then(|r| r.json.as_ref())
}

fn step_duration_ms(steps: &[StepOutcome], idx: usize) -> u128 {
    steps
        .get(idx)
        .map(|s| s.duration.as_millis())
        .unwrap_or(0)
}

pub fn eval_check(
    check: &CheckSpec,
    steps: &[StepOutcome],
    llm_judge_enabled: bool,
    judge_backend: Option<&dyn AgentBackend>,
) -> Result<CheckOutcome> {
    match check {
        CheckSpec::Contains {
            step,
            text,
            case_sensitive,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let mut hay = step_output_text(steps, idx);
            let mut needle = text.clone();
            let cs = case_sensitive.unwrap_or(false);
            if !cs {
                hay = hay.to_lowercase();
                needle = needle.to_lowercase();
            }
            let pass = hay.contains(&needle);
            Ok(CheckOutcome {
                check_type: "contains".to_string(),
                step: Some(idx),
                pass,
                score: if pass { 1.0 } else { 0.0 },
                details: if pass {
                    format!("found substring '{}'", text)
                } else {
                    format!(
                        "missing substring '{}'. output was: {}",
                        text,
                        step_output_text(steps, idx)
                    )
                },
            })
        }

        CheckSpec::NotContains {
            step,
            text,
            case_sensitive,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let mut hay = step_output_text(steps, idx);
            let mut needle = text.clone();
            let cs = case_sensitive.unwrap_or(false);
            if !cs {
                hay = hay.to_lowercase();
                needle = needle.to_lowercase();
            }
            let found = hay.contains(&needle);
            let pass = !found;
            Ok(CheckOutcome {
                check_type: "not_contains".to_string(),
                step: Some(idx),
                pass,
                score: if pass { 1.0 } else { 0.0 },
                details: if pass {
                    format!("correctly absent substring '{}'", text)
                } else {
                    format!(
                        "forbidden substring '{}' was found. output was: {}",
                        text,
                        step_output_text(steps, idx)
                    )
                },
            })
        }

        CheckSpec::NotEmpty { step } => {
            let idx = resolve_step_index(*step, steps)?;
            let out = step_output_text(steps, idx);
            let pass = !out.trim().is_empty();
            Ok(CheckOutcome {
                check_type: "not_empty".to_string(),
                step: Some(idx),
                pass,
                score: if pass { 1.0 } else { 0.0 },
                details: if pass {
                    "output is non-empty".to_string()
                } else {
                    "output is empty or whitespace-only".to_string()
                },
            })
        }

        CheckSpec::Regex { step, pattern } => {
            let idx = resolve_step_index(*step, steps)?;
            let out = step_output_text(steps, idx);
            let re = Regex::new(pattern)?;
            let pass = re.is_match(&out);
            Ok(CheckOutcome {
                check_type: "regex".to_string(),
                step: Some(idx),
                pass,
                score: if pass { 1.0 } else { 0.0 },
                details: if pass {
                    format!("matched regex {}", pattern)
                } else {
                    format!("did not match regex {}. output was: {}", pattern, out)
                },
            })
        }

        CheckSpec::EqualsTrim {
            step,
            text,
            case_sensitive,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let mut out = step_output_text(steps, idx).trim().to_string();
            let mut expected = text.trim().to_string();
            let cs = case_sensitive.unwrap_or(true);
            if !cs {
                out = out.to_lowercase();
                expected = expected.to_lowercase();
            }
            let pass = out == expected;
            Ok(CheckOutcome {
                check_type: "equals_trim".to_string(),
                step: Some(idx),
                pass,
                score: if pass { 1.0 } else { 0.0 },
                details: if pass {
                    "exact match".to_string()
                } else {
                    format!("expected '{}', got '{}'", expected, out)
                },
            })
        }

        CheckSpec::JsonPointerEquals {
            step,
            pointer,
            expected,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let v = step_json(steps, idx).ok_or_else(|| {
                anyhow!(
                    "no JSON available for step {}. Enable backend.json (OpenClaw --json) or use a backend that returns json",
                    idx
                )
            })?;
            let got = v.pointer(pointer);
            let pass = got == Some(expected);
            Ok(CheckOutcome {
                check_type: "json_pointer_equals".to_string(),
                step: Some(idx),
                pass,
                score: if pass { 1.0 } else { 0.0 },
                details: if pass {
                    format!("pointer {} matched", pointer)
                } else {
                    format!("pointer {} expected {}, got {:?}", pointer, expected, got)
                },
            })
        }

        CheckSpec::JsonSchema { step, schema } => {
            let idx = resolve_step_index(*step, steps)?;
            let out = step_output_text(steps, idx);
            let instance: serde_json::Value = match serde_json::from_str(&out) {
                Ok(v) => v,
                Err(e) => {
                    return Ok(CheckOutcome {
                        check_type: "json_schema".to_string(),
                        step: Some(idx),
                        pass: false,
                        score: 0.0,
                        details: format!("output is not valid JSON: {}", e),
                    });
                }
            };
            match jsonschema::validate(schema, &instance) {
                Ok(()) => Ok(CheckOutcome {
                    check_type: "json_schema".to_string(),
                    step: Some(idx),
                    pass: true,
                    score: 1.0,
                    details: "JSON schema validation passed".to_string(),
                }),
                Err(errors) => Ok(CheckOutcome {
                    check_type: "json_schema".to_string(),
                    step: Some(idx),
                    pass: false,
                    score: 0.0,
                    details: format!("schema validation failed: {}", errors.join("; ")),
                }),
            }
        }

        CheckSpec::LatencyUnderMs { step, max_ms } => {
            let idx = resolve_step_index(*step, steps)?;
            let dur = step_duration_ms(steps, idx);
            let pass = dur <= *max_ms;
            Ok(CheckOutcome {
                check_type: "latency_under_ms".to_string(),
                step: Some(idx),
                pass,
                score: if pass { 1.0 } else { 0.0 },
                details: if pass {
                    format!("{}ms <= {}ms", dur, max_ms)
                } else {
                    format!("{}ms > {}ms", dur, max_ms)
                },
            })
        }

        CheckSpec::LlmJudge {
            step,
            rubric,
            reference,
            min_score,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            if !llm_judge_enabled {
                return Ok(CheckOutcome {
                    check_type: "llm_judge".to_string(),
                    step: Some(idx),
                    pass: true,
                    score: 1.0,
                    details: "LLM judge disabled; treated as pass".to_string(),
                });
            }
            let backend = judge_backend.ok_or_else(|| anyhow!("no judge backend configured"))?;
            let candidate = step_output_text(steps, idx);
            let jr = judge::run_llm_judge(backend, rubric, &candidate, reference.as_deref())?;
            let threshold = min_score.unwrap_or(0.5);
            let pass = jr.pass && jr.score >= threshold;
            Ok(CheckOutcome {
                check_type: "llm_judge".to_string(),
                step: Some(idx),
                pass,
                score: jr.score,
                details: format!(
                    "judge pass={} score={} threshold={} notes={}",
                    jr.pass, jr.score, threshold, jr.notes
                ),
            })
        }
    }
}
