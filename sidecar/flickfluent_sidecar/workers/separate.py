"""Voice and music separation with Demucs `htdemucs`, two stems, CPU baseline.

Audio is processed in short windows with a cross-fade, so memory stays flat however long the
section is. `vocals + music` equals the input within model error; both files have the input's
length.
"""

from __future__ import annotations

from typing import Any, Dict

from ..protocol import Context
from .common import model_dir, require

WINDOW_S = 30
OVERLAP_S = 1.0


def fade_weights(length: int, overlap: int, first: bool, last: bool):
    """Per-sample weights for one window: ramps up over the head overlap and down over the tail
    overlap, so two neighbouring windows add up to exactly 1 across the overlap."""
    import numpy as np

    w = np.ones(length, dtype="float32")
    if not first and overlap > 0:
        w[:overlap] = np.linspace(0.0, 1.0, overlap, endpoint=False, dtype="float32")
    if not last and overlap > 0:
        w[-overlap:] = np.linspace(1.0, 0.0, overlap, endpoint=False, dtype="float32")
    return w


def run(ctx: Context, req: Dict[str, Any]) -> Dict[str, Any]:
    np = require("numpy")
    sf = require("soundfile")
    torch = require("torch")
    pretrained = require("demucs.pretrained")
    apply = require("demucs.apply")

    out = req["output"]
    out_dir = ctx.path(out["dir"])
    audio_path = ctx.path(req["input"]["audio"])
    start_ms = int(req["input"].get("startMs", 0))
    end_ms = int(req["input"].get("endMs", 0))

    threads = int(ctx.params.get("threads", 0))
    if threads:
        torch.set_num_threads(threads)
    model = pretrained.get_model("htdemucs", repo=model_dir(ctx, req))
    model.eval()
    rate = int(model.samplerate)  # 44100
    vocals_index = model.sources.index("vocals")

    info = sf.info(str(audio_path))
    if info.samplerate != rate:
        raise ValueError(f"separation needs {rate} Hz audio, got {info.samplerate}")
    first = int(start_ms * rate / 1000)
    total = int((end_ms - start_ms) * rate / 1000) if end_ms > start_ms else info.frames - first
    total = min(total, info.frames - first)
    window, overlap = WINDOW_S * rate, int(OVERLAP_S * rate)
    step = window - overlap

    vocals_part = out_dir / f"{out['prefix']}-vocals.wav.part"
    music_part = out_dir / f"{out['prefix']}-music.wav.part"
    vocals_name, music_name = f"{out['prefix']}-vocals.wav", f"{out['prefix']}-music.wav"
    tail = None  # the overlapped end of the previous window, waiting to be blended
    written = 0
    try:
        with sf.SoundFile(str(vocals_part), "w", rate, 2, "PCM_16", format="WAV") as vf, sf.SoundFile(
            str(music_part), "w", rate, 2, "PCM_16", format="WAV"
        ) as mf:
            pos = 0
            while pos < total:
                ctx.check_cancel()
                length = min(window, total - pos)
                audio, _ = sf.read(str(audio_path), start=first + pos, frames=length, dtype="float32", always_2d=True)
                if audio.shape[1] == 1:
                    audio = np.repeat(audio, 2, axis=1)
                wav = torch.from_numpy(audio.T.copy())
                ref = wav.mean(0)
                norm = (wav - ref.mean()) / (ref.std() + 1e-8)
                with torch.no_grad():
                    stems = apply.apply_model(model, norm[None], device="cpu", split=True, overlap=0.25, progress=False)[0]
                stems = stems * (ref.std() + 1e-8) + ref.mean()
                vocals = stems[vocals_index].numpy().T
                music = (stems.sum(0) - stems[vocals_index]).numpy().T
                is_first, is_last = pos == 0, pos + length >= total
                w = fade_weights(length, min(overlap, length), is_first, is_last)[:, None]
                v_blend, m_blend = vocals * w, music * w
                if tail is not None:
                    n = tail[0].shape[0]
                    v_blend[:n] += tail[0]
                    m_blend[:n] += tail[1]
                keep = length if is_last else length - overlap
                vf.write(np.clip(v_blend[:keep], -1, 1))
                mf.write(np.clip(m_blend[:keep], -1, 1))
                written += keep
                tail = None if is_last else (v_blend[keep:], m_blend[keep:])
                pos += step if not is_last else total
                ctx.progress(min(pos / total, 0.99))
        import os

        os.replace(vocals_part, out_dir / vocals_name)
        os.replace(music_part, out_dir / music_name)
    except BaseException:
        for p in (vocals_part, music_part):
            p.unlink(missing_ok=True)
        raise
    ctx.progress(1.0)
    return {"result": {"frames": written}, "files": [vocals_name, music_name]}
