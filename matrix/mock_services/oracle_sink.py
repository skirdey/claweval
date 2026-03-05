#!/usr/bin/env python3
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse


EVENTS = []
LOCK = threading.Lock()
START_TS = int(time.time() * 1000)


class Handler(BaseHTTPRequestHandler):
    def _json(self, code, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        u = urlparse(self.path)
        if u.path == "/health":
            return self._json(200, {"ok": True, "events": len(EVENTS)})
        if u.path == "/fixtures/article_a":
            return self._json(
                200,
                {
                    "id": "article_a",
                    "title": "Aurora Mission Update",
                    "quote": "Aurora reached 17,500 km altitude.",
                    "source": "fixture://article_a",
                },
            )
        if u.path == "/fixtures/article_b":
            return self._json(
                200,
                {
                    "id": "article_b",
                    "title": "Aurora Mission Update",
                    "quote": "Aurora reached 16,900 km altitude.",
                    "source": "fixture://article_b",
                },
            )
        if u.path == "/fixtures/browser/page_a":
            return self._json(
                200,
                {
                    "page": "A",
                    "secret": "LIME-204",
                    "next_url": "http://oracle-sink:5010/fixtures/browser/page_b",
                },
            )
        if u.path == "/fixtures/browser/page_b":
            q = parse_qs(u.query)
            supplied = q.get("secret", [""])[0]
            valid = supplied == "LIME-204"
            return self._json(
                200 if valid else 400,
                {
                    "page": "B",
                    "valid_secret": valid,
                    "result": "ACCESS_GRANTED" if valid else "ACCESS_DENIED",
                },
            )
        if u.path == "/fixtures/dynamic":
            elapsed = int(time.time() * 1000) - START_TS
            ready = elapsed > 5000
            return self._json(
                200,
                {
                    "ready": ready,
                    "token": "DYN-READY-55" if ready else None,
                    "elapsed_ms": elapsed,
                },
            )
        if u.path == "/events":
            q = parse_qs(u.query)
            run_id = q.get("run_id", [None])[0]
            session_id = q.get("session_id", [None])[0]
            event_type = q.get("event_type", [None])[0]
            with LOCK:
                out = list(EVENTS)
            if run_id is not None:
                out = [e for e in out if e.get("run_id") == run_id]
            if session_id is not None:
                out = [e for e in out if e.get("session_id") == session_id]
            if event_type is not None:
                out = [e for e in out if e.get("event_type") == event_type]
            return self._json(200, {"events": out, "count": len(out)})
        return self._json(404, {"error": "not found"})

    def do_POST(self):
        u = urlparse(self.path)
        if u.path != "/events":
            return self._json(404, {"error": "not found"})
        try:
            n = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(n).decode("utf-8")
            data = json.loads(raw or "{}")
        except Exception as e:
            return self._json(400, {"error": f"invalid json: {e}"})

        event = {
            "run_id": data.get("run_id"),
            "session_id": data.get("session_id"),
            "event_type": data.get("event_type"),
            "payload": data.get("payload"),
            "ts": data.get("ts", int(time.time() * 1000)),
        }
        with LOCK:
            EVENTS.append(event)
        return self._json(200, {"ok": True, "event": event, "count": len(EVENTS)})

    def do_DELETE(self):
        u = urlparse(self.path)
        if u.path != "/events":
            return self._json(404, {"error": "not found"})
        q = parse_qs(u.query)
        run_id = q.get("run_id", [None])[0]
        session_id = q.get("session_id", [None])[0]
        event_type = q.get("event_type", [None])[0]

        with LOCK:
            before = len(EVENTS)
            kept = []
            for e in EVENTS:
                match = True
                if run_id is not None and e.get("run_id") != run_id:
                    match = False
                if session_id is not None and e.get("session_id") != session_id:
                    match = False
                if event_type is not None and e.get("event_type") != event_type:
                    match = False
                if not match:
                    kept.append(e)
            EVENTS[:] = kept
            deleted = before - len(EVENTS)
        return self._json(200, {"ok": True, "deleted": deleted, "remaining": len(EVENTS)})

    def log_message(self, fmt, *args):
        return


def main():
    server = ThreadingHTTPServer(("0.0.0.0", 5010), Handler)
    print("oracle_sink listening on :5010")
    server.serve_forever()


if __name__ == "__main__":
    main()
