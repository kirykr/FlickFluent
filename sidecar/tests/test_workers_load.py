"""Every worker the core asks for is found by the sidecar, even without the heavy libraries.

This caught a packaging bug: when the app bundled the sidecar, the `workers/` folder was lost and
the sidecar started with no workers ("UNKNOWN_WORKER").
"""

from flickfluent_sidecar.main import load_workers

NEEDED = {"separate", "transcribe", "translate", "synthesize", "detect_language"}


def test_all_workers_are_available():
    assert NEEDED <= set(load_workers())
