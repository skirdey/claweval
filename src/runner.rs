use crate::backend::{build_backend, AgentBackend, SendRequest, SendResponse};
use crate::checks;
use crate::printer::Printer;
use crate::report::{
    self, BackendInfo, EpisodeReport, EpisodeRunReport, EpisodeSummary, OverallSummary, StepKindDetails,
    StepReport, SuiteReport,
};
use crate::services::ServiceManager;
use crate::spec::{EpisodeSpec, PollCondition, StepSpec, SuiteSpec};
use crate::stats;
use crate::types::{HttpMethod, StepKind};
use crate::util::parse_embedded_json;
use crate::vars;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use rayon::prelude::*;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub enable_llm_judge: bool,
    pub verbose: bool,
    /// Glob pattern to filter episodes by id (None = run all).
    pub filter: Option<String>,
    /// Number of parallel episode workers (1 = serial).
    pub jobs: u32,
}

#[derive(Debug, Clone)]
pub struct StepOutcome {
    pub index: usize,
    pub kind: StepKind,
    pub name: Option<String>,
    pub input: Option<String>,
    pub response: Option<SendResponse>,
    pub duration: Duration,
    pub status_code: Option<u16>,
    pub exit_code: Option<i32>,
    pub poll_attempts: Option<u32>,
    pub poll_satisfied: Option<bool>,
    pub started_at: Instant,
}

impl StepOutcome {
    fn new(index: usize, kind: StepKind, name: Option<String>, input: Option<String>, duration: Duration, started_at: Instant) -> Self {
        Self {
            index,
            kind,
            name,
            input,
            response: None,
            duration,
            status_code: None,
            exit_code: None,
            poll_attempts: None,
            poll_satisfied: None,
            started_at,
        }
    }

    fn user(idx: usize, name: Option<String>, input: String, response: SendResponse, started_at: Instant) -> Self {
        Self {
            index: idx,
            kind: StepKind::User,
            name,
            input: Some(input),
            duration: response.duration,
            response: Some(response),
            status_code: None,
            exit_code: None,
            poll_attempts: None,
            poll_satisfied: None,
            started_at,
        }
    }

    fn sleep(index: usize, name: Option<String>, duration: Duration, input: String, started_at: Instant) -> Self {
        let mut outcome = Self::new(index, StepKind::Sleep, name, Some(input), duration, started_at);
        outcome.response = None;
        outcome
    }

    fn note(index: usize, name: Option<String>, text: String, duration: Duration, started_at: Instant) -> Self {
        Self::new(index, StepKind::Note, name, Some(text), duration, started_at)
    }

    fn exec(index: usize, name: Option<String>, input: String, response: SendResponse, started_at: Instant) -> Self {
        Self {
            index,
            kind: StepKind::Exec,
            name,
            input: Some(input),
            duration: response.duration,
            response: Some(response.clone()),
            status_code: None,
            exit_code: response.exit_code,
            poll_attempts: None,
            poll_satisfied: None,
            started_at,
        }
    }

    fn http_probe(
        index: usize,
        name: Option<String>,
        input: String,
        response: SendResponse,
        status_code: u16,
        started_at: Instant,
    ) -> Self {
        Self {
            index,
            kind: StepKind::HttpProbe,
            name,
            input: Some(input),
            duration: response.duration,
            response: Some(response),
            status_code: Some(status_code),
            exit_code: None,
            poll_attempts: None,
            poll_satisfied: None,
            started_at,
        }
    }

    fn poll(
        index: usize,
        name: Option<String>,
        input: String,
        response: SendResponse,
        status_code: Option<u16>,
        exit_code: Option<i32>,
        attempts: u32,
        satisfied: bool,
        duration: Duration,
        started_at: Instant,
    ) -> Self {
        Self {
            response: Some(response),
            status_code,
            exit_code,
            poll_attempts: Some(attempts),
            poll_satisfied: Some(satisfied),
            ..Self::new(index, StepKind::Poll, name, Some(input), duration, started_at)
        }
    }
}

