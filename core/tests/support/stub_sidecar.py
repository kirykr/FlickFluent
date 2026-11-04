"""A sidecar for core tests: the real protocol with fake workers.

Usage: stub_sidecar.py --allow <dir> [--no-ready-delay]
Behaviour is chosen by the worker name and by `params` in the request.
"""

import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "sidecar"))

from flickfluent_sidecar.protocol import Server, WorkerError, atomic_write  # noqa: E402


def sleep_checking(ctx, seconds):
    end = time.time() + seconds
    while time.time() < end:
        ctx.check_cancel()
        time.sleep(0.01)


def work(ctx, req):
    """Writes one output file, reports progress, optionally sleeps."""
    steps = int(ctx.params.get("steps", 2))
    for i in range(steps):
        ctx.check_cancel()
        ctx.progress((i + 1) / steps)
        sleep_checking(ctx, float(ctx.params.get("stepSeconds", 0)))
    out = Path(req["output"]["dir"]) / (req["output"].get("prefix", "out") + ".json")
    atomic_write(out, json.dumps({"ok": True, "input": req["input"]}).encode())
    return {"result": {"done": True}, "files": [out.name]}


def crash_once(ctx, req):
    marker = Path(ctx.params["marker"])
    if not marker.exists():
        marker.write_text("crashed")
        os._exit(3)
    return work(ctx, req)


def always_crash(ctx, req):
    os._exit(4)


def hang(ctx, req):
    time.sleep(60)


def fail(ctx, req):
    raise WorkerError("STAGE_FAILED", "nope", retryable=False)


workers = {"work": work, "crash_once": crash_once, "always_crash": always_crash, "hang": hang, "fail": fail}

if __name__ == "__main__":
    allow = [a for i, a in enumerate(sys.argv) if i and sys.argv[i - 1] == "--allow"]
    real_stdout = sys.stdout
    sys.stdout = sys.stderr
    Server(workers, allow, stdout=real_stdout).serve()
