"""Throwaway M0.3 stand-in for the settings routes M3.1 will add to allotd.

Serves on 127.0.0.1:<port> with the token from $HOME/.allot/token, so run it under a throwaway
HOME. Values live in memory; the revision counts every write.

  GET   /health          no token; {"version": "spike"}
  GET   /settings        {"revision", "values", "defaults", "restart_pending"}
  PATCH /settings        {"revision", "set": {key: value}, "reset": [key]}
                         200 like GET, 409 {"error": "conflict", ...GET} on a stale revision,
                         422 {"errors": [{"key", "message"}]} on an invalid value
  POST  /spike/hand-edit no token; changes admission.headroom_pct as a hand edit would

The file $HOME/settings-spike-mode is read on every request:
  normal        answers as above
  slow          waits 4 s before every answer
  down          closes the connection without an answer
  unauthorized  answers 401 to every request that needs the token
Every request is logged to stdout as one line.
"""

import json
import os
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HOME = Path(os.environ["HOME"])
TOKEN = (HOME / ".allot" / "token").read_text().strip()
MODE_PATH = HOME / "settings-spike-mode"
PORT = int(sys.argv[1])
SLOW_SECS = 4
LOCK = threading.Lock()

DEFAULTS = {
    "admission.headroom_pct": 15,
    "estimates.lower": True,
    "estimates.min_runs": 3,
    "estimates.margin_pct": 10,
    "leases.idle_release_secs": 180,
    "retention.history_days": 90,
    "daemon.port": 47823,
}
RESTART_KEYS = {"daemon.port"}
state = {"revision": 1, "values": dict(DEFAULTS), "running_port": DEFAULTS["daemon.port"]}


def problem(key, value):
    """Why value is not allowed for key, as allotd's Config::validate would say; None if fine."""
    expected = type(DEFAULTS[key])
    if expected is bool:
        return None if isinstance(value, bool) else f"{key} must be true or false"
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return f"{key} must be a number"
    if key == "admission.headroom_pct" and not 0 <= value < 100:
        return "admission.headroom_pct must be at least 0 and below 100"
    if key == "estimates.margin_pct" and not 0 <= value <= 100:
        return "estimates.margin_pct must be 0 to 100"
    if key == "daemon.port" and not 1024 <= value <= 65535:
        return "daemon.port must be 1024 to 65535"
    if key in ("estimates.min_runs", "retention.history_days") and value <= 0:
        return f"{key} must be positive"
    if value < 0:
        return f"{key} must not be negative"
    return None


def snapshot():
    return {
        "revision": f"r{state['revision']}",
        "values": dict(state["values"]),
        "defaults": DEFAULTS,
        "restart_pending": state["values"]["daemon.port"] != state["running_port"],
    }


def mode():
    try:
        return MODE_PATH.read_text().strip()
    except FileNotFoundError:
        return "normal"


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/health":
            self.answer(200, {"version": "spike"}, needs_token=False)
        elif self.path == "/settings":
            with LOCK:
                body = snapshot()
            self.answer(200, body)
        else:
            self.answer(404, {"error": "not found"}, needs_token=False)

    def do_PATCH(self):
        if self.path != "/settings":
            self.answer(404, {"error": "not found"}, needs_token=False)
            return
        try:
            request = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
        except ValueError:
            self.answer(400, {"error": "body is not JSON"})
            return
        changes = request.get("set", {})
        resets = request.get("reset", [])
        with LOCK:
            if request.get("revision") != f"r{state['revision']}":
                status, body = 409, {"error": "conflict", **snapshot()}
            else:
                errors = [
                    {"key": key, "message": "unknown setting"}
                    for key in [*changes, *resets]
                    if key not in DEFAULTS
                ]
                errors += [
                    {"key": key, "message": message}
                    for key, value in changes.items()
                    if key in DEFAULTS and (message := problem(key, value)) is not None
                ]
                if errors:
                    status, body = 422, {"errors": errors}
                else:
                    state["values"].update(changes)
                    for key in resets:
                        state["values"][key] = DEFAULTS[key]
                    if changes or resets:
                        state["revision"] += 1
                    status, body = 200, snapshot()
        self.answer(status, body)

    def do_POST(self):
        if self.path != "/spike/hand-edit":
            self.answer(404, {"error": "not found"}, needs_token=False)
            return
        with LOCK:
            values = state["values"]
            values["admission.headroom_pct"] = (values["admission.headroom_pct"] + 5) % 50
            state["revision"] += 1
            body = snapshot()
        self.answer(200, body, needs_token=False)

    def answer(self, status, body, needs_token=True):
        current = mode()
        if current == "down":
            self.close_connection = True
            return
        if current == "slow":
            time.sleep(SLOW_SECS)
        authorized = self.headers.get("Authorization") == f"Bearer {TOKEN}"
        if needs_token and (current == "unauthorized" or not authorized):
            status, body = 401, {"error": "unauthorized"}
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, format, *args):
        print(f"{time.strftime('%H:%M:%S')} {mode()} {format % args}", flush=True)


ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
