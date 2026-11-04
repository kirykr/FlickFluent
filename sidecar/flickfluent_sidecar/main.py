"""Entry point: `python -m flickfluent_sidecar.main --allow <dir> [--allow <dir> ...]`."""

from __future__ import annotations

import argparse
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


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--allow", action="append", default=[], help="folder the sidecar may read and write")
    args = parser.parse_args(argv)
    # Protocol lines own stdout. Route stray prints from libraries to stderr.
    real_stdout = sys.stdout
    sys.stdout = sys.stderr
    Server(load_workers(), args.allow, stdout=real_stdout).serve()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