fn send_exec(
    command: &str,
    args: &[String],
    env: Option<&std::collections::HashMap<String, String>>,
) -> Result<SendResponse> {
    let start = Instant::now();
    let mut cmd = Command::new(command);
    cmd.args(args);
    if let Some(envs) = env {
        for (k, v) in envs {
            cmd.env(k, v);
        }
    }
    let output = cmd
        .output()
        .with_context(|| format!("failed to run exec command '{}'", command))?;
    let duration = start.elapsed();
    let raw_stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let raw_stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(SendResponse {
            output_text: raw_stdout.trim().to_string(),
            raw_stdout: raw_stdout.clone(),
            raw_stderr,
            json: parse_embedded_json(&raw_stdout),
            duration,
            exit_code: output.status.code(),
        })
}

fn send_http_probe(
    method: HttpMethod,
    url: &str,
    headers: Option<&std::collections::HashMap<String, String>>,
    body: Option<&serde_json::Value>,
    timeout_ms: Option<u64>,
) -> Result<(SendResponse, u16)> {
    let start = Instant::now();
    let mut req = ureq::request(method.as_str(), url);
    if let Some(ms) = timeout_ms {
        req = req.timeout(Duration::from_millis(ms));
    }
    if let Some(hs) = headers {
        for (k, v) in hs {
            req = req.set(k, v);
        }
    }

    let resp_result = match body {
        Some(v) => req.send_string(&v.to_string()),
        None => req.call(),
    };

    let (status, body_text) = match resp_result {
        Ok(r) => {
            let code = r.status();
            let text = r.into_string().unwrap_or_default();
            (code, text)
        }
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            (code, text)
        }
        Err(e) => {
            return Err(anyhow!("http_probe request failed: {}", e));
        }
    };

    let duration = start.elapsed();
    let json = parse_embedded_json(&body_text);
    Ok((
        SendResponse {
            output_text: body_text.trim().to_string(),
            raw_stdout: body_text,
            raw_stderr: String::new(),
            json,
            duration,
            exit_code: None,
        },
        status as u16,
    ))
}

fn poll_condition_satisfied(until: &PollCondition, response: &SendResponse, status_code: Option<u16>) -> Result<bool> {
    match until {
        PollCondition::ContainsText { text, case_sensitive } => {
            let cs = case_sensitive.unwrap_or(false);
            if cs {
                Ok(response.output_text.contains(text))
            } else {
                Ok(response.output_text.to_lowercase().contains(&text.to_lowercase()))
            }
        }
        PollCondition::Regex { pattern } => Ok(Regex::new(pattern)?.is_match(&response.output_text)),
        PollCondition::StatusCode { code } => Ok(status_code == Some(*code)),
        PollCondition::JsonPointerEquals { pointer, expected } => {
            Ok(response.json.as_ref().and_then(|v| v.pointer(pointer)) == Some(expected))
        }
    }
}

fn execute_probe_step(step: &StepSpec) -> Result<(SendResponse, Option<u16>, Option<i32>)> {
    match step {
        StepSpec::Exec { command, args, env, .. } => {
            let args_vec = args.clone().unwrap_or_default();
            let resp = send_exec(command, &args_vec, env.as_ref())?;
            Ok((resp.clone(), None, resp.exit_code))
        }
        StepSpec::HttpProbe {
            url,
            method,
            headers,
            body,
            timeout_ms,
            ..
        } => {
            let method = method.unwrap_or_default();
            let (resp, status) = send_http_probe(method, url, headers.as_ref(), body.as_ref(), *timeout_ms)?;
            Ok((resp, Some(status), None))
        }
        _ => Err(anyhow!("poll probe must be either 'exec' or 'http_probe' step")),
    }
}

#[derive(Debug, Clone)]
struct PollStepResult {
    response: SendResponse,
    status_code: Option<u16>,
    exit_code: Option<i32>,
    attempts: u32,
    satisfied: bool,
    errors: Vec<String>,
    duration: Duration,
}

