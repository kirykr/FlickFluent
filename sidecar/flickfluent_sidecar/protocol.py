"""JSON-line protocol between the Rust core and the sidecar.

See specs/002-core-dub-pipeline/contracts/sidecar-protocol.md. One request runs at a time.
stdout carries only protocol lines; anything else the workers print must go to stderr.
"""

from __future__ import annotations

import json
import os
import queue
import sys
import threading
import time
from pathlib import Path
from typing import Any, Callable, Dict, Iterable, List, Optional, TextIO

PROTOCOL_VERSION = 1
# Keys inside "input" that hold file paths and so are checked against the allow-list.
PATH_INPUT_KEYS = ("audio", "vocals", "music", "file", "path")


class Cancelled(Exception):
    """Raised inside a worker when the core asked it to stop."""


class WorkerError(Exception):
    def __init__(self, code: str, message: str = "", retryable: bool = False):
        super().__init__(message or code)
        self.code = code
        self.retryable = retryable


def peak_ram_mb() -> int:
    """Peak resident memory of this process in MB. 0 when it cannot be read."""
    try:
        import resource  # not on Windows

        peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        # macOS reports bytes, Linux reports kilobytes.
        return int(peak / (1024 * 1024)) if sys.platform == "darwin" else int(peak / 1024)
    except ImportError:
        pass
    try:  # Windows
        import ctypes
        from ctypes import wintypes

        class Counters(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
            ]

        counters = Counters()
        counters.cb = ctypes.sizeof(Counters)
        handle = ctypes.windll.kernel32.GetCurrentProcess()
        ctypes.windll.psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb)
        return int(counters.PeakWorkingSetSize / (1024 * 1024))
    except Exception:
        return 0


def atomic_write(path: os.PathLike | str, data: bytes) -> None:
    """Write `path.part`, flush it to disk, then rename. Never leaves a half-written file."""
    target = Path(path)
    part = target.with_name(target.name + ".part")
    with open(part, "wb") as f:
        f.write(data)
        f.flush()
        os.fsync(f.fileno())
    os.replace(part, target)


class Context:
    """What a worker may use: progress, partial items, cancel check, path check."""

    def __init__(self, server: "Server", request_id: str, cancel: threading.Event, params: Dict[str, Any]):
        self._server = server
        self.id = request_id
        self._cancel = cancel
        self.params = params
        self.last_emit = time.monotonic()

    def progress(self, fraction: float) -> None:
        self.last_emit = time.monotonic()
        self._server.send({"id": self.id, "event": "progress", "progress": round(float(fraction), 4)})

    def item(self, kind: str, data: Any) -> None:
        self.last_emit = time.monotonic()
        self._server.send({"id": self.id, "event": "item", "kind": kind, "data": data})

    def check_cancel(self) -> None:
        if self._cancel.is_set():
            raise Cancelled()

    def cancelled(self) -> bool:
        return self._cancel.is_set()

    def path(self, value: str) -> Path:
        return self._server.check_path(value)


Worker = Callable[[Context, Dict[str, Any]], Dict[str, Any]]


class Server:
    def __init__(
        self,
        workers: Dict[str, Worker],
        allowed_roots: Iterable[str],
        stdin: Optional[TextIO] = None,
        stdout: Optional[TextIO] = None,
    ):
        self.workers = workers
        self.roots: List[Path] = [Path(r).resolve() for r in allowed_roots]
        self.stdin = stdin or sys.stdin
        self.stdout = stdout or sys.stdout
        self._lock = threading.Lock()
        self._queue: "queue.Queue[Optional[dict]]" = queue.Queue()
        self._cancels: Dict[str, threading.Event] = {}

    # --- output -------------------------------------------------------------------------
    def send(self, obj: dict) -> None:
        line = json.dumps(obj, ensure_ascii=False, separators=(",", ":"))
        with self._lock:
            self.stdout.write(line + "\n")
            self.stdout.flush()

    # --- paths --------------------------------------------------------------------------
    def check_path(self, value: str) -> Path:
        p = Path(value)
        if not p.is_absolute():
            raise WorkerError("PATH_NOT_ALLOWED", f"not an absolute path: {value}")
        resolved = p.resolve()
        for root in self.roots:
            try:
                resolved.relative_to(root)
                return resolved
            except ValueError:
                continue
        raise WorkerError("PATH_NOT_ALLOWED", f"outside the allowed folders: {value}")

    def _check_request_paths(self, req: dict) -> None:
        out = req.get("output") or {}
        if "dir" in out:
            self.check_path(out["dir"])
        for v in (req.get("models") or {}).values():
            self.check_path(v)
        inp = req.get("input") or {}
        for key in PATH_INPUT_KEYS:
            if isinstance(inp.get(key), str):
                self.check_path(inp[key])

    # --- loop ---------------------------------------------------------------------------
    def _read_stdin(self) -> None:
        for line in self.stdin:
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            if msg.get("cmd") == "cancel":
                ev = self._cancels.get(msg.get("target", ""))
                if ev:
                    ev.set()
                continue
            self._queue.put(msg)
        self._queue.put(None)

    def serve(self) -> None:
        self.send(
            {
                "event": "ready",
                "protocol": PROTOCOL_VERSION,
                "workers": sorted(self.workers),
                "python": ".".join(map(str, sys.version_info[:3])),
            }
        )
        threading.Thread(target=self._read_stdin, daemon=True).start()
        while True:
            msg = self._queue.get()
            if msg is None:
                return
            cmd = msg.get("cmd")
            rid = msg.get("id", "")
            if cmd == "hello":
                if msg.get("protocol") == PROTOCOL_VERSION:
                    self.send({"id": rid, "ok": True})
                else:
                    self.send({"id": rid, "ok": False, "error": {"code": "PROTOCOL_MISMATCH"}})
            elif cmd == "ping":
                self.send({"id": rid, "ok": True})
            elif cmd == "shutdown":
                self.send({"id": rid, "ok": True})
                return
            elif cmd == "run":
                self._run(msg)
            else:
                self.send({"id": rid, "ok": False, "error": {"code": "UNKNOWN_COMMAND", "message": str(cmd)}})

    def _run(self, req: dict) -> None:
        rid = req.get("id", "")
        cancel = threading.Event()
        self._cancels[rid] = cancel
        started = time.monotonic()
        try:
            worker = self.workers.get(req.get("worker", ""))
            if worker is None:
                raise WorkerError("UNKNOWN_WORKER", str(req.get("worker")))
            self._check_request_paths(req)
            ctx = Context(self, rid, cancel, req.get("params") or {})
            out = worker(ctx, req) or {}
            peak = peak_ram_mb()
            self.send(
                {
                    "id": rid,
                    "ok": True,
                    "result": out.get("result", {}),
                    "files": out.get("files", []),
                    "stats": {"wallMs": int((time.monotonic() - started) * 1000), "peakRamMb": peak},
                }
            )
            ceiling = (req.get("params") or {}).get("ramCeilingMb")
            if ceiling and peak > ceiling:
                self.send({"id": rid, "event": "restart_requested"})
        except Cancelled:
            self.send({"id": rid, "ok": False, "error": {"code": "CANCELLED", "retryable": False}})
        except WorkerError as e:
            self.send(
                {"id": rid, "ok": False, "error": {"code": e.code, "message": str(e), "retryable": e.retryable}}
            )
        except Exception as e:  # a worker bug must not kill the loop
            print(f"worker error: {e!r}", file=sys.stderr, flush=True)
            self.send({"id": rid, "ok": False, "error": {"code": "WORKER_FAILED", "message": str(e), "retryable": True}})
        finally:
            self._cancels.pop(rid, None)
