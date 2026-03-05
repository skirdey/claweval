use crate::backend::AgentBackend;
use crate::judge;
use crate::jsonschema;
use crate::runner::StepOutcome;
use crate::spec::CheckSpec;
use crate::types::CheckType;
use anyhow::{anyhow, Result};
use regex::Regex;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckOutcome {
    pub check_type: CheckType,
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

fn mk_outcome(check_type: CheckType, idx: usize, pass: bool, details: String) -> CheckOutcome {
    CheckOutcome {
        check_type,
        step: Some(idx),
        pass,
        score: if pass { 1.0 } else { 0.0 },
        details,
    }
}

fn output_matches(text: &str, needle: &str, case_sensitive: bool) -> bool {
    let mut hay = text.to_string();
    let mut need = needle.to_string();
    if !case_sensitive {
        hay = hay.to_lowercase();
        need = need.to_lowercase();
    }
    hay.contains(&need)
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
            let out = step_output_text(steps, idx);
            let pass = output_matches(&out, text, case_sensitive.unwrap_or(false));
            Ok(mk_outcome(
                CheckType::Contains,
                idx,
                pass,
                if pass {
                    format!("found substring '{}'", text)
                } else {
                    format!("missing substring '{}'. output was: {}", text, out)
                },
            ))
        }

        CheckSpec::NotContains {
            step,
            text,
            case_sensitive,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let out = step_output_text(steps, idx);
            let found = output_matches(&out, text, case_sensitive.unwrap_or(false));
            let pass = !found;
            Ok(mk_outcome(
                CheckType::NotContains,
                idx,
                pass,
                if pass {
                    format!("correctly absent substring '{}'", text)
                } else {
                    format!("forbidden substring '{}' was found. output was: {}", text, out)
                },
            ))
        }

        CheckSpec::NotEmpty { step } => {
            let idx = resolve_step_index(*step, steps)?;
            let out = step_output_text(steps, idx);
            let pass = !out.trim().is_empty();
            Ok(mk_outcome(
                CheckType::NotEmpty,
                idx,
                pass,
                if pass {
                    "output is non-empty".to_string()
                } else {
                    "output is empty or whitespace-only".to_string()
                },
            ))
        }

        CheckSpec::Regex { step, pattern } => {
            let idx = resolve_step_index(*step, steps)?;
            let out = step_output_text(steps, idx);
            let re = Regex::new(pattern)?;
            let pass = re.is_match(&out);
            Ok(mk_outcome(
                CheckType::Regex,
                idx,
                pass,
                if pass {
                    format!("matched regex {}", pattern)
                } else {
                    format!("did not match regex {}. output was: {}", pattern, out)
                },
            ))
        }

        CheckSpec::EqualsTrim {
            step,
            text,
            case_sensitive,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let cs = case_sensitive.unwrap_or(true);
            let mut out = step_output_text(steps, idx).trim().to_string();
            let mut expected = text.trim().to_string();
            if !cs {
                out = out.to_lowercase();
                expected = expected.to_lowercase();
            }
            let pass = out == expected;
            Ok(mk_outcome(
                CheckType::EqualsTrim,
                idx,
                pass,
                if pass {
                    "exact match".to_string()
                } else {
                    format!("expected '{}', got '{}'", expected, out)
                },
            ))
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
            Ok(mk_outcome(
                CheckType::JsonPointerEquals,
                idx,
                pass,
                if pass {
                    format!("pointer {} matched", pointer)
                } else {
                    format!("pointer {} expected {}, got {:?}", pointer, expected, got)
                },
            ))
        }

        CheckSpec::JsonSchema { step, schema } => {
            let idx = resolve_step_index(*step, steps)?;
            let out = step_output_text(steps, idx);
            let instance: serde_json::Value = match serde_json::from_str(&out) {
                Ok(v) => v,
                Err(e) => {
                    return Ok(mk_outcome(
                        CheckType::JsonSchema,
                        idx,
                        false,
                        format!("output is not valid JSON: {}", e),
                    ));
                }
            };
            match jsonschema::validate(schema, &instance) {
                Ok(()) => Ok(mk_outcome(
                    CheckType::JsonSchema,
                    idx,
                    true,
                    "JSON schema validation passed".to_string(),
                )),
                Err(errors) => Ok(mk_outcome(
                    CheckType::JsonSchema,
                    idx,
                    false,
                    format!("schema validation failed: {}", errors.join("; ")),
                )),
            }
        }

        CheckSpec::LatencyUnderMs { step, max_ms } => {
            let idx = resolve_step_index(*step, steps)?;
            let dur = step_duration_ms(steps, idx);
            let pass = dur <= *max_ms as u128;
            Ok(mk_outcome(
                CheckType::LatencyUnderMs,
                idx,
                pass,
                if pass {
                    format!("{}ms <= {}ms", dur, max_ms)
                } else {
                    format!("{}ms > {}ms", dur, max_ms)
                },
            ))
        }

        CheckSpec::LlmJudge {
            step,
            rubric,
            reference,
            min_score,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            if !llm_judge_enabled {
                return Ok(mk_outcome(
                    CheckType::LlmJudge,
                    idx,
                    true,
                    "LLM judge disabled; treated as pass".to_string(),
                ));
            }
            let backend = judge_backend.ok_or_else(|| anyhow!("no judge backend configured"))?;
            let candidate = step_output_text(steps, idx);
            let jr = judge::run_llm_judge(backend, rubric, &candidate, reference.as_deref())?;
            let threshold = min_score.unwrap_or(0.5);
            let pass = jr.pass && jr.score >= threshold;
            Ok(mk_outcome(
                CheckType::LlmJudge,
                idx,
                pass,
                format!(
                    "judge pass={} score={} threshold={} notes={}",
                    jr.pass, jr.score, threshold, jr.notes
                ),
            ))
        }

        CheckSpec::EventuallyContains {
            step,
            text,
            within_ms,
            case_sensitive,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let dur = step_duration_ms(steps, idx);
            let out = step_output_text(steps, idx);
            let found = output_matches(&out, text, case_sensitive.unwrap_or(false));
            let pass = found && dur <= *within_ms as u128;
            Ok(mk_outcome(
                CheckType::EventuallyContains,
                idx,
                pass,
                if pass {
                    format!("found '{}' within {}ms ({}ms)", text, within_ms, dur)
                } else if !found {
                    format!("missing substring '{}'. output was: {}", text, out)
                } else {
                    format!("found '{}' but exceeded window: {}ms > {}ms", text, dur, within_ms)
                },
            ))
        }

        CheckSpec::StatusCodeEquals { step, code } => {
            let idx = resolve_step_index(*step, steps)?;
            let got = steps.get(idx).and_then(|s| s.status_code);
            let pass = got == Some(*code);
            Ok(mk_outcome(
                CheckType::StatusCodeEquals,
                idx,
                pass,
                if pass {
                    format!("status code matched {}", code)
                } else {
                    format!("expected status {}, got {:?}", code, got)
                },
            ))
        }

        CheckSpec::JsonPointerExists { step, pointer } => {
            let idx = resolve_step_index(*step, steps)?;
            let Some(v) = step_json(steps, idx) else {
                return Ok(mk_outcome(
                    CheckType::JsonPointerExists,
                    idx,
                    false,
                    format!("no JSON available for step {}", idx),
                ));
            };
            let pass = v.pointer(pointer).is_some();
            Ok(mk_outcome(
                CheckType::JsonPointerExists,
                idx,
                pass,
                if pass {
                    format!("pointer {} exists", pointer)
                } else {
                    format!("pointer {} does not exist", pointer)
                },
            ))
        }

        CheckSpec::WithinTimeWindowMs {
            step,
            min_ms,
            max_ms,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let dur = step_duration_ms(steps, idx);
            let pass = dur >= *min_ms as u128 && dur <= *max_ms as u128;
            Ok(mk_outcome(
                CheckType::WithinTimeWindowMs,
                idx,
                pass,
                if pass {
                    format!("{}ms within [{}ms, {}ms]", dur, min_ms, max_ms)
                } else {
                    format!("{}ms outside [{}ms, {}ms]", dur, min_ms, max_ms)
                },
            ))
        }

        CheckSpec::JsonPointerContains {
            step,
            pointer,
            text,
            case_sensitive,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let Some(json) = step_json(steps, idx) else {
                return Ok(mk_outcome(
                    CheckType::JsonPointerContains,
                    idx,
                    false,
                    format!("no JSON available for step {}", idx),
                ));
            };
            let got = json.pointer(pointer).and_then(|v| v.as_str()).unwrap_or("");
            let cs = case_sensitive.unwrap_or(false);
            let pass = output_matches(got, text, cs);
            Ok(mk_outcome(
                CheckType::JsonPointerContains,
                idx,
                pass,
                if pass {
                    format!("pointer {} contains '{}'", pointer, text)
                } else {
                    format!("pointer {} value '{}' does not contain '{}'", pointer, got, text)
                },
            ))
        }

        CheckSpec::JsonArrayLength {
            step,
            pointer,
            min,
            max,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let Some(json) = step_json(steps, idx) else {
                return Ok(mk_outcome(
                    CheckType::JsonArrayLength,
                    idx,
                    false,
                    format!("no JSON available for step {}", idx),
                ));
            };
            let arr = json.pointer(pointer).and_then(|v| v.as_array());
            match arr {
                None => Ok(mk_outcome(
                    CheckType::JsonArrayLength,
                    idx,
                    false,
                    format!("pointer {} is not an array or does not exist", pointer),
                )),
                Some(a) => {
                    let len = a.len();
                    let above_min = min.map_or(true, |m| len >= m);
                    let below_max = max.map_or(true, |m| len <= m);
                    let pass = above_min && below_max;
                    Ok(mk_outcome(
                        CheckType::JsonArrayLength,
                        idx,
                        pass,
                        if pass {
                            format!("array at {} has length {} (within bounds)", pointer, len)
                        } else {
                            format!(
                                "array at {} has length {} (expected [{}, {}])",
                                pointer,
                                len,
                                min.map_or("*".to_string(), |m| m.to_string()),
                                max.map_or("*".to_string(), |m| m.to_string()),
                            )
                        },
                    ))
                }
            }
        }

        CheckSpec::WebhookReceived {
            step,
            min_count,
            payload_pointer,
            payload_expected,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let Some(json) = step_json(steps, idx) else {
                return Ok(mk_outcome(
                    CheckType::WebhookReceived,
                    idx,
                    false,
                    format!("no JSON available for step {}", idx),
                ));
            };
            let count = json.pointer("/count").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let min = min_count.unwrap_or(1);
            let count_ok = count >= min;

            let payload_ok = match (payload_pointer, payload_expected) {
                (Some(ptr), Some(expected)) => {
                    // Check first request's body
                    json.pointer("/requests/0/body")
                        .and_then(|body| body.pointer(ptr))
                        == Some(expected)
                }
                _ => true,
            };

            let pass = count_ok && payload_ok;
            Ok(mk_outcome(
                CheckType::WebhookReceived,
                idx,
                pass,
                if pass {
                    format!("received {} webhook request(s) (min: {})", count, min)
                } else if !count_ok {
                    format!("received {} webhook request(s), expected at least {}", count, min)
                } else {
                    format!("webhook payload mismatch at {:?}", payload_pointer)
                },
            ))
        }

        CheckSpec::SseEventReceived {
            step,
            min_count,
            data_contains,
            data_pointer,
            data_expected,
        } => {
            let idx = resolve_step_index(*step, steps)?;
            let Some(json) = step_json(steps, idx) else {
                return Ok(mk_outcome(
                    CheckType::SseEventReceived,
                    idx,
                    false,
                    format!("no JSON available for step {}", idx),
                ));
            };
            let count = json.pointer("/count").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let min = min_count.unwrap_or(1);
            let count_ok = count >= min;

            let data_ok = if let Some(needle) = data_contains {
                // Check if any event's data contains the substring
                json.pointer("/events")
                    .and_then(|v| v.as_array())
                    .map(|events| {
                        events.iter().any(|e| {
                            e.get("data")
                                .and_then(|d| d.as_str())
                                .map(|d| d.contains(needle.as_str()))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            } else {
                true
            };

            let pointer_ok = match (data_pointer, data_expected) {
                (Some(ptr), Some(expected)) => {
                    // Check first event's data parsed as JSON
                    json.pointer("/events/0/data")
                        .and_then(|d| d.as_str())
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                        .and_then(|parsed| parsed.pointer(ptr).cloned())
                        .as_ref()
                        == Some(expected)
                }
                _ => true,
            };

            let pass = count_ok && data_ok && pointer_ok;
            Ok(mk_outcome(
                CheckType::SseEventReceived,
                idx,
                pass,
                if pass {
                    format!("received {} SSE event(s) (min: {})", count, min)
                } else if !count_ok {
                    format!("received {} SSE event(s), expected at least {}", count, min)
                } else if !data_ok {
                    format!("no SSE event data contains '{}'", data_contains.as_deref().unwrap_or(""))
                } else {
                    format!("SSE event data mismatch at {:?}", data_pointer)
                },
            ))
        }

        CheckSpec::StepOrder {
            before_step,
            after_step,
        } => {
            let a = steps.get(*before_step).ok_or_else(|| {
                anyhow!("step_order: before_step {} does not exist", before_step)
            })?;
            let b = steps.get(*after_step).ok_or_else(|| {
                anyhow!("step_order: after_step {} does not exist", after_step)
            })?;
            let a_end = a.started_at + a.duration;
            let pass = a_end <= b.started_at;
            Ok(CheckOutcome {
                check_type: CheckType::StepOrder,
                step: None,
                pass,
                score: if pass { 1.0 } else { 0.0 },
                details: if pass {
                    format!("step {} finished before step {} started", before_step, after_step)
                } else {
                    format!("step {} did not finish before step {} started", before_step, after_step)
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::SendResponse;
    use crate::runner::StepOutcome;
    use crate::types::StepKind;
    use std::time::Duration;

    fn mk_step(
        output: &str,
        json: Option<serde_json::Value>,
        dur_ms: u64,
        status_code: Option<u16>,
    ) -> StepOutcome {
        StepOutcome {
            index: 0,
            kind: StepKind::HttpProbe,
            name: None,
            input: None,
            response: Some(SendResponse {
                output_text: output.to_string(),
                raw_stdout: output.to_string(),
                raw_stderr: String::new(),
                json,
                duration: Duration::from_millis(dur_ms),
                exit_code: None,
            }),
            duration: Duration::from_millis(dur_ms),
            status_code,
            exit_code: None,
            poll_attempts: None,
            poll_satisfied: None,
            started_at: std::time::Instant::now(),
        }
    }

    #[test]
    fn status_code_equals_check() {
        let steps = vec![mk_step("{}", Some(serde_json::json!({})), 10, Some(200))];
        let c = CheckSpec::StatusCodeEquals {
            step: Some(0),
            code: 200,
        };
        let out = eval_check(&c, &steps, false, None).expect("check should run");
        assert!(out.pass);
    }

    #[test]
    fn json_pointer_exists_check() {
        let steps = vec![mk_step(
            "{\"ok\":true}",
            Some(serde_json::json!({"ok": true})),
            10,
            Some(200),
        )];
        let c = CheckSpec::JsonPointerExists {
            step: Some(0),
            pointer: "/ok".to_string(),
        };
        let out = eval_check(&c, &steps, false, None).expect("check should run");
        assert!(out.pass);
    }

    #[test]
    fn within_time_window_check() {
        let steps = vec![mk_step("x", None, 350, None)];
        let c = CheckSpec::WithinTimeWindowMs {
            step: Some(0),
            min_ms: 300,
            max_ms: 400,
        };
        let out = eval_check(&c, &steps, false, None).expect("check should run");
        assert!(out.pass);
    }

    #[test]
    fn contains_case_insensitive() {
        let steps = vec![mk_step("Hello World", None, 1, None)];
        let check = CheckSpec::Contains {
            step: Some(0),
            text: "hello".to_string(),
            case_sensitive: Some(false),
        };
        let out = eval_check(&check, &steps, false, None).expect("check should run");
        assert!(out.pass);
        assert_eq!(out.details, "found substring 'hello'".to_string());
    }

    #[test]
    fn contains_case_sensitive_failure_details() {
        let steps = vec![mk_step("Hello", None, 1, None)];
        let check = CheckSpec::Contains {
            step: Some(0),
            text: "hello".to_string(),
            case_sensitive: Some(true),
        };
        let out = eval_check(&check, &steps, false, None).expect("check should run");
        assert!(!out.pass);
        assert_eq!(
            out.details,
            "missing substring 'hello'. output was: Hello".to_string()
        );
    }

    #[test]
    fn not_contains_case_sensitive_failure_details() {
        let steps = vec![mk_step("token", None, 1, None)];
        let check = CheckSpec::NotContains {
            step: Some(0),
            text: "token".to_string(),
            case_sensitive: Some(true),
        };
        let out = eval_check(&check, &steps, false, None).expect("check should run");
        assert!(!out.pass);
        assert_eq!(
            out.details,
            "forbidden substring 'token' was found. output was: token".to_string()
        );
    }

    #[test]
    fn not_empty_failure_details() {
        let steps = vec![mk_step("   ", None, 1, None)];
        let check = CheckSpec::NotEmpty {
            step: Some(0),
        };
        let out = eval_check(&check, &steps, false, None).expect("check should run");
        assert!(!out.pass);
        assert_eq!(out.details, "output is empty or whitespace-only".to_string());
    }

    #[test]
    fn latency_checks_pass_and_fail() {
        let steps = vec![mk_step("x", None, 120, None)];
        let fast = CheckSpec::LatencyUnderMs {
            step: Some(0),
            max_ms: 200,
        };
        let slow = CheckSpec::LatencyUnderMs {
            step: Some(0),
            max_ms: 50,
        };
        let fast_out = eval_check(&fast, &steps, false, None).expect("check should run");
        let slow_out = eval_check(&slow, &steps, false, None).expect("check should run");
        assert!(fast_out.pass);
        assert!(!slow_out.pass);
    }

    #[test]
    fn json_pointer_contains_check() {
        let steps = vec![mk_step(
            "",
            Some(serde_json::json!({"message": "Hello World", "status": "ok"})),
            10,
            Some(200),
        )];
        let pass_check = CheckSpec::JsonPointerContains {
            step: Some(0),
            pointer: "/message".to_string(),
            text: "World".to_string(),
            case_sensitive: Some(true),
        };
        let out = eval_check(&pass_check, &steps, false, None).expect("check should run");
        assert!(out.pass);

        let fail_check = CheckSpec::JsonPointerContains {
            step: Some(0),
            pointer: "/message".to_string(),
            text: "world".to_string(),
            case_sensitive: Some(true),
        };
        let out = eval_check(&fail_check, &steps, false, None).expect("check should run");
        assert!(!out.pass);

        // Case insensitive should pass
        let ci_check = CheckSpec::JsonPointerContains {
            step: Some(0),
            pointer: "/message".to_string(),
            text: "world".to_string(),
            case_sensitive: Some(false),
        };
        let out = eval_check(&ci_check, &steps, false, None).expect("check should run");
        assert!(out.pass);
    }

    #[test]
    fn json_array_length_check() {
        let steps = vec![mk_step(
            "",
            Some(serde_json::json!({"items": [1, 2, 3]})),
            10,
            Some(200),
        )];
        let pass_check = CheckSpec::JsonArrayLength {
            step: Some(0),
            pointer: "/items".to_string(),
            min: Some(1),
            max: Some(5),
        };
        let out = eval_check(&pass_check, &steps, false, None).expect("check should run");
        assert!(out.pass);

        let fail_check = CheckSpec::JsonArrayLength {
            step: Some(0),
            pointer: "/items".to_string(),
            min: Some(5),
            max: None,
        };
        let out = eval_check(&fail_check, &steps, false, None).expect("check should run");
        assert!(!out.pass);
    }

    #[test]
    fn webhook_received_check() {
        let steps = vec![mk_step(
            "",
            Some(serde_json::json!({
                "requests": [{"method": "POST", "path": "/callback", "body": {"status": "done"}}],
                "count": 1,
                "port": 9090
            })),
            10,
            None,
        )];
        let check = CheckSpec::WebhookReceived {
            step: Some(0),
            min_count: Some(1),
            payload_pointer: Some("/status".to_string()),
            payload_expected: Some(serde_json::json!("done")),
        };
        let out = eval_check(&check, &steps, false, None).expect("check should run");
        assert!(out.pass);
    }

    #[test]
    fn sse_event_received_check() {
        let steps = vec![mk_step(
            "",
            Some(serde_json::json!({
                "events": [
                    {"event": "message", "data": "update: new content"},
                    {"data": "second event"}
                ],
                "count": 2
            })),
            10,
            None,
        )];
        let check = CheckSpec::SseEventReceived {
            step: Some(0),
            min_count: Some(1),
            data_contains: Some("update".to_string()),
            data_pointer: None,
            data_expected: None,
        };
        let out = eval_check(&check, &steps, false, None).expect("check should run");
        assert!(out.pass);
    }

    #[test]
    fn step_order_check() {
        let now = std::time::Instant::now();
        let steps = vec![
            StepOutcome {
                index: 0,
                kind: StepKind::Exec,
                name: None,
                input: None,
                response: None,
                duration: Duration::from_millis(100),
                status_code: None,
                exit_code: None,
                poll_attempts: None,
                poll_satisfied: None,
                started_at: now,
            },
            StepOutcome {
                index: 1,
                kind: StepKind::Exec,
                name: None,
                input: None,
                response: None,
                duration: Duration::from_millis(50),
                status_code: None,
                exit_code: None,
                poll_attempts: None,
                poll_satisfied: None,
                started_at: now + Duration::from_millis(200),
            },
        ];
        let check = CheckSpec::StepOrder {
            before_step: 0,
            after_step: 1,
        };
        let out = eval_check(&check, &steps, false, None).expect("check should run");
        assert!(out.pass);

        // Reverse should fail
        let check_rev = CheckSpec::StepOrder {
            before_step: 1,
            after_step: 0,
        };
        let out_rev = eval_check(&check_rev, &steps, false, None).expect("check should run");
        assert!(!out_rev.pass);
    }

    #[test]
    fn json_schema_valid_and_invalid_match_scores() {
        let steps = vec![mk_step("{\"ok\": true}", None, 1, None)];
        let passing = CheckSpec::JsonSchema {
            step: Some(0),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "ok": {"type":"boolean"}
                },
                "required": ["ok"],
                "additionalProperties": false
            }),
        };
        let pass = eval_check(&passing, &steps, false, None).expect("check should run");
        assert!(pass.pass);

        let failing = CheckSpec::JsonSchema {
            step: Some(0),
            schema: serde_json::json!({"type": "object", "properties": {"count": {"type":"number"}}, "required": ["count"]}),
        };
        let fail = eval_check(&failing, &steps, false, None).expect("check should run");
        assert!(!fail.pass);
        assert!(fail.details.contains("schema validation failed"));
    }
}
