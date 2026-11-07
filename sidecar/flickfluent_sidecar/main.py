"""Entry point: `python -m flickfluent_sidecar.main --allow <dir> [--allow <dir> ...]`."""

from __future__ import annotations

import argparse
import io
import os
import sys

from .protocol import Server


def load_workers() -> dict:
    """Workers import heavy libraries, so each is imported only when the module exists."""
    workers = {}
    for name in ("separate", "transcribe", "translate", "synthesize", "detect_language"):
        try:
            module = __import__(f"flickfluent_sidecar.workers.{name}", fromlist=["run"])
        except ImportError as e:
            print(f"worker {name} unavailable: {e}", file=sys.stderr, flush=True)
            continue
        if hasattr(module, "run"):
            workers[name] = module.run
    return workers


def private_protocol_pipes():
    """Move the protocol pipes off fds 0 and 1 and return them as text streams.

    The server keeps a thread blocked reading stdin. On Windows, a pending read on the standard
    input pipe makes anything else in the process that touches that handle wait for the read to
    end, so loading a Whisper model hung forever and the core reported a crash. Reading from a
    private duplicate (not inheritable) and pointing fd 0 at NUL removes the shared handle. fd 1
    goes to stderr so native libraries that write to stdout cannot corrupt protocol lines.
    """
    proto_in = os.dup(0)
    proto_out = os.dup(1)
    null = os.open(os.devnull, os.O_RDONLY)
    os.dup2(null, 0)
    os.close(null)
    os.dup2(2, 1)
    sys.stdin = open(os.devnull, encoding="utf-8")
    reader = io.open(proto_in, "r", encoding="utf-8", newline=None)
    writer = io.open(proto_out, "w", encoding="utf-8", newline="\n")
    return reader, writer


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--allow", action="append", default=[], help="folder the sidecar may read and write")
    args = parser.parse_args(argv)
    proto_in, proto_out = private_protocol_pipes()
    # Protocol lines own the private stdout. Route stray prints from libraries to stderr.
    sys.stdout = sys.stderr
    Server(load_workers(), args.allow, stdin=proto_in, stdout=proto_out).serve()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
