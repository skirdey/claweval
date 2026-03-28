# ClawEval

Composable eval runner for long-running, tool-using AI assistants. Suite > episode > step > check.

## Eval results (2026-03-28)

From [`reports/matrix/run_20260328_133737/_matrix_summary.md`](reports/matrix/run_20260328_133737/_matrix_summary.md):

| Rank | Agent | Pass Rate | Passed/Total | Avg Duration |
|------|-------|-----------|--------------|--------------|
| 1 | hermes | 94.4% | 17/18 | 23044ms |
| 2 | openclaw | 88.9% | 16/18 | 30018ms |
| 3 | nanobot | 83.3% | 15/18 | 20805ms |
| 4 | openai_direct | 83.3% | 15/18 | 56629ms |
| 5 | picoclaw | 83.3% | 15/18 | 64774ms |
| 6 | ironclaw | 66.7% | 12/18 | 27207ms |

All agents evaluated against `qwen/qwen3.5-397b-a17b` via OpenRouter.

### Per-suite breakdown

| Suite | hermes | openclaw | nanobot | openai_direct | picoclaw | ironclaw |
|-------|--------|----------|---------|---------------|----------|----------|
| matrix_basic | 4/4 | 4/4 | 4/4 | 4/4 | 4/4 | 4/4 |
| matrix_memory | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 | 2/3 |
| matrix_structured | 3/3 | 3/3 | 2/3 | 3/3 | 3/3 | 2/3 |
| matrix_longhorizon_reliability | 2/2 | 1/2 | 2/2 | 2/2 | 2/2 | 1/2 |
| matrix_async_simulated | 3/3 | 3/3 | 3/3 | 2/3 | 2/3 | 2/3 |
| matrix_tools_grounding | 2/3 | 2/3 | 1/3 | 1/3 | 1/3 | 1/3 |

## Eval suite descriptions

- `matrix_basic` (4 episodes): single-turn reliability checks (`PONG`, echo, factual answer, non-refusal).
- `matrix_memory` (3 episodes): multi-turn memory and state retention across a session.
- `matrix_reasoning` (4 episodes): arithmetic, logic, word problems, plus one `llm_judge` quality check.
- `matrix_structured` (3 episodes): structured output checks for exact JSON, schema compliance, and JSON array format.
- `matrix_async_simulated` (3 episodes): exec probes, oracle event writes, and polling for dynamic readiness.
- `matrix_tools_grounding` (3 episodes): oracle HTTP probes, deterministic fetch, and conflict resolution with uncertainty.
- `matrix_longhorizon_reliability` (2 episodes): ten-turn secret retention and counter state drift across long sessions.

## Features

- **Backends:** OpenClaw, HTTP/OpenAI, generic command
- **Checks:** `contains`, `regex`, `equals_trim`, `json_pointer_equals`, `json_pointer_exists`, `latency_under_ms`, `within_time_window_ms`, `status_code_equals`, `eventually_contains`, `llm_judge`
- **Step types:** `user`, `sleep`, `note`, `exec`, `http_probe`, `poll`
- Parallel execution, pass-rate stats with Wilson 95% CI

## Install

```bash
cargo build --release
```

## Run

```bash
claweval run suites/openclaw_basic.json --out reports/report.json

# all options
claweval run suites/openclaw_basic.json \
  --local --profile claweval --repeats 3 --enable-llm-judge \
  --out reports/report.json
```

## Suite format

See [`suites/`](suites/) for examples. Episodes contain steps (user messages, sleeps) and checks (assertions on responses).

## Custom backends

Use the `command` backend to evaluate any executable without writing Rust:

```json
{
  "type": "command",
  "command": "myagent",
  "args": ["--session", "{session}", "--message", "{message}"]
}
```

Or implement `AgentBackend` in [`src/backend/mod.rs`](src/backend/mod.rs).
