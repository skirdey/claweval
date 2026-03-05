# ClawEval

Composable eval runner for long-running, tool-using AI assistants. Suite > episode > step > check.

## Eval results (2026-02-23)

From [`reports/matrix/run_20260223_150924/_matrix_summary.md`](reports/matrix/run_20260223_150924/_matrix_summary.md):

| Rank | Agent | Pass rate | Passed/Total |
|------|-------|-----------|--------------|
| 1 | picoclaw | 100.0% | 14/14 |
| 2 | openclaw | 92.9% | 13/14 |
| 3 | openai_direct | 92.9% | 13/14 |
| 4 | ironclaw | 92.9% | 13/14 |
| 5 | nanobot | 85.7% | 12/14 |

## Eval suite descriptions

- `matrix_basic` (4 episodes): single-turn reliability checks (`PONG`, echo, factual answer, non-refusal).
- `matrix_memory` (3 episodes): multi-turn memory and state retention across a session.
- `matrix_reasoning` (4 episodes): arithmetic, logic, word problems, plus one `llm_judge` quality check.
- `matrix_structured` (3 episodes): structured output checks for exact JSON, schema compliance, and JSON array format.

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
