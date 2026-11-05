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


KOKORO = "kokoro-multi-lang-v1_0"
ESPEAK = "voice/vits-piper-fr_FR-siwis-medium-int8/espeak-ng-data"


def kokoro_lines():
    base = {"voiceId": "kokoro", "targetMs": 4000}
    return [
        {**base, "id": "fr", "text": "Bonjour, je m'appelle Thomas.", "speakerId": 30, "lang": "fr"},
        {**base, "id": "en", "text": "Hello, my name is Thomas.", "speakerId": 3, "lang": "en-us", "lexicon": "lexicon-us-en.txt"},
        {**base, "id": "en-male", "text": "Hello, my name is Thomas.", "speakerId": 16, "lang": "en-us", "lexicon": "lexicon-us-en.txt"},
        {**base, "id": "es", "text": "Hola, me llamo Tomás.", "speakerId": 28, "lang": "es"},
        {**base, "id": "zh", "text": "大家好，欢迎光临。", "speakerId": 47, "lang": "zh"},
    ]


def test_kokoro_speaks_several_languages_in_one_request(call, tmp_path, models):
    if not (models / KOKORO).is_dir():
        pytest.skip("no Kokoro model folder")
    out = call(synthesize.run, {"lines": kokoro_lines()}, {"kokoro": KOKORO, "espeak": ESPEAK})
    assert sorted(out["files"]) == ["en-male.wav", "en.wav", "es.wav", "fr.wav", "zh.wav"]
    audio = {}
    for clip in out["result"]["clips"]:
        data, rate = sf.read(tmp_path / f"{clip['id']}.wav")
        assert rate == clip["sampleRate"] == 24000
        assert float(np.abs(data).max()) > 0.05, f"{clip['id']} is silent"
        assert 600 < clip["audioMs"] < 6000, clip
        audio[clip["id"]] = data
    # The male and the female English voice say the same words differently.
    n = min(len(audio["en"]), len(audio["en-male"]))
    assert not np.allclose(audio["en"][:n], audio["en-male"][:n])


def test_an_empty_result_is_an_error_not_a_crash(call, tmp_path, models):
    if not (models / KOKORO).is_dir():
        pytest.skip("no Kokoro model folder")
    from flickfluent_sidecar.protocol import WorkerError

    # A language tag the engine does not know gives no sound.
    bad = [{"id": "x", "text": "Hello.", "voiceId": "kokoro", "speakerId": 3, "lang": "xx-nope", "targetMs": 2000}]
    with pytest.raises(WorkerError):
        call(synthesize.run, {"lines": bad}, {"kokoro": KOKORO, "espeak": ESPEAK})
