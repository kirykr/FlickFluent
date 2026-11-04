"""Text to speech with sherpa-onnx: Piper (VITS) and Kokoro voices, one wav per line."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Dict, List

from ..protocol import Context, WorkerError
from .common import require

# Speaking faster to fit a slot is allowed up to this factor; beyond it the line is flagged
# `tooLong` by the core instead of sounding rushed.
MAX_SPEEDUP = 1.3


def find_voice_files(folder: Path, espeak: Path | None = None) -> Dict[str, Any]:
    """Describe a voice folder: Kokoro has `voices.bin`, Piper has a single `.onnx` and tokens."""
    onnx = sorted(p for p in folder.glob("*.onnx"))
    tokens = folder / "tokens.txt"
    if not onnx or not tokens.exists():
        raise WorkerError("MODEL_MISSING", f"not a voice folder: {folder}")
    # The pronunciation data is its own download shared by all Piper voices (`models.espeak`);
    # a voice folder that carries its own copy still works.
    data_dir = espeak or folder / "espeak-ng-data"
    return {
        "kind": "kokoro" if (folder / "voices.bin").exists() else "vits",
        "model": onnx[0],
        "tokens": tokens,
        "voices": folder / "voices.bin",
        "data_dir": data_dir if data_dir.exists() else None,
        "lexicon": next(iter(folder.glob("lexicon*.txt")), None),
    }


def build_tts(so, files: Dict[str, Any], threads: int):
    if files["kind"] == "kokoro":
        model = so.OfflineTtsModelConfig(
            kokoro=so.OfflineTtsKokoroModelConfig(
                model=str(files["model"]),
                voices=str(files["voices"]),
                tokens=str(files["tokens"]),
                data_dir=str(files["data_dir"] or ""),
                lexicon=str(files["lexicon"] or ""),
            ),
            num_threads=threads,
        )
    else:
        model = so.OfflineTtsModelConfig(
            vits=so.OfflineTtsVitsModelConfig(
                model=str(files["model"]), tokens=str(files["tokens"]), data_dir=str(files["data_dir"] or "")
            ),
            num_threads=threads,
        )
    return so.OfflineTts(so.OfflineTtsConfig(model=model))


def run(ctx: Context, req: Dict[str, Any]) -> Dict[str, Any]:
    so = require("sherpa_onnx")
    sf = require("soundfile")
    np = require("numpy")
    lines: List[dict] = req["input"]["lines"]
    out_dir = ctx.path(req["output"]["dir"])
    threads = int(ctx.params.get("threads", 2)) or 2
    voices: Dict[str, Any] = {}
    results, names = [], []

    for i, line in enumerate(lines):
        ctx.check_cancel()
        voice_id = line["voiceId"]
        if voice_id not in voices:
            folder = (req.get("models") or {}).get(voice_id)
            if not folder:
                raise WorkerError("MODEL_MISSING", f"no model for voice {voice_id}")
            espeak = (req.get("models") or {}).get("espeak")
            files = find_voice_files(ctx.path(folder), ctx.path(espeak) if espeak else None)
            voices[voice_id] = (build_tts(so, files, threads), int(line.get("speakerId", 0)))
        tts, sid = voices[voice_id]
        slot_ms = max(int(line.get("targetMs", 0)), 1)

        speed = 1.0
        audio = tts.generate(line["text"], sid=sid, speed=speed)
        ms = int(len(audio.samples) * 1000 / audio.sample_rate)
        if ms > slot_ms * 1.25:  # one retry, faster, within the allowed limit
            speed = min(ms / slot_ms, MAX_SPEEDUP)
            audio = tts.generate(line["text"], sid=sid, speed=speed)
            ms = int(len(audio.samples) * 1000 / audio.sample_rate)
        samples = np.asarray(audio.samples, dtype="float32")

        name = f"{line['id']}.wav"
        target = out_dir / name
        part = target.with_name(name + ".part")
        sf.write(str(part), samples, audio.sample_rate, subtype="PCM_16", format="WAV")
        import os

        os.replace(part, target)
        names.append(name)
        results.append({"id": line["id"], "audioMs": ms, "sampleRate": audio.sample_rate, "speed": round(speed, 2)})
        ctx.item("clip", results[-1])
        ctx.progress((i + 1) / len(lines))
    return {"result": {"clips": results}, "files": names}
