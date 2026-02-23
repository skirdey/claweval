#!/usr/bin/env python3
"""Universal HTTP adapter for ClawEval agent matrix.

Exposes POST /chat {session_id, message} -> {response} for any agent.

Three modes (set via config.json):
  cli          - Spawn subprocess per message with placeholder substitution.
  openai_proxy - Forward to OpenAI-compatible /v1/chat/completions.
  http_proxy   - Forward to an internal HTTP endpoint.

Config is loaded from the path in ADAPTER_CONFIG env var (default: /app/config.json).
"""

import json
import logging
import os
import subprocess
import sys
import threading
import time

from flask import Flask, jsonify, request

app = Flask(__name__)
log = logging.getLogger("adapter")

# ---------- globals ----------

CONFIG: dict = {}
AGENT_NAME: str = "unknown"

# Per-session conversation history (thread-safe).
_sessions: dict[str, list[dict]] = {}
_lock = threading.Lock()


# ---------- config ----------

def load_config():
    global CONFIG, AGENT_NAME
    path = os.environ.get("ADAPTER_CONFIG", "/app/config.json")
    with open(path) as f:
        CONFIG = json.load(f)
    AGENT_NAME = CONFIG.get("agent_name", "unknown")
    log.info("Loaded config from %s  mode=%s  agent=%s", path, CONFIG.get("mode"), AGENT_NAME)


# ---------- session helpers ----------

def get_history(session_id: str) -> list[dict]:
    with _lock:
        return _sessions.setdefault(session_id, [])


def append_history(session_id: str, role: str, content: str):
    with _lock:
        _sessions.setdefault(session_id, []).append({"role": role, "content": content})


# ---------- mode: cli ----------

def handle_cli(session_id: str, message: str) -> str:
    cmd = CONFIG["command"]
    args = [
        a.replace("{message}", message).replace("{session_id}", session_id)
        for a in CONFIG.get("args", [])
    ]
    timeout = CONFIG.get("timeout_seconds", 120)
    env = {**os.environ, **CONFIG.get("env", {})}

    log.debug("cli exec: %s %s", cmd, args)
    try:
        result = subprocess.run(
            [cmd] + args,
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
        )
    except subprocess.TimeoutExpired:
        return "[ADAPTER_ERROR] subprocess timed out"
    except FileNotFoundError:
        return f"[ADAPTER_ERROR] command not found: {cmd}"

    stdout = result.stdout.strip()
    stderr = result.stderr.strip()

    if result.returncode != 0:
        log.warning("cli exit=%d  stderr=%s", result.returncode, stderr[:500])

    # If the CLI outputs JSON with a response field, extract it.
    response_field = CONFIG.get("response_field")
    if response_field and stdout:
        try:
            data = json.loads(stdout)
            if response_field in data:
                return str(data[response_field])
        except json.JSONDecodeError:
            pass

    return stdout if stdout else stderr


# ---------- mode: openai_proxy ----------

def handle_openai_proxy(session_id: str, message: str) -> str:
    import requests as req

    base_url = CONFIG["base_url"].rstrip("/")
    model = CONFIG["model"]
    api_key = CONFIG.get("api_key") or os.environ.get("OPENROUTER_API_KEY", "")
    system_prompt = CONFIG.get("system_prompt", "You are a helpful assistant.")
    temperature = CONFIG.get("temperature", 0.7)
    max_tokens = CONFIG.get("max_tokens", 1024)
    timeout = CONFIG.get("timeout_seconds", 120)

    # Build messages with per-session history.
    append_history(session_id, "user", message)
    history = get_history(session_id)
    messages = [{"role": "system", "content": system_prompt}] + history

    try:
        resp = req.post(
            f"{base_url}/chat/completions",
            headers={
                "Authorization": f"Bearer {api_key}",
                "Content-Type": "application/json",
            },
            json={
                "model": model,
                "messages": messages,
                "temperature": temperature,
                "max_tokens": max_tokens,
            },
            timeout=timeout,
        )
        resp.raise_for_status()
    except req.RequestException as e:
        return f"[ADAPTER_ERROR] openai_proxy request failed: {e}"

    data = resp.json()
    content = data["choices"][0]["message"]["content"]
    append_history(session_id, "assistant", content)
    return content


# ---------- mode: http_proxy ----------

def handle_http_proxy(session_id: str, message: str) -> str:
    import requests as req

    target_url = CONFIG["target_url"]
    timeout = CONFIG.get("timeout_seconds", 120)
    headers = CONFIG.get("proxy_headers", {"Content-Type": "application/json"})

    # Build request body. Supports field name remapping.
    session_field = CONFIG.get("session_field", "session_id")
    message_field = CONFIG.get("message_field", "message")
    response_field = CONFIG.get("response_field", "response")

    body = {session_field: session_id, message_field: message}
    extra = CONFIG.get("extra_body", {})
    body.update(extra)

    try:
        resp = req.post(target_url, headers=headers, json=body, timeout=timeout)
        resp.raise_for_status()
    except req.RequestException as e:
        return f"[ADAPTER_ERROR] http_proxy request failed: {e}"

    data = resp.json()
    return str(data.get(response_field, data))


# ---------- dispatch ----------

MODE_HANDLERS = {
    "cli": handle_cli,
    "openai_proxy": handle_openai_proxy,
    "http_proxy": handle_http_proxy,
}


# ---------- routes ----------

@app.route("/health", methods=["GET"])
def health():
    return jsonify({"status": "ok", "agent": AGENT_NAME})


@app.route("/chat", methods=["POST"])
def chat():
    data = request.get_json(force=True, silent=True) or {}
    session_id = data.get("session_id", "default")
    message = data.get("message", "")

    if not message:
        return jsonify({"error": "message is required"}), 400

    mode = CONFIG.get("mode", "cli")
    handler = MODE_HANDLERS.get(mode)
    if not handler:
        return jsonify({"error": f"unknown mode: {mode}"}), 500

    try:
        response = handler(session_id, message)
    except Exception as e:
        log.exception("handler error")
        return jsonify({"error": str(e)}), 500

    return jsonify({"response": response})


# ---------- main ----------

if __name__ == "__main__":
    logging.basicConfig(
        level=logging.DEBUG if os.environ.get("ADAPTER_DEBUG") else logging.INFO,
        format="%(asctime)s %(name)s %(levelname)s %(message)s",
    )
    load_config()
    port = int(os.environ.get("PORT", "5000"))
    log.info("Starting adapter on port %d  agent=%s  mode=%s", port, AGENT_NAME, CONFIG.get("mode"))
    app.run(host="0.0.0.0", port=port, threaded=True)
