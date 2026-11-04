"""Synthesis contract: a real, non-silent clip per line, and honest lengths."""

import pytest

np = pytest.importorskip("numpy")
sf = pytest.importorskip("soundfile")
pytest.importorskip("sherpa_onnx")

from flickfluent_sidecar.workers import synthesize

VOICE = "voice/vits-piper-fr_FR-siwis-medium-int8"


def lines():
    return [
        {"id": "l1", "text": "Bonjour, je m'appelle Thomas.", "voiceId": "fr-siwis", "targetMs": 3000},
        {"id": "l2", "text": "Merci beaucoup pour votre aide aujourd'hui.", "voiceId": "fr-siwis", "targetMs": 3500},
    ]


def test_each_line_has_a_non_silent_file_with_the_voice_rate(call, tmp_path, models):
    out = call(synthesize.run, {"lines": lines()}, {"fr-siwis": VOICE})
    assert sorted(out["files"]) == ["l1.wav", "l2.wav"]
    for clip, spec in zip(out["result"]["clips"], lines()):
        data, rate = sf.read(tmp_path / f"{clip['id']}.wav")
        assert rate == clip["sampleRate"] == 22050
        assert float(np.abs(data).max()) > 0.05, "clip is not silent"
        reported = len(data) * 1000 / rate
        assert abs(reported - clip["audioMs"]) <= 2
        ratio = clip["audioMs"] / spec["targetMs"]
        assert 0.3 <= ratio <= 3.0, ratio
    assert not list(tmp_path.glob("*.part"))


def test_a_slot_too_short_speeds_the_line_up_within_the_limit(call):
    long_line = [{"id": "x", "text": "Merci beaucoup pour votre aide aujourd'hui.", "voiceId": "fr-siwis", "targetMs": 500}]
    clip = call(synthesize.run, {"lines": long_line}, {"fr-siwis": VOICE})["result"]["clips"][0]
    assert 1.0 < clip["speed"] <= synthesize.MAX_SPEEDUP
