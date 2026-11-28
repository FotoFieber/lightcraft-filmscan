#!/usr/bin/env python3
"""Drive a running app over its control port: convert the loaded scans and take screenshots.

    drive-app.py <port> <out>

Writes <out>/replies.jsonl and <out>/*.png (see docs/control-protocol.md). Exits with 1 when a
request fails.
"""
import json
import socket
import sys
import time
from pathlib import Path

port, out = int(sys.argv[1]), Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
log = (out / "replies.jsonl").open("w")
failures = []


def connect():
    end = time.monotonic() + 90
    while True:
        try:
            return socket.create_connection(("127.0.0.1", port), timeout=90)
        except OSError:
            if time.monotonic() > end:
                raise
            time.sleep(1)


sock = connect()
replies = sock.makefile("r", encoding="utf-8")


def request(method, params=None):
    sock.sendall((json.dumps({"id": 1, "method": method, "params": params or {}}) + "\n").encode())
    reply = json.loads(replies.readline())
    log.write(json.dumps({"method": method, "params": params, "reply": reply}) + "\n")
    log.flush()
    if not reply.get("ok"):
        failures.append(f"{method} {params}: {reply.get('error')}")
        print(f"FAILED {method} {params}: {reply.get('error')}")
    return reply.get("result")


def execute(command, params=None):
    return request("engine.execute", {"command": command, "params": params or {}})


def settle():
    """Wait until imports and renders are done."""
    quiet, end = 0, time.monotonic() + 120
    while quiet < 3:
        state = request("ui.inspect") or {}
        loupe = state.get("loupe") or {}
        busy = state.get("import") or state.get("activity") or loupe.get("pending")
        quiet = 0 if busy else quiet + 1
        if time.monotonic() > end:
            failures.append("the app did not settle within 120 s")
            return
        time.sleep(1)


def dismiss(widget):
    """Click a dialog button if it is on screen."""
    settle()
    found = request("ui.widgets", {"filter": widget})
    if widget in json.dumps(found):
        request("ui.clickWidget", {"id": widget})


def shot(name):
    settle()
    request("ui.screenshot", {"path": str(out / f"{name}.png")})


time.sleep(5)
shot("0-first-start")
# Close what covers the photos on a first start: the welcome tour, then the lens profile offer.
dismiss("button:welcomeSkip")
dismiss("button:dialogCancel")
shot("1-the-negative-scans")
execute("library.selectAll")
execute("develop.negativeZap", {"format": "35mm"})
shot("2-zapped")
request("ui.set", {"view": "detail", "right": "edit"})
shot("3-detail")
state = request("ui.inspect") or {}
print(json.dumps({k: (state.get("perf") or {}).get(k) for k in ("gpu", "gpuReason", "gpuFallback", "mainOnGpu", "fps")}))
(out / "inspect.json").write_text(json.dumps(state, indent=2))
request("app.quit")
if failures:
    print("\n".join(failures))
    sys.exit(1)