fn run_poll_step(
    mut do_probe: impl FnMut() -> Result<(SendResponse, Option<u16>, Option<i32>)>,
    mut is_satisfied: impl FnMut(&SendResponse, Option<u16>) -> Result<bool>,
    interval_ms: u64,
    timeout_ms: u64,
) -> Result<PollStepResult> {
    let poll_start = Instant::now();
    let mut attempts: u32 = 0;
    let mut satisfied = false;
    let mut last_response: Option<SendResponse> = None;
    let mut last_status: Option<u16> = None;
    let mut last_exit: Option<i32> = None;
    let mut errors: Vec<String> = Vec::new();

    while poll_start.elapsed() < Duration::from_millis(timeout_ms) {
        attempts += 1;
        match do_probe() {
            Ok((resp, status, exit)) => {
                let ok = is_satisfied(&resp, status)?;
                last_status = status;
                last_exit = exit;
                last_response = Some(resp);
                if ok {
                    satisfied = true;
                    break;
                }
            }
            Err(e) => errors.push(e.to_string()),
        }
        std::thread::sleep(Duration::from_millis(interval_ms));
    }

    let response = match last_response {
        Some(r) => r,
        None => SendResponse {
            output_text: String::new(),
            raw_stdout: String::new(),
            raw_stderr: String::new(),
            json: None,
            duration: poll_start.elapsed(),
            exit_code: None,
        },
    };

    Ok(PollStepResult {
        response,
        status_code: last_status,
        exit_code: last_exit,
        attempts,
        satisfied,
        errors,
        duration: poll_start.elapsed(),
    })
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis()
}

fn matches_filter(filter: &Option<String>, id: &str) -> bool {
    match filter {
        None => true,
        Some(pat) => glob::Pattern::new(pat)
            .map(|p| p.matches(id))
            .unwrap_or(true),
    }
}

pub fn run_suite(spec: &SuiteSpec, opts: RunOptions) -> Result<SuiteReport> {
    let started_at = now_unix_ms();
    let suite_start = Instant::now();

    // Start background services if configured.
    let _services = match &spec.services {
        Some(svc_specs) if !svc_specs.is_empty() => {
            Some(ServiceManager::start(svc_specs).context("failed to start suite services")?)
        }
        _ => None,
    };

    let backend: Arc<dyn AgentBackend> = Arc::from(build_backend(&spec.backend)?);

    // Build a separate judge backend if configured; otherwise fall back to the
    // agent backend (preserving original behaviour).
    let judge_backend: Option<Arc<dyn AgentBackend>> = spec
        .judge_backend
        .as_ref()
        .map(build_backend)
        .transpose()?
        .map(Arc::from);

    let printer = Arc::new(Printer::new());

    // Filter episodes.
    let episodes: Vec<&EpisodeSpec> = spec
        .episodes
        .iter()
        .filter(|ep| matches_filter(&opts.filter, &ep.id))
        .collect();

    let jobs = opts.jobs.max(1) as usize;

    type EpResult = Result<(EpisodeReport, u32, u32)>;

    let run_ep = |ep: &&EpisodeSpec| -> EpResult {
        let global_mul = spec.global_repeats.unwrap_or(1);
        let repeats = ep.repeats.unwrap_or(1) * global_mul;
        // Effective judge backend: prefer separate judge, fall back to agent.
        let eff_judge: Option<&dyn AgentBackend> = judge_backend
            .as_deref()
            .or_else(|| Some(backend.as_ref()));
        run_episode(ep, repeats, backend.as_ref(), eff_judge, &opts, printer.as_ref())
            .with_context(|| format!("episode '{}' failed", ep.id))
    };

    let episode_results: Vec<(EpisodeReport, u32, u32)> = if jobs > 1 {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build()
            .context("failed to build rayon thread pool")?;
        pool.install(|| episodes.par_iter().map(run_ep).collect::<Result<Vec<_>>>())?
    } else {
        episodes
            .iter()
            .map(run_ep)
            .collect::<Result<Vec<_>>>()?
    };

    let duration_ms = suite_start.elapsed().as_millis();

    let mut total_runs: u32 = 0;
    let mut passed_runs: u32 = 0;
    let mut episode_reports = Vec::new();

    for (report, ep_total, ep_passed) in episode_results {
        total_runs += ep_total;
        passed_runs += ep_passed;
        episode_reports.push(report);
    }

    printer.suite_summary(total_runs, passed_runs, duration_ms);

    Ok(SuiteReport {
        name: spec.name.clone(),
        description: spec.description.clone(),
        capability_tags: spec.capability_tags.clone(),
        scoring_class: spec.scoring_class.clone(),
        backend: BackendInfo {
            backend_type: backend.backend_type(),
            detail: Some(serde_json::json!({
                "suite_backend_spec": spec.backend,
            })),
        },
        started_at_unix_ms: started_at,
        duration_ms,
        overall: OverallSummary {
            total_runs,
            passed_runs,
            pass_rate: stats::pass_rate(passed_runs, total_runs),
        },
        episodes: episode_reports,
    })
}

