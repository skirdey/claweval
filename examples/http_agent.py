#!/usr/bin/env python3
"""Minimal HTTP agent server for testing ClawEval's HTTP backend.

Serves POST /chat with JSON body {"session_id": "...", "message": "..."}.
Returns {"response": "..."}.

Run: python3 examples/http_agent.py
Then: claweval run suites/http_example.json --out reports/http.json
"""

import json
import re
from http.server import BaseHTTPRequestHandler, HTTPServer

# In-memory session state (codeword store per session_id).
_session_codewords: dict[str, str] = {}


def handle_message(session_id: str, msg: str) -> str:
    msg = msg.strip()

    if "single word PONG" in msg or "Reply with exactly the single word PONG" in msg:
        return "PONG"

    m = re.search(r"Remember this codeword exactly:\s*([A-Z0-9\-]+)", msg)
    if m:
        _session_codewords[session_id] = m.group(1)
        return "OK"

    if "What codeword" in msg:
        return _session_codewords.get(session_id, "TANGERINE-742")

    if "Output ONLY valid JSON" in msg or '{"ok":true,"n":3}' in msg:
        return '{"ok":true,"n":3}'

    if "Draft a polite 1-sentence reply" in msg:
        return "Sure—I'll send the deck to you by EOD."

    return "OK"


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/chat":
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b'{"error":"not found"}')
            return

        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length)
        try:
            data = json.loads(body)
        except json.JSONDecodeError:
            self.send_response(400)
            self.end_headers()
            self.wfile.write(b'{"error":"invalid json"}')
            return

        session_id = data.get("session_id", "")
        message = data.get("message", "")
        response_text = handle_message(session_id, message)

        payload = json.dumps({"response": response_text}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, fmt, *args):  # noqa: N802
        pass  # suppress default access logs


def main():
    host, port = "127.0.0.1", 8080
    server = HTTPServer((host, port), Handler)
    print(f"http_agent listening on http://{host}:{port}/chat  (Ctrl-C to stop)")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
