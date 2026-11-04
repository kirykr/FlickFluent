"""Language of the first speech in a window, with a confidence."""

from __future__ import annotations

from typing import Any, Dict

from ..protocol import Context
from .common import model_dir, normalise_code, require, window_ms


def run(ctx: Context, req: Dict[str, Any]) -> Dict[str, Any]:
    fw = require("faster_whisper")
    sf = require("soundfile")
    model = fw.WhisperModel(
        str(model_dir(ctx, req)),
        device=ctx.params.get("device", "cpu"),
        compute_type=ctx.params.get("computeType", "int8"),
        cpu_threads=int(ctx.params.get("threads", 0)),
    )
    start, end = window_ms(req)
    audio_path = ctx.path(req["input"]["audio"])
    info = sf.info(str(audio_path))
    first = int(start * info.samplerate / 1000)
    frames = int((end - start) * info.samplerate / 1000) if end > start else -1
    data, _ = sf.read(str(audio_path), start=first, frames=frames, dtype="float32", always_2d=False)
    ctx.progress(0.3)
    language, probability, _ = model.detect_language(data, vad_filter=True)
    ctx.progress(1.0)
    return {"result": {"language": normalise_code(language, ctx.params.get("codeMap")), "confidence": round(float(probability), 3)}}