fn run_episode(
    ep: &EpisodeSpec,
    repeats: u32,
    backend: &dyn AgentBackend,
    judge_backend: Option<&dyn AgentBackend>,
    opts: &RunOptions,
    printer: &Printer,
) -> Result<(EpisodeReport, u32, u32)> {
    printer.episode_start(&ep.id, ep.description.as_deref());

    let mut runs = Vec::new();
    let mut passed: u32 = 0;
    let mut durations: Vec<u128> = Vec::new();

    for i in 0..repeats {
        let (run, pass) = run_episode_once(ep, i, backend, judge_backend, opts)?;
        printer.run_result(&ep.id, i, pass, &run.checks);
        durations.push(run.duration_ms);
        if pass {
            passed += 1;
        }
        runs.push(run);
    }

    let avg_duration_ms = if durations.is_empty() {
        0.0
    } else {
        durations.iter().sum::<u128>() as f64 / durations.len() as f64
    };

    let total = repeats;

    Ok((
        EpisodeReport {
            id: ep.id.clone(),
            description: ep.description.clone(),
            repeats,
            summary: EpisodeSummary {
                total_runs: total,
                passed_runs: passed,
                pass_rate: stats::pass_rate(passed, total),
                avg_duration_ms,
            },
            runs,
        },
        total,
        passed,
    ))
}

/// Interpolate a step spec in-place: serialize to JSON, interpolate vars, deserialize back.
fn interpolate_step(step: &StepSpec, step_vars: &HashMap<String, String>) -> Result<StepSpec> {
    if step_vars.is_empty() {
        return Ok(step.clone());
    }
    let mut json = serde_json::to_value(step)?;
    vars::interpolate_json(&mut json, step_vars);
    Ok(serde_json::from_value(json)?)
}

