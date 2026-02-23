use crate::backend::{build_backend, AgentBackend, SendRequest, SendResponse};
use crate::checks;
use crate::printer::Printer;
use crate::report::{
    self, BackendInfo, EpisodeReport, EpisodeRunReport, EpisodeSummary, OverallSummary,
    StepReport, SuiteReport,
};
use crate::spec::{EpisodeSpec, StepSpec, SuiteSpec};
use crate::stats;
use anyhow::{Context, Result};
use rayon::prelude::*;
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
    pub kind: String,
    pub name: Option<String>,
    pub input: Option<String>,
    pub response: Option<SendResponse>,
    pub duration: Duration,
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
        backend: BackendInfo {
            backend_type: backend.name().to_string(),
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

fn run_episode_once(
    ep: &EpisodeSpec,
    run_index: u32,
    backend: &dyn AgentBackend,
    judge_backend: Option<&dyn AgentBackend>,
    opts: &RunOptions,
) -> Result<(EpisodeRunReport, bool)> {
    let session_id = backend.new_session_id();
    let start = Instant::now();

    let mut step_outcomes: Vec<StepOutcome> = Vec::new();

    for (idx, step) in ep.steps.iter().enumerate() {
        let step_start = Instant::now();
        match step {
            StepSpec::User { input, name } => {
                if opts.verbose {
                    eprintln!("[{}:{}] USER> {}", ep.id, idx, input);
                }
                let resp = backend.send(SendRequest {
                    session_id: session_id.clone(),
                    message: input.clone(),
                })?;
                if opts.verbose {
                    eprintln!("[{}:{}] ASSISTANT> {}", ep.id, idx, resp.output_text);
                }
                step_outcomes.push(StepOutcome {
                    index: idx,
                    kind: "user".to_string(),
                    name: name.clone(),
                    input: Some(input.clone()),
                    duration: resp.duration,
                    response: Some(resp),
                });
            }
            StepSpec::Sleep { ms, name } => {
                if opts.verbose {
                    eprintln!("[{}:{}] SLEEP {}ms", ep.id, idx, ms);
                }
                std::thread::sleep(Duration::from_millis(*ms));
                step_outcomes.push(StepOutcome {
                    index: idx,
                    kind: "sleep".to_string(),
                    name: name.clone(),
                    input: Some(format!("sleep {}ms", ms)),
                    response: None,
                    duration: step_start.elapsed(),
                });
            }
            StepSpec::Note { text, name } => {
                if opts.verbose {
                    eprintln!("[{}:{}] NOTE {}", ep.id, idx, text);
                }
                step_outcomes.push(StepOutcome {
                    index: idx,
                    kind: "note".to_string(),
                    name: name.clone(),
                    input: Some(text.clone()),
                    response: None,
                    duration: step_start.elapsed(),
                });
            }
        }
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
