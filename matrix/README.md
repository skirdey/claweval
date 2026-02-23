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

| Agent | Mode | Port | Description |
|-------|------|------|-------------|
| openclaw | cli | 5001 | OpenClaw CLI agent (Node.js) |
| ironclaw | http_proxy | 5002 | IronClaw Rust agent + PostgreSQL sidecar |
| nanobot | cli | 5003 | Nanobot Python agent |
| picoclaw | cli | 5004 | PicoClaw Go agent |
| openai_direct | openai_proxy | 5005 | Raw LLM baseline (no agent framework) |

## Suites

| Suite | Episodes | Tests |
|-------|----------|-------|
| matrix_basic | 4 | Echo, single-token, factual, no-refusal |
| matrix_memory | 3 | Codeword recall, 5-turn secret, running counter |
| matrix_reasoning | 4 | Arithmetic, word problem, logic, LLM-judged rewrite |
| matrix_structured | 3 | Exact JSON, schema compliance, JSON array |

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
