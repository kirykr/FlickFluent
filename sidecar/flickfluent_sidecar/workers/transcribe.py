"""Speech to text with faster-whisper: lines with times, a speaker per line, confidence.

Times in the result are relative to the start of the window (the core shifts them).
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from ..protocol import Context
from .common import model_dir, normalise_code, require, window_ms, write_json

# A new line starts when the speaker pauses this long, or a sentence ends, or the line gets long.
GAP_S = 0.8
MAX_CHARS = 220
SENTENCE_END = (".", "!", "?", "…", "。", "！", "？", "។")
LOW_CONFIDENCE = 0.5


def split_lines(words: List[dict]) -> List[dict]:
    """Group words (`{start, end, word, probability}`) into lines.

    Always: `end > start`, lines sorted, no overlap between neighbours.
    """
    lines: List[dict] = []
    cur: List[dict] = []

    def flush() -> None:
        if not cur:
            return
        text = "".join(w["word"] for w in cur).strip()
        start, end = cur[0]["start"], cur[-1]["end"]
        if text and end > start:
            probs = [w.get("probability", 1.0) for w in cur]
            lines.append({"start": start, "end": end, "text": text, "confidence": sum(probs) / len(probs)})
        cur.clear()

    for w in words:
        if cur and (w["start"] - cur[-1]["end"] > GAP_S or sum(len(x["word"]) for x in cur) > MAX_CHARS):
            flush()
        cur.append(w)
        if w["word"].strip().endswith(SENTENCE_END):
            flush()
    flush()
    lines.sort(key=lambda l: l["start"])
    for a, b in zip(lines, lines[1:]):
        if b["start"] < a["end"]:  # whisper word times can overlap by a few ms
            a["end"] = max(a["start"] + 0.001, b["start"])
    return lines


def assign_speakers(lines: List[dict], turns: List[dict]) -> None:
    """Give each line the speaker whose turns overlap it most. `turns`: `{start, end, speaker}`."""
    for line in lines:
        overlap: Dict[str, float] = {}
        for t in turns:
            o = min(line["end"], t["end"]) - max(line["start"], t["start"])
            if o > 0:
                overlap[t["speaker"]] = overlap.get(t["speaker"], 0.0) + o
        if overlap:
            best = max(overlap, key=overlap.get)
            line["speaker"] = best
            line["speakerShare"] = overlap[best] / max(line["end"] - line["start"], 1e-6)
        else:
            line["speaker"] = None
            line["speakerShare"] = 0.0


def estimate_gender(samples, rate: int) -> str:
    """`male`, `female` or `unknown` from the median pitch of voiced frames (autocorrelation).

    Rough by design: it picks a default voice, and the user can change it later.
    """
    import numpy as np

    x = np.asarray(samples, dtype="float32")
    if x.ndim > 1:
        x = x.mean(axis=1)
    frame, hop = int(0.04 * rate), int(0.02 * rate)
    lo, hi = int(rate / 400), int(rate / 70)
    pitches = []
    for i in range(0, len(x) - frame, hop):
        f = x[i : i + frame] - x[i : i + frame].mean()
        if float(np.sqrt((f**2).mean())) < 0.01:
            continue
        ac = np.correlate(f, f, mode="full")[frame - 1 :]
        if ac[0] <= 0:
            continue
        lag = lo + int(np.argmax(ac[lo:hi]))
        if ac[lag] / ac[0] > 0.4:
            pitches.append(rate / lag)
    if len(pitches) < 5:
        return "unknown"
    median = float(np.median(pitches))
    return "male" if median < 165 else "female"


def diarize(ctx: Context, req: Dict[str, Any], data) -> Optional[List[dict]]:
    """Speaker turns from sherpa-onnx when a `speaker` model folder is given, else `None`."""
    folder = (req.get("models") or {}).get("speaker")
    if not folder:
        return None
    so = require("sherpa_onnx")
    root = ctx.path(folder)
    # The folder holds one segmentation model and one embedding model, told apart by their path.
    models = sorted(root.glob("**/*.onnx"))
    segmentation = next((p for p in models if "segmentation" in str(p.relative_to(root)).lower()), None)
    embedding = next((p for p in models if "segmentation" not in str(p.relative_to(root)).lower()), None)
    if not segmentation or not embedding:
        return None
    config = so.OfflineSpeakerDiarizationConfig(
        segmentation=so.OfflineSpeakerSegmentationModelConfig(
            pyannote=so.OfflineSpeakerSegmentationPyannoteModelConfig(model=str(segmentation)),
            num_threads=int(ctx.params.get("threads", 2)) or 2,
        ),
        embedding=so.SpeakerEmbeddingExtractorConfig(model=str(embedding), num_threads=int(ctx.params.get("threads", 2)) or 2),
        clustering=so.FastClusteringConfig(num_clusters=-1, threshold=0.6),
        min_duration_on=0.3,
        min_duration_off=0.5,
    )
    sd = so.OfflineSpeakerDiarization(config)
    result = sd.process(data).sort_by_start_time()
    return [{"start": r.start, "end": r.end, "speaker": f"spk{r.speaker + 1}"} for r in result]


def run(ctx: Context, req: Dict[str, Any]) -> Dict[str, Any]:
    fw = require("faster_whisper")
    require("numpy")
    inp, out = req["input"], req["output"]
    audio_path = ctx.path(inp["audio"])
    language = normalise_code(inp.get("language", "auto"), ctx.params.get("codeMap"))
    model = fw.WhisperModel(
        str(model_dir(ctx, req)),
        device=ctx.params.get("device", "cpu"),
        compute_type=ctx.params.get("computeType", "int8"),
        cpu_threads=int(ctx.params.get("threads", 0)),
    )
    # One decode for everything: mono 16 kHz whatever the file is, cut to the window.
    rate = 16000
    data = fw.audio.decode_audio(str(audio_path), sampling_rate=rate)
    start_ms, end_ms = window_ms(req)
    if end_ms > start_ms:
        data = data[int(start_ms * rate / 1000) : int(end_ms * rate / 1000)]
    segments, info = model.transcribe(
        data,
        language=None if language in ("", "auto") else language,
        vad_filter=True,
        word_timestamps=True,
        condition_on_previous_text=False,
    )
    total = max(float(info.duration), 0.001)
    words: List[dict] = []
    for seg in segments:
        ctx.check_cancel()
        for w in seg.words or []:
            words.append({"start": w.start, "end": w.end, "word": w.word, "probability": w.probability})
        ctx.progress(min(0.8, seg.end / total * 0.8))
        new = split_lines(words[-len(seg.words or []) :]) if seg.words else []
        for line in new:
            ctx.item("line", {"startMs": int(line["start"] * 1000), "endMs": int(line["end"] * 1000), "text": line["text"]})
    lines = split_lines(words)

    ctx.check_cancel()
    turns = diarize(ctx, req, data) if lines else None
    if turns:
        assign_speakers(lines, turns)
    speakers: Dict[str, dict] = {}
    for line in lines:
        spk = line.get("speaker") or "spk1"
        line["speaker"] = spk
        sp = speakers.setdefault(spk, {"id": spk, "samples": [], "shares": []})
        sp["shares"].append(line.get("speakerShare", 1.0) if turns else 1.0)
        a, b = int(line["start"] * rate), int(line["end"] * rate)
        if len(sp["samples"]) < 12:  # a dozen lines are enough to judge a voice
            sp["samples"].append(data[a:b])
    speaker_list = []
    for sp in speakers.values():
        import numpy as np

        joined = np.concatenate(sp["samples"]) if sp["samples"] else np.zeros(1, dtype="float32")
        speaker_list.append(
            {
                "id": sp["id"],
                "gender": estimate_gender(joined, rate),
                "confidence": round(float(sum(sp["shares"]) / len(sp["shares"])), 3),
            }
        )

    result_lines = [
        {
            "startMs": int(l["start"] * 1000),
            "endMs": max(int(l["end"] * 1000), int(l["start"] * 1000) + 1),
            "text": l["text"],
            "language": info.language,
            "speakerHint": l["speaker"],
            "confidence": round(float(l["confidence"]), 3),
        }
        for l in lines
    ]
    name = f"{out['prefix']}.json"
    write_json(ctx.path(out["dir"]) / name, {"language": info.language, "lines": result_lines, "speakers": speaker_list})
    ctx.progress(1.0)
    return {
        "result": {"language": info.language, "lineCount": len(result_lines), "speakers": speaker_list},
        "files": [name],
    }
