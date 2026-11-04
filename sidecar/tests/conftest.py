import sys
from pathlib import Path

# `python -I` ignores PYTHONPATH, so put the package folder on the path here.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import io
import os
import subprocess
import threading

import pytest

from flickfluent_sidecar.protocol import Context, Server

FIXTURES = Path(__file__).resolve().parents[2] / "fixtures"


@pytest.fixture(scope="session")
def models() -> Path:
    """Folder with real model folders (`whisper-tiny`, `m2m100`, `htdemucs`, `voice`, `speaker`).
    Set FLICK_TEST_MODELS to run the tests that need them; they are skipped otherwise."""
    value = os.environ.get("FLICK_TEST_MODELS")
    if not value or not Path(value).is_dir():
        pytest.skip("FLICK_TEST_MODELS not set")
    return Path(value)


@pytest.fixture
def work(tmp_path):
    return tmp_path


@pytest.fixture
def call(models, tmp_path):
    """`call(run, input=..., models={...}, output_prefix=..., params=...)` runs a worker's `run`
    in-process with paths allowed under the temp folder and the models folder."""

    def _call(run, input, model_map=None, prefix="sec0001", params=None):
        server = Server({}, [str(tmp_path), str(models), str(FIXTURES)], stdout=io.StringIO())
        ctx = Context(server, "t1", threading.Event(), params or {})
        req = {
            "input": input,
            "output": {"dir": str(tmp_path), "prefix": prefix},
            "models": {k: str(models / v) for k, v in (model_map or {}).items()},
            "params": params or {},
        }
        return run(ctx, req)

    return _call


def extract_wav(source: Path, out: Path, rate: int, channels: int) -> Path:
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-i", str(source), "-vn", "-ar", str(rate), "-ac", str(channels), "-c:a", "pcm_s16le", str(out)],
        check=True,
    )
    return out
