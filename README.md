# ClawEval

Composable eval runner for long-running, tool-using AI assistants. Suite > episode > step > check.

## Features

- **Backends:** OpenClaw, HTTP/OpenAI, generic command
- **Checks:** `contains`, `regex`, `equals_trim`, `json_pointer_equals`, `latency_under_ms`, `llm_judge`
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
