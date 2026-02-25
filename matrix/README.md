# ClawEval Agent Matrix

Docker-based evaluation matrix that runs the same test suites against multiple agents for side-by-side comparison.

## Architecture

```
                     matrix_runner.py
                           |
          +--------+-------+-------+--------+
          |        |       |       |        |
      openclaw  ironclaw nanobot picoclaw openai_direct
      :5001     :5002    :5003   :5004    :5005
          |        |       |       |        |
      [adapter.py receives POST /chat, dispatches to agent]
          |        |       |       |        |
      [cli]    [http_   [cli]   [cli]   [openai_
               proxy]                    proxy]
```

Each agent runs in its own Docker container with:
- Own filesystem, network namespace, and process tree
- Dedicated host port (5001-5005)
- Shared `adapter.py` (Flask) handling the HTTP contract
- Config-driven mode: `cli`, `openai_proxy`, or `http_proxy`

ClawEval's existing `HttpBackend` is the universal integration point — zero Rust changes needed.

## Quick Start

### 1. Prerequisites

- Docker + Docker Compose
- Rust toolchain (`cargo build --release` in project root)
- Python 3.11+ with `requests` (`pip install requests`)
- An OpenRouter API key

### 2. Set up environment

```bash
cp matrix/.env.example matrix/.env
# Edit matrix/.env and add your OPENROUTER_API_KEY
```

### 3. Build claweval

```bash
cargo build --release
```

### 4. Run the full matrix

```bash
# Source .env for the runner
export $(cat matrix/.env | xargs)

# Run all agents x all suites
python matrix/matrix_runner.py

# Or run a subset
python matrix/matrix_runner.py --agents openclaw openai_direct --suites matrix_basic.json
```

### 5. View results

Reports are written to `reports/matrix/run_<timestamp>/`:
- Per-agent JSON reports: `<agent>__<suite>.json`
- Aggregated summary: `_matrix_summary.json` and `_matrix_summary.md`

## Single Agent Quick Test

```bash
# Start just one agent
docker compose -f matrix/docker-compose.yml up -d openai-direct

# Health check
curl http://localhost:5005/health

# Send a test message
curl -X POST http://localhost:5005/chat \
  -H 'Content-Type: application/json' \
  -d '{"session_id":"test","message":"Say PONG"}'

# Run one suite
python matrix/matrix_runner.py --agents openai_direct --suites matrix_basic.json --no-docker
```

## Agents

| Agent | Framework | Mode | Port | Description |
|-------|-----------|------|------|-------------|
| openclaw | Node.js (TypeScript) | cli | 5001 | OpenClaw CLI agent with `--local` embedded mode |
| ironclaw | Rust | cli | 5002 | IronClaw agent via single-message CLI with libSQL |
| nanobot | Python | cli | 5003 | Nanobot AI assistant with OpenRouter provider |
| picoclaw | Go | cli | 5004 | PicoClaw agent built from source |
| openai_direct | None (raw API) | openai_proxy | 5005 | Raw LLM baseline — no agent framework, just the model |

## Results (2026-02-23)

5 agents x 4 suites = 20 runs, 14 episodes per agent, all using `anthropic/claude-opus-4-6` via OpenRouter.

### Leaderboard

| Rank | Agent | Pass Rate | Passed | Avg Duration |
|------|-------|-----------|--------|--------------|
| 1 | **picoclaw** | **100.0%** | 14/14 | 11.9s |
| 2 | openclaw | 92.9% | 13/14 | 32.1s |
| 3 | openai_direct | 92.9% | 13/14 | 68.1s |
| 4 | ironclaw | 92.9% | 13/14 | 165.1s |
| 5 | nanobot | 85.7% | 12/14 | 22.0s |

### Per-Suite Breakdown

| Suite | ironclaw | nanobot | openai_direct | openclaw | picoclaw |
|-------|----------|---------|---------------|----------|----------|
| matrix_basic | 4/4 | 4/4 | 3/4 | 4/4 | 4/4 |
| matrix_memory | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 |
| matrix_reasoning | 4/4 | 4/4 | 4/4 | 4/4 | 4/4 |
| matrix_structured | 2/3 | 1/3 | 3/3 | 2/3 | 3/3 |

### Per-Episode Results

| Episode | ironclaw | nanobot | openai_direct | openclaw | picoclaw |
|---------|----------|---------|---------------|----------|----------|
| basic::pong_single_token | pass | pass | pass | pass | pass |
| basic::echo_sentence | pass | pass | **FAIL** | pass | pass |
| basic::factual_answer | pass | pass | pass | pass | pass |
| basic::no_refusal | pass | pass | pass | pass | pass |
| memory::two_turn_codeword | pass | pass | pass | pass | pass |
| memory::five_turn_secret | pass | pass | pass | pass | pass |
| memory::counter_accumulation | pass | pass | pass | pass | pass |
| reasoning::arithmetic | pass | pass | pass | pass | pass |
| reasoning::word_problem | pass | pass | pass | pass | pass |
| reasoning::logic_puzzle | pass | pass | pass | pass | pass |
| reasoning::llm_judge_rewrite | pass | pass | pass | pass | pass |
| structured::json_exact | pass | pass | pass | pass | pass |
| structured::json_schema | pass | **FAIL** | pass | **FAIL** | pass |
| structured::json_array | **FAIL** | **FAIL** | pass | pass | pass |

### Failure Analysis

