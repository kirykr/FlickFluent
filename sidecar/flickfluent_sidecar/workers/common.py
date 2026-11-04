"""Small helpers the workers share. No model code here, so tests import it without ML libraries."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Dict, Optional

from ..protocol import WorkerError, atomic_write


def require(module_name: str):
    """Import a heavy library, turning a missing one into a clear worker error."""
    try:
        return __import__(module_name, fromlist=["_"])
    except ImportError as e:
        raise WorkerError("RUNTIME_INCOMPLETE", f"{module_name} is not installed in the runtime pack: {e}") from e


def model_dir(ctx, req: Dict[str, Any], key: str = "main") -> Path:
    value = (req.get("models") or {}).get(key)
    if not value:
        raise WorkerError("MODEL_MISSING", f"no '{key}' model in the request")
    path = ctx.path(value)
    if not path.exists():
        raise WorkerError("MODEL_MISSING", f"model folder not found: {path}")
    return path


def window_ms(req: Dict[str, Any]) -> tuple[int, int]:
    inp = req.get("input") or {}
    return int(inp.get("startMs", 0)), int(inp.get("endMs", 0))


def write_json(path: Path, data: Any) -> None:
    atomic_write(path, json.dumps(data, ensure_ascii=False, indent=1).encode("utf-8"))


def normalise_code(code: str, mapping: Optional[Dict[str, str]]) -> str:
    """Map a language code to the one the models use (`iw` to `he`). One mapping file, owned by
    the core (`config/language-codes.json`) and sent in `params.codeMap`."""
    code = (code or "").strip().lower().replace("_", "-")
    if mapping and code in mapping:
        return mapping[code]
    return code
