# ClawEval (Rust) — composable evals for long-running agentic assistants

ClawEval is a **suite/episode/step/check** evaluation runner designed for **single-thread, long-running assistants** that use tools and maintain state (e.g., OpenClaw).

This repository ships with:

- A Rust CLI: `claweval run suites/openclaw_basic.json`
- An **OpenClaw backend** that drives `openclaw agent` via CLI
- A **generic command backend** so you can evaluate other frameworks without writing Rust code
- Deterministic checks (contains/regex/equality/json-pointer/latency)
- Optional **LLM-as-judge** checks (`llm_judge`) using the selected backend itself as the judge
- Basic statistical summaries (pass rate + Wilson 95% CI)

> NOTE: This repo is "source-only". You need a Rust toolchain to build.

## Install

1) Install Rust (stable) using `rustup`.
2) Build:

```bash
cargo build --release
```

Binary will be at `target/release/claweval`.

## Run an OpenClaw suite

OpenClaw must be installed and available as `openclaw` on PATH.

```bash
./target/release/claweval run suites/openclaw_basic.json --out reports/report.json
```

Useful overrides:

```bash
./target/release/claweval run suites/openclaw_basic.json \
  --local \
  --profile claweval \
  --repeats 3 \
  --enable-llm-judge \
  --out reports/report.json
```

## Suite JSON format (high level)

```json
{
  "name": "basic",
  "backend": {
    "type": "openclaw",
    "openclaw_bin": "openclaw",
    "local": true,
    "profile": "claweval",
    "json": true
  },
  "global_repeats": 2,
  "episodes": [
    {
      "id": "memory",
      "repeats": 1,
      "steps": [
        {"type": "user", "input": "Remember: TANGERINE-742"},
        {"type": "user", "input": "What did I ask you to remember?"}
      ],
      "checks": [
        {"type": "equals_trim", "step": 1, "text": "TANGERINE-742"}
      ]
    }
  ]
}
```

### Steps

- `{"type":"user","input":"..."}` sends a message to the agent.
- `{"type":"sleep","ms":500}` sleeps (for async/latency oriented scenarios).
- `{"type":"note","text":"..."}` no-op marker.

### Checks

- `contains`: substring
- `regex`: Rust regex
- `equals_trim`: exact string match after trimming
- `json_pointer_equals`: check a JSON pointer value against expected JSON (requires backend JSON)
- `latency_under_ms`: step duration constraint
- `llm_judge`: judge rubric (requires `--enable-llm-judge`)

## Clean interfaces: evaluating non-OpenClaw frameworks

### Option A: `command` backend (no Rust changes)

The command backend runs any executable and captures stdout.

**Config**:

- `backend.command`: executable
- `backend.args`: args array with placeholders:
  - `{session}` => unique session id
  - `{message}` => user message

Example: evaluate a hypothetical agent binary that accepts `--session` and `--message`:

```json
{
  "type": "command",
  "command": "myagent",
  "args": ["--session", "{session}", "--message", "{message}"]
}
```

### Option B: implement a native backend in Rust

Implement the trait in `src/backend/mod.rs`:

```rust
pub trait AgentBackend {
  fn name(&self) -> &str;
  fn send(&self, req: SendRequest) -> Result<SendResponse>;
  fn new_session_id(&self) -> String { ... }
}
```

Add it to the factory `build_backend()`.

## Notes / limitations

- This is a **minimal runnable** foundation. For production-grade evals you will likely add:
  - transcript ingestion (JSONL), tool-call scoring, artifact verification
  - deterministic mocks for Gmail/Calendar/Slack
  - temporal assertions over a full event stream

