#!/usr/bin/env python3
import hashlib
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse


EVENTS = []
UPLOADS = {}  # upload_id -> {"data": bytes, "size": int, "sha256": str, "content_type": str, "ts": int}
LOCK = threading.Lock()
START_TS = int(time.time() * 1000)
COUNTERS = {}  # per-key request counters for /fixtures/flaky endpoint
DISCOVERY_ITEMS = [
    {"id": 1, "name": "Laptop Stand", "price": 49.99, "category": "Office", "in_stock": True},
    {"id": 2, "name": "USB-C Hub", "price": 35.00, "category": "Electronics", "in_stock": True},
    {"id": 3, "name": "Desk Pad", "price": 24.95, "category": "Office", "in_stock": False},
    {"id": 4, "name": "Monitor Light", "price": 59.99, "category": "Electronics", "in_stock": True},
    {"id": 5, "name": "Cable Clips", "price": 8.99, "category": "Office", "in_stock": True},
]
DISCOVERY_NEXT_ID = 6


class Handler(BaseHTTPRequestHandler):
    def _json(self, code, payload):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _html(self, code, html):
        body = html.encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        u = urlparse(self.path)
        if u.path == "/health":
            return self._json(200, {"ok": True, "events": len(EVENTS)})

        # --- Fixtures ---
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
        if u.path == "/fixtures/browser/target_page":
            return self._html(
                200,
                "<!DOCTYPE html>\n"
                "<html><head><title>ClawEval Target Page</title></head>\n"
                '<body style="background:white;font-family:monospace;padding:40px">\n'
                '<h1 id="marker">CLAWEVAL-BROWSER-MARKER-7742</h1>\n'
                "<p>This page is served by the oracle-sink fixture server.</p>\n"
                "</body></html>\n",
            )
        if u.path == "/fixtures/browser/js_challenge":
            return self._html(
                200,
                "<!DOCTYPE html>\n"
                "<html><head><title>JS Challenge</title></head>\n"
                "<body>\n"
                '<div id="token" style="display:none">JSTOKEN-CLAWEVAL-9981</div>\n'
                "<p>Waiting for token...</p>\n"
                "<script>\n"
                "setTimeout(function() {\n"
                '  document.getElementById("token").style.display = "block";\n'
                '  document.querySelector("p").textContent = "Token revealed.";\n'
                "}, 2000);\n"
                "</script>\n"
                "</body></html>\n",
            )

        # --- Flaky endpoint: returns 503 until Nth request, then 200 ---
        if u.path.startswith("/fixtures/flaky/"):
            key = u.path[len("/fixtures/flaky/"):]
            q = parse_qs(u.query)
            threshold = int(q.get("after", ["3"])[0])
            with LOCK:
                COUNTERS[key] = COUNTERS.get(key, 0) + 1
                count = COUNTERS[key]
            if count < threshold:
                return self._json(503, {
                    "ready": False,
                    "attempt": count,
                    "needs": threshold,
                    "retry_after_seconds": 1,
                })
            return self._json(200, {
                "ready": True,
                "token": "FLAKY-OK-" + key.upper(),
                "attempt": count,
            })

        # --- Uploads ---
        if u.path.startswith("/uploads/"):
            upload_id = u.path[len("/uploads/"):]
            with LOCK:
                entry = UPLOADS.get(upload_id)
            if entry is None:
                return self._json(404, {"error": "not found", "upload_id": upload_id})
            return self._json(200, {
                "ok": True,
                "upload_id": upload_id,
                "size": entry["size"],
                "sha256": entry["sha256"],
                "content_type": entry["content_type"],
                "ts": entry["ts"],
            })

        # --- Events ---
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

        # --- API v1: Discovery endpoints ---
        if u.path in ("/api/v1", "/api/v1/"):
            return self._json(200, {
                "service": "inventory",
                "version": "1.3.0",
                "docs": "not available",
                "hint": "try GET /api/v1/status for service info",
            })
        if u.path == "/api/v1/status":
            uptime = (int(time.time() * 1000) - START_TS) // 1000
            with LOCK:
                item_count = len(DISCOVERY_ITEMS)
            return self._json(200, {
                "service": "inventory",
                "version": "1.3.0",
                "uptime_seconds": uptime,
                "total_items": item_count,
                "endpoints": [
                    "GET /api/v1/items",
                    "GET /api/v1/items/{id}",
                    "POST /api/v1/items",
                    "DELETE /api/v1/items/{id}",
                ],
            })
        if u.path == "/api/v1/items":
            q = parse_qs(u.query)
            page = int(q.get("page", ["1"])[0])
            per_page = int(q.get("per_page", ["10"])[0])
            category = q.get("category", [None])[0]
            with LOCK:
                items = [dict(i) for i in DISCOVERY_ITEMS]
            if category:
                items = [i for i in items if i["category"].lower() == category.lower()]
            start = (page - 1) * per_page
            page_items = items[start:start + per_page]
            return self._json(200, {
                "items": page_items,
                "total": len(items),
                "page": page,
                "per_page": per_page,
            })
        if u.path.startswith("/api/v1/items/"):
            item_id_str = u.path[len("/api/v1/items/"):]
            try:
                item_id = int(item_id_str)
            except ValueError:
                return self._json(400, {"error": "invalid item id", "received": item_id_str})
            with LOCK:
                item = next((dict(i) for i in DISCOVERY_ITEMS if i["id"] == item_id), None)
            if item is None:
                return self._json(404, {"error": "item not found", "id": item_id})
            return self._json(200, item)

        return self._json(404, {
            "error": "not found",
            "available_paths": [
                "/health", "/events", "/fixtures/*", "/uploads/*",
                "/api/v1/status", "/api/v1/items",
            ],
        })

    def do_POST(self):
        u = urlparse(self.path)
        if u.path.startswith("/uploads/"):
            upload_id = u.path[len("/uploads/"):]
            n = int(self.headers.get("Content-Length", "0"))
            data = self.rfile.read(n) if n > 0 else b""
            content_type = self.headers.get("Content-Type", "application/octet-stream")
            sha = hashlib.sha256(data).hexdigest()
            entry = {
                "data": data,
                "size": len(data),
                "sha256": sha,
                "content_type": content_type,
                "ts": int(time.time() * 1000),
            }
            with LOCK:
                UPLOADS[upload_id] = entry
            return self._json(200, {
                "ok": True,
                "upload_id": upload_id,
                "size": entry["size"],
                "sha256": sha,
                "content_type": content_type,
            })
        if u.path == "/api/v1/items":
            try:
                n = int(self.headers.get("Content-Length", "0"))
                raw = self.rfile.read(n).decode("utf-8")
                data = json.loads(raw or "{}")
            except Exception as e:
                return self._json(400, {"error": f"invalid json: {e}"})
            required = ["name", "price", "category"]
            missing = [f for f in required if f not in data]
            if missing:
                return self._json(400, {"error": "missing required fields", "missing": missing})
            global DISCOVERY_NEXT_ID
            with LOCK:
                item = {
                    "id": DISCOVERY_NEXT_ID,
                    "name": data["name"],
                    "price": data["price"],
                    "category": data["category"],
                    "in_stock": data.get("in_stock", True),
                }
                DISCOVERY_ITEMS.append(item)
                DISCOVERY_NEXT_ID += 1
            return self._json(201, item)
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
        if u.path.startswith("/uploads/"):
            upload_id = u.path[len("/uploads/"):]
            with LOCK:
                removed = UPLOADS.pop(upload_id, None)
            if removed is None:
                return self._json(404, {"error": "not found", "upload_id": upload_id})
            return self._json(200, {"ok": True, "deleted": upload_id})
        if u.path.startswith("/fixtures/flaky/"):
            key = u.path[len("/fixtures/flaky/"):]
            with LOCK:
                COUNTERS.pop(key, None)
            return self._json(200, {"ok": True, "reset": key})
        if u.path.startswith("/api/v1/items/"):
            item_id_str = u.path[len("/api/v1/items/"):]
            try:
                item_id = int(item_id_str)
            except ValueError:
                return self._json(400, {"error": "invalid item id"})
            with LOCK:
                idx = next((i for i, item in enumerate(DISCOVERY_ITEMS) if item["id"] == item_id), None)
                if idx is None:
                    return self._json(404, {"error": "item not found", "id": item_id})
                removed = DISCOVERY_ITEMS.pop(idx)
            return self._json(200, {"ok": True, "deleted": removed})
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