/// Execute a single step and return its outcome. Shared between main steps, setup, and teardown.
fn execute_step(
    step: &StepSpec,
    idx: usize,
    ep_id: &str,
    session_id: &str,
    backend: &dyn AgentBackend,
    step_outcomes: &[StepOutcome],
    step_vars: &mut HashMap<String, String>,
    opts: &RunOptions,
) -> Result<StepOutcome> {
    let step_start = Instant::now();
    match step {
        StepSpec::User { input, name } => {
            if opts.verbose {
                eprintln!("[{}:{}] USER> {}", ep_id, idx, input);
            }
            let resp = backend.send(SendRequest {
                session_id: session_id.to_string(),
                message: input.clone(),
            })?;
            if opts.verbose {
                eprintln!("[{}:{}] ASSISTANT> {}", ep_id, idx, resp.output_text);
            }
            Ok(StepOutcome::user(idx, name.clone(), input.clone(), resp, step_start))
        }
        StepSpec::Sleep { ms, name } => {
            if opts.verbose {
                eprintln!("[{}:{}] SLEEP {}ms", ep_id, idx, ms);
            }
            std::thread::sleep(Duration::from_millis(*ms));
            Ok(StepOutcome::sleep(
                idx,
                name.clone(),
                step_start.elapsed(),
                format!("sleep {}ms", ms),
                step_start,
            ))
        }
        StepSpec::Note { text, name } => {
            if opts.verbose {
                eprintln!("[{}:{}] NOTE {}", ep_id, idx, text);
            }
            Ok(StepOutcome::note(
                idx,
                name.clone(),
                text.clone(),
                step_start.elapsed(),
                step_start,
            ))
        }
        StepSpec::Exec {
            command,
            args,
            env,
            name,
        } => {
            if opts.verbose {
                eprintln!("[{}:{}] EXEC {} {:?}", ep_id, idx, command, args);
            }
            let args_vec = args.clone().unwrap_or_default();
            let resp = send_exec(command, &args_vec, env.as_ref())?;
            Ok(StepOutcome::exec(
                idx,
                name.clone(),
                format!("{} {}", command, args_vec.join(" ")).trim().to_string(),
                resp,
                step_start,
            ))
        }
        StepSpec::HttpProbe {
            url,
            method,
            headers,
            body,
            timeout_ms,
            name,
        } => {
            let verb = method.unwrap_or_default();
            if opts.verbose {
                eprintln!("[{}:{}] HTTP_PROBE {} {}", ep_id, idx, verb, url);
            }
            let (resp, status_code) =
                send_http_probe(verb, url, headers.as_ref(), body.as_ref(), *timeout_ms)?;
            Ok(StepOutcome::http_probe(
                idx,
                name.clone(),
                format!("{} {}", verb, url),
                resp,
                status_code,
                step_start,
            ))
        }
        StepSpec::Poll {
            probe,
            interval_ms,
            timeout_ms,
            until,
            name,
        } => {
            let poll = run_poll_step(
                || execute_probe_step(probe),
                |resp, status| poll_condition_satisfied(until, resp, status),
                *interval_ms,
                *timeout_ms,
            )?;
            Ok(StepOutcome::poll(
                idx,
                name.clone(),
                format!("poll every {}ms for {}ms", interval_ms, timeout_ms),
                poll.response,
                poll.status_code,
                poll.exit_code,
                poll.attempts,
                poll.satisfied,
                poll.duration,
                step_start,
            ))
        }
        StepSpec::SetVar {
            var,
            step: step_ref,
            pointer,
            name,
        } => {
            let source_idx = step_ref.unwrap_or_else(|| {
                if step_outcomes.is_empty() { 0 } else { step_outcomes.len() - 1 }
            });
            let value = vars::extract_var(step_outcomes, source_idx, pointer)
                .unwrap_or_default();
            if opts.verbose {
                eprintln!("[{}:{}] SET_VAR {}={}", ep_id, idx, var, value);
            }
            step_vars.insert(var.clone(), value.clone());
            let resp = SendResponse {
                output_text: value.clone(),
                raw_stdout: value,
                raw_stderr: String::new(),
                json: Some(serde_json::json!({ var.as_str(): step_vars.get(var) })),
                duration: step_start.elapsed(),
                exit_code: None,
            };
            Ok(StepOutcome {
                response: Some(resp),
                ..StepOutcome::new(idx, StepKind::SetVar, name.clone(), Some(format!("set {} from {}", var, pointer)), step_start.elapsed(), step_start)
            })
        }
        StepSpec::WebhookListen {
            port,
            path,
            timeout_ms,
            min_requests,
            name,
        } => {
            if opts.verbose {
                eprintln!("[{}:{}] WEBHOOK_LISTEN port={} timeout={}ms", ep_id, idx, port, timeout_ms);
            }
            let path_filter = path.as_deref().unwrap_or("/");
            let min_reqs = min_requests.unwrap_or(0);
            let result = crate::webhook_listener::listen(
                *port,
                path_filter,
                Duration::from_millis(*timeout_ms),
                min_reqs,
            )?;
            let json = serde_json::to_value(&result)?;
            let resp = SendResponse {
                output_text: serde_json::to_string(&result)?,
                raw_stdout: serde_json::to_string_pretty(&result)?,
                raw_stderr: String::new(),
                json: Some(json),
                duration: step_start.elapsed(),
                exit_code: None,
            };
            Ok(StepOutcome {
                response: Some(resp),
                ..StepOutcome::new(idx, StepKind::WebhookListen, name.clone(), Some(format!("webhook_listen :{}", port)), step_start.elapsed(), step_start)
            })
        }
        StepSpec::SseSubscribe {
            url,
            headers,
            timeout_ms,
            max_events,
            event_filter,
            name,
        } => {
            if opts.verbose {
                eprintln!("[{}:{}] SSE_SUBSCRIBE {}", ep_id, idx, url);
            }
            let result = crate::sse_client::subscribe(
                url,
                headers.as_ref(),
                Duration::from_millis(*timeout_ms),
                max_events.unwrap_or(0),
                event_filter.as_deref(),
            )?;
            let json = serde_json::to_value(&result)?;
            let resp = SendResponse {
                output_text: serde_json::to_string(&result)?,
                raw_stdout: serde_json::to_string_pretty(&result)?,
                raw_stderr: String::new(),
                json: Some(json),
                duration: step_start.elapsed(),
                exit_code: None,
            };
            Ok(StepOutcome {
                response: Some(resp),
                ..StepOutcome::new(idx, StepKind::SseSubscribe, name.clone(), Some(format!("sse {}", url)), step_start.elapsed(), step_start)
            })
        }
        StepSpec::Parallel {
            steps: sub_steps,
            name,
        } => {
            if opts.verbose {
                eprintln!("[{}:{}] PARALLEL ({} sub-steps)", ep_id, idx, sub_steps.len());
            }
            // Run sub-steps concurrently using std::thread::scope.
            let sub_outcomes: Vec<Result<StepOutcome>> = std::thread::scope(|s| {
                let handles: Vec<_> = sub_steps
                    .iter()
                    .enumerate()
                    .map(|(sub_idx, sub_step)| {
                        let session_id = session_id.to_string();
                        let vars_snapshot = step_vars.clone();
                        s.spawn(move || {
                            let mut sub_vars = vars_snapshot;
                            execute_step(
                                sub_step,
                                sub_idx,
                                ep_id,
                                &session_id,
                                backend,
                                step_outcomes,
                                &mut sub_vars,
                                opts,
                            )
                        })
                    })
                    .collect();

                handles
                    .into_iter()
                    .map(|h| h.join().unwrap_or_else(|_| Err(anyhow!("parallel sub-step panicked"))))
                    .collect()
            });

            // Collect sub-step outcomes into a JSON array.
            let mut sub_json = Vec::new();
            for result in &sub_outcomes {
                match result {
                    Ok(outcome) => {
                        let json = outcome.response.as_ref().and_then(|r| r.json.clone())
                            .unwrap_or(serde_json::Value::Null);
                        sub_json.push(json);
                    }
                    Err(e) => {
                        sub_json.push(serde_json::json!({"error": e.to_string()}));
                    }
                }
            }

            let combined_json = serde_json::json!({ "sub_steps": sub_json });
            let resp = SendResponse {
                output_text: serde_json::to_string(&combined_json)?,
                raw_stdout: serde_json::to_string_pretty(&combined_json)?,
                raw_stderr: String::new(),
                json: Some(combined_json),
                duration: step_start.elapsed(),
                exit_code: None,
            };

            Ok(StepOutcome {
                response: Some(resp),
                ..StepOutcome::new(idx, StepKind::Parallel, name.clone(), Some(format!("parallel {} steps", sub_steps.len())), step_start.elapsed(), step_start)
            })
        }
    }
}