| Agent | Episode | Cause |
|-------|---------|-------|
| openai_direct | basic::echo_sentence | OpenRouter API timeout (transient network issue, not a capability failure) |
| openclaw | structured::json_schema | Wrapped JSON in markdown code fences — `json_schema` check requires raw JSON |
| nanobot | structured::json_schema | Same — markdown fencing around JSON output |
| ironclaw | structured::json_array | Regex check `\[.*"color".*"hex".*\]` didn't match multiline output |
| nanobot | structured::json_array | Same regex mismatch on valid JSON array output |

## Suites & Episodes

### matrix_basic (4 episodes)

Baseline capabilities every agent should handle.

| Episode | What it tests | Prompt | Check |
|---------|---------------|--------|-------|
| **pong_single_token** | Exact single-token compliance | "Reply with exactly the single word PONG" | `equals_trim("PONG")` |
| **echo_sentence** | Verbatim sentence repetition | "Repeat the following sentence exactly: The quick brown fox..." | `contains("The quick brown fox...")` |
| **factual_answer** | Simple factual recall | "What is the capital of Japan?" | `contains("Tokyo")` |
| **no_refusal** | Willingness to answer benign questions | "Tell me something interesting about Mars" | `not_empty` + `not_contains("I cannot")` |

### matrix_memory (3 episodes)

Multi-turn conversation memory — does the agent retain context across messages within a session?

| Episode | What it tests | Turns | Check |
|---------|---------------|-------|-------|
| **two_turn_codeword** | Store and recall a codeword in 2 turns | "Remember TANGERINE-742" then "What was it?" | `equals_trim("TANGERINE-742")` on turn 2 |
| **five_turn_secret** | Retain a passphrase across 5 turns with 3 distractor questions in between | Store "ZEPHYR-DELTA-7", ask capital of France, translate hello, compute 17x6, then recall | `equals_trim("ZEPHYR-DELTA-7")` on turn 5 |
| **counter_accumulation** | Running arithmetic across 4 turns | Start at 0, add 10, add 25, add 5 — final total? | `equals_trim("40")` on turn 4 |

### matrix_reasoning (4 episodes)

Arithmetic, word problems, logic, and LLM-judged quality.

| Episode | What it tests | Prompt | Check |
|---------|---------------|--------|-------|
| **arithmetic** | Basic math | "Sum of first 10 natural numbers?" | `equals_trim("55")` |
| **word_problem** | Multi-step word problem | 6 apples at $0.50, pay with $5, how much change? | `contains("ANSWER:")` + `contains("$2.00")` |
| **logic_puzzle** | Syllogistic reasoning | "All dogs are animals. Rex is a dog. Is Rex an animal?" | `contains("YES")` |
| **llm_judge_rewrite** | Writing quality via LLM-as-judge | Rewrite a verbose sentence to be concise and professional | `llm_judge(min_score=0.7)` — a separate LLM scores the rewrite on conciseness, tone, and fidelity |

### matrix_structured (3 episodes)

Raw JSON output compliance — can the agent emit valid, parseable structured data?

| Episode | What it tests | Prompt | Check |
|---------|---------------|--------|-------|
| **json_exact** | Emit an exact JSON object | Output `{"ok":true,"n":3}` | `regex` matching the exact shape |
| **json_schema_compliance** | Emit JSON conforming to a schema | Output `{name, score, passed}` with correct types | `json_schema` validation against a Draft-7 schema |
| **json_array_output** | Emit a JSON array of objects | Output 3 `{color, hex}` objects as a JSON array | `regex` matching array-of-objects pattern |

## Adding a New Agent

1. **Copy the template**
   ```bash
   cp -r matrix/agents/_template matrix/agents/my_agent
   ```

2. **Edit `Dockerfile`** — install your agent binary/runtime

3. **Edit `config.json`** — set `agent_name`, `mode`, `command`/`args` or proxy settings

4. **Add to `docker-compose.yml`**
   ```yaml
   my-agent:
     build:
       context: ..
       dockerfile: matrix/agents/my_agent/Dockerfile
     container_name: claweval-my-agent
     ports:
       - "5006:5000"
     environment:
       - OPENROUTER_API_KEY
     networks:
       - evalnet
     restart: "no"
     healthcheck:
       test: ["CMD", "curl", "-sf", "http://localhost:5000/health"]
       interval: 5s
       timeout: 5s
       retries: 12
       start_period: 10s
   ```

5. **Add to `matrix.json`**
   ```json
   {
     "name": "my_agent",
     "service": "my-agent",
     "port": 5006,
     "enabled": true,
     "description": "My custom agent"
   }
   ```

## CLI Options

```
python matrix/matrix_runner.py [OPTIONS]

Options:
  --config PATH          Path to matrix.json (default: matrix/matrix.json)
  --agents NAME [NAME]   Agent names to run (default: all enabled)
  --suites FILE [FILE]   Suite filenames to run (default: all in matrix.json)
  --no-docker            Skip docker compose up/down (containers already running)
  --keep-containers      Don't stop containers after the run
  --no-build             Skip --build flag on docker compose up
  --jobs N               Parallel jobs for claweval (default: 1)
  --enable-llm-judge     Enable LLM judge checks
  --timeout N            Health check timeout in seconds (default: 60)
```

## Adapter Modes

The universal `adapter.py` supports three modes:

- **`cli`** — Spawns a subprocess per message. Placeholder substitution for `{message}` and `{session_id}` in args. Optionally extracts response from JSON stdout via `response_field`.
- **`openai_proxy`** — Forwards to an OpenAI-compatible `/v1/chat/completions` endpoint with client-side per-session conversation history.
- **`http_proxy`** — Forwards to an internal HTTP endpoint with configurable field name mapping.
