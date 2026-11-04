import io
import json
import threading
import time

from flickfluent_sidecar.protocol import Cancelled, Server, WorkerError, atomic_write


class Pipe:
    """Feeds lines to the server's stdin and collects what it prints."""

    def __init__(self, workers, roots):
        import os

        r, w = os.pipe()
        self.stdin_w = os.fdopen(w, "w")
        self.out = io.StringIO()
        self.server = Server(workers, roots, stdin=os.fdopen(r, "r"), stdout=self.out)
        self.thread = threading.Thread(target=self.server.serve, daemon=True)
        self.thread.start()

    def send(self, obj):
        self.stdin_w.write(json.dumps(obj) + "\n")
        self.stdin_w.flush()

    def lines(self):
        return [json.loads(x) for x in self.out.getvalue().splitlines() if x]

    def wait_for(self, pred, timeout=5):
        end = time.time() + timeout
        while time.time() < end:
            for m in self.lines():
                if pred(m):
                    return m
            time.sleep(0.02)
        raise AssertionError(f"timeout; got {self.lines()}")

    def close(self):
        self.send({"id": "bye", "cmd": "shutdown"})
        self.thread.join(timeout=5)


def echo(ctx, req):
    ctx.progress(0.5)
    return {"result": {"echo": req["input"]}, "files": []}


def slow(ctx, req):
    for _ in range(500):
        ctx.check_cancel()
        time.sleep(0.01)
    return {"result": {}}


def boom(ctx, req):
    raise WorkerError("STAGE_BROKE", "no", retryable=True)


def make(tmp_path):
    return Pipe({"echo": echo, "slow": slow, "boom": boom}, [str(tmp_path)])


def test_ready_then_handshake(tmp_path):
    p = make(tmp_path)
    ready = p.wait_for(lambda m: m.get("event") == "ready")
    assert ready["protocol"] == 1 and "echo" in ready["workers"]
    p.send({"id": "h1", "cmd": "hello", "protocol": 1})
    assert p.wait_for(lambda m: m.get("id") == "h1")["ok"] is True
    p.send({"id": "h2", "cmd": "hello", "protocol": 99})
    assert p.wait_for(lambda m: m.get("id") == "h2")["error"]["code"] == "PROTOCOL_MISMATCH"
    p.close()


def test_run_reports_progress_result_and_stats(tmp_path):
    p = make(tmp_path)
    p.send({"id": "r1", "cmd": "run", "worker": "echo", "input": {"x": 1}, "output": {"dir": str(tmp_path)}})
    done = p.wait_for(lambda m: m.get("id") == "r1" and "ok" in m)
    assert done["ok"] and done["result"] == {"echo": {"x": 1}}
    assert "wallMs" in done["stats"]
    assert any(m.get("event") == "progress" for m in p.lines())
    p.close()


def test_cancel_stops_within_five_seconds(tmp_path):
    p = make(tmp_path)
    p.send({"id": "r2", "cmd": "run", "worker": "slow", "input": {}, "output": {"dir": str(tmp_path)}})
    time.sleep(0.1)
    start = time.time()
    p.send({"cmd": "cancel", "target": "r2"})
    done = p.wait_for(lambda m: m.get("id") == "r2" and "ok" in m)
    assert done["error"]["code"] == "CANCELLED"
    assert time.time() - start < 5
    p.close()


def test_paths_outside_allowed_folders_are_rejected(tmp_path):
    p = make(tmp_path)
    outside = str(tmp_path.parent / "elsewhere")
    p.send({"id": "r3", "cmd": "run", "worker": "echo", "input": {}, "output": {"dir": outside}})
    assert p.wait_for(lambda m: m.get("id") == "r3" and "ok" in m)["error"]["code"] == "PATH_NOT_ALLOWED"
    p.send({"id": "r4", "cmd": "run", "worker": "echo", "input": {"audio": "relative.wav"}, "output": {"dir": str(tmp_path)}})
    assert p.wait_for(lambda m: m.get("id") == "r4" and "ok" in m)["error"]["code"] == "PATH_NOT_ALLOWED"
    p.send({"id": "r5", "cmd": "run", "worker": "echo", "input": {}, "models": {"main": "/etc"}, "output": {"dir": str(tmp_path)}})
    assert p.wait_for(lambda m: m.get("id") == "r5" and "ok" in m)["error"]["code"] == "PATH_NOT_ALLOWED"
    p.close()


def test_worker_error_keeps_code_and_retryable(tmp_path):
    p = make(tmp_path)
    p.send({"id": "r6", "cmd": "run", "worker": "boom", "input": {}, "output": {"dir": str(tmp_path)}})
    err = p.wait_for(lambda m: m.get("id") == "r6" and "ok" in m)["error"]
    assert err["code"] == "STAGE_BROKE" and err["retryable"] is True
    p.close()


def test_atomic_write_leaves_no_part_file(tmp_path):
    target = tmp_path / "out.json"
    atomic_write(target, b"{}")
    assert target.read_bytes() == b"{}"
    assert not (tmp_path / "out.json.part").exists()


def test_cancelled_is_an_exception_workers_can_raise():
    assert issubclass(Cancelled, Exception)
