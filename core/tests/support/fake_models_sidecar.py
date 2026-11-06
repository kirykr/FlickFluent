"""A sidecar with fake model workers, for run tests. Real protocol, no ML, stdlib only.

Params the tests use: `speech` (false gives no transcript lines), `slowWorker` + `slowSection`
+ `slowSeconds` (the named worker lingers in the named section so a test can kill the run),
`synthLog` (a file that gets one JSON line of line ids per synthesize request) and
`lineDelayMs` (synthesize waits this long after each line, so a test can stop it half way).
"""

import errno
import json
import math
import struct
import sys
import time
import wave
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "sidecar"))

from flickfluent_sidecar.protocol import Server, atomic_write  # noqa: E402


def linger(ctx, req, worker):
    disk_full(ctx, req, worker)
    p = ctx.params
    if p.get("slowWorker") == worker and f"/{p.get('slowSection')}" in req["output"]["dir"].replace("\\", "/"):
        end = time.time() + float(p.get("slowSeconds", 30))
        while time.time() < end:
            ctx.check_cancel()
            time.sleep(0.02)


def disk_full(ctx, req, worker):
    """`diskFullWorker` + `diskFullSection`: that worker fails in that section as a full disk does."""
    p = ctx.params
    if p.get("diskFullWorker") == worker and f"/{p.get('diskFullSection')}" in req["output"]["dir"].replace("\\", "/"):
        raise OSError(errno.ENOSPC, "No space left on device")


def separate(ctx, req):
    inp, out = req["input"], req["output"]
    linger(ctx, req, "separate")
    with wave.open(inp["audio"], "rb") as src:
        rate, ch, width = src.getframerate(), src.getnchannels(), src.getsampwidth()
        src.setpos(int(inp["startMs"] * rate / 1000))
        frames = src.readframes(int((inp["endMs"] - inp["startMs"]) * rate / 1000))
    silence = bytes(len(frames))
    names = []
    for name, data in (("vocals", frames), ("music", silence)):
        target = Path(out["dir"]) / f"{out['prefix']}-{name}.wav"
        part = target.with_name(target.name + ".part")
        with wave.open(str(part), "wb") as w:
            w.setnchannels(ch)
            w.setsampwidth(width)
            w.setframerate(rate)
            w.writeframes(data)
        part.replace(target)
        names.append(target.name)
    ctx.progress(1.0)
    return {"result": {}, "files": names}


def transcribe(ctx, req):
    inp, out = req["input"], req["output"]
    length = int(inp["endMs"]) - int(inp["startMs"])
    lines = []
    if ctx.params.get("speech", True):
        spans = [(500, 2500, "spk1"), (3000, 5000, "spk2")] if length > 5500 else [(100, max(length - 100, 200), "spk1")]
        for i, (a, b, spk) in enumerate(spans):
            lines.append({"startMs": a, "endMs": b, "text": f"ligne {i}", "language": "fr", "speakerHint": spk, "confidence": 0.9})
    ctx.progress(0.5)
    linger(ctx, req, "transcribe")
    speakers = [{"id": "spk1", "gender": "male", "confidence": 1.0}, {"id": "spk2", "gender": "female", "confidence": 1.0}]
    name = f"{out['prefix']}.json"
    atomic_write(Path(out["dir"]) / name, json.dumps({"language": "fr", "lines": lines, "speakers": speakers if lines else []}).encode())
    return {"result": {"language": "fr", "lineCount": len(lines)}, "files": [name]}


def translate(ctx, req):
    linger(ctx, req, "translate")
    return {"result": {"translated": ["T:" + l["text"] for l in req["input"]["lines"]]}}


def synthesize(ctx, req):
    out_dir = Path(req["output"]["dir"])
    clips, names = [], []
    log = ctx.params.get("synthLog")
    if log:
        with open(log, "a") as f:
            f.write(json.dumps([l["id"] for l in req["input"]["lines"]]) + "\n")
    for i, line in enumerate(req["input"]["lines"]):
        ms = int(line["targetMs"] * 0.9)
        rate = 22050
        samples = b"".join(struct.pack("<h", int(8000 * math.sin(2 * math.pi * 440 * n / rate))) for n in range(int(rate * ms / 1000)))
        target = out_dir / f"{line['id']}.wav"
        part = target.with_name(target.name + ".part")
        with wave.open(str(part), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(rate)
            w.writeframes(samples)
        part.replace(target)
        names.append(target.name)
        clips.append({"id": line["id"], "audioMs": ms, "sampleRate": rate, "speed": 1.0})
        ctx.item("clip", clips[-1])
        ctx.progress((i + 1) / len(req["input"]["lines"]))
        linger(ctx, req, "synthesize")
        delay = float(ctx.params.get("lineDelayMs", 0)) / 1000
        end = time.time() + delay
        while time.time() < end:
            ctx.check_cancel()
            time.sleep(0.01)
    return {"result": {"clips": clips}, "files": names}


def detect_language(ctx, req):
    return {"result": {"language": "fr", "confidence": 0.99}}


def main():
    allow = [sys.argv[i + 1] for i, a in enumerate(sys.argv) if a == "--allow"]
    Server(
        {"separate": separate, "transcribe": transcribe, "translate": translate, "synthesize": synthesize, "detect_language": detect_language},
        allow,
    ).serve()


if __name__ == "__main__":
    main()