fn run_episode_once(
    ep: &EpisodeSpec,
    run_index: u32,
    backend: &dyn AgentBackend,
    judge_backend: Option<&dyn AgentBackend>,
    opts: &RunOptions,
) -> Result<(EpisodeRunReport, bool)> {
    let session_id = backend.new_session_id();
    let start = Instant::now();

    // Initialize variables: pre-seeded from episode spec.
    let mut step_vars: HashMap<String, String> = ep.vars.clone().unwrap_or_default();

    // Run setup steps (failures abort the run).
    if let Some(setup_steps) = &ep.setup {
        for (idx, step) in setup_steps.iter().enumerate() {
            let interpolated = interpolate_step(step, &step_vars)?;
            execute_step(&interpolated, idx, &ep.id, &session_id, backend, &[], &mut step_vars, opts)
                .with_context(|| format!("setup step {} failed", idx))?;
        }
    }

    let mut step_outcomes: Vec<StepOutcome> = Vec::new();

    for (idx, step) in ep.steps.iter().enumerate() {
        let interpolated = interpolate_step(step, &step_vars)?;
        let outcome = execute_step(
            &interpolated,
            idx,
            &ep.id,
            &session_id,
            backend,
            &step_outcomes,
            &mut step_vars,
            opts,
        )?;
        step_outcomes.push(outcome);
    }

    // Evaluate checks.
    let mut check_outcomes = Vec::new();
    for c in &ep.checks {
        let outcome =
            checks::eval_check(c, &step_outcomes, opts.enable_llm_judge, judge_backend)?;
        check_outcomes.push(outcome);
    }

    let pass = check_outcomes.iter().all(|c| c.pass);
    let duration_ms = start.elapsed().as_millis();

    // Run teardown steps (best-effort, failures logged but don't affect pass/fail).
    if let Some(teardown_steps) = &ep.teardown {
        for (idx, step) in teardown_steps.iter().enumerate() {
            let interpolated = interpolate_step(step, &step_vars).unwrap_or_else(|_| step.clone());
            if let Err(e) = execute_step(&interpolated, idx, &ep.id, &session_id, backend, &step_outcomes, &mut step_vars, opts) {
                eprintln!("[{}] teardown step {} failed (ignored): {}", ep.id, idx, e);
            }
        }
    }

    let step_reports = step_outcomes
        .into_iter()
        .map(|s| StepReport {
            index: s.index,
            kind: s.kind,
            name: s.name,
            input: s.input,
            output_text: s.response.as_ref().map(|r| r.output_text.clone()),
            duration_ms: report::dur_ms(s.duration),
            json: s.response.as_ref().and_then(|r| r.json.clone()),
            raw_stdout: s.response.as_ref().map(|r| r.raw_stdout.clone()),
            raw_stderr: s.response.as_ref().map(|r| r.raw_stderr.clone()),
            step_kind_details: Some(StepKindDetails {
                status_code: s.status_code,
                exit_code: s.exit_code,
                poll_attempts: s.poll_attempts,
                poll_satisfied: s.poll_satisfied,
            })
            .filter(|d| {
                d.status_code.is_some()
                    || d.exit_code.is_some()
                    || d.poll_attempts.is_some()
                    || d.poll_satisfied.is_some()
            }),
        })
        .collect();

    Ok((
        EpisodeRunReport {
            run_index,
            session_id,
            pass,
            duration_ms,
            steps: step_reports,
            checks: check_outcomes,
        },
        pass,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::SendResponse;
    #[allow(unused_imports)]
    use anyhow::anyhow;
    use std::time::Duration;

    fn mk_poll_response(output: &str, exit: Option<i32>) -> SendResponse {
        SendResponse {
            output_text: output.to_string(),
            raw_stdout: output.to_string(),
            raw_stderr: String::new(),
            json: None,
            duration: Duration::from_millis(10),
            exit_code: exit,
        }
    }

    #[test]
    fn poll_step_satisfy_first_attempt() {
        let mut attempts = 0;
        let result = run_poll_step(
            || {
                attempts += 1;
                Ok((mk_poll_response("all good", Some(0)), Some(200), Some(0)))
            },
            |resp, status| Ok(resp.output_text == "all good" && status == Some(200)),
            1,
            50,
        )
        .expect("poll should succeed");
        assert_eq!(attempts, 1);
        assert_eq!(result.attempts, 1);
        assert!(result.satisfied);
        assert_eq!(result.status_code, Some(200));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn poll_step_never_satisfied_times_out() {
        let mut attempts = 0;
        let result = run_poll_step(
            || {
                attempts += 1;
                Ok((mk_poll_response("pending", Some(0)), Some(200), Some(0)))
            },
            |resp, status| Ok(resp.output_text == "complete" && status == Some(200)),
            1,
            5,
        )
        .expect("poll should run");
        assert!(attempts >= 1);
        assert!(result.attempts >= 1);
        assert!(!result.satisfied);
        assert_eq!(result.response.output_text, "pending");
        assert!(result.errors.is_empty());
    }

    #[test]
    fn poll_step_transient_error_then_success() {
        let mut attempts = 0;
        let mut calls = 0;
        let result = run_poll_step(
            || {
                calls += 1;
                if calls == 1 {
                    Err(anyhow!("temporary failure"))
                } else {
                    attempts += 1;
                    Ok((mk_poll_response("complete", Some(0)), Some(200), Some(0)))
                }
            },
            |resp, status| Ok(resp.output_text == "complete" && status == Some(200)),
            1,
            50,
        )
        .expect("poll should recover after transient error");
        assert_eq!(attempts, 1);
        assert!(result.satisfied);
        assert_eq!(result.attempts, 2);
        assert!(result.response.output_text == "complete");
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.errors.len(), 1);
    }
}
