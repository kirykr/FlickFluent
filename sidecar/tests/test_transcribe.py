"""Transcription contract on the French fixture, with the tiny Whisper model."""

import json
import re

import pytest

pytest.importorskip("faster_whisper")
pytest.importorskip("soundfile")

from conftest import FIXTURES, extract_wav
from flickfluent_sidecar.workers import detect_language, transcribe

EXPECTED = json.loads((FIXTURES / "expected" / "clip-20s-fr.json").read_text(encoding="utf-8"))


def words(text):
    return re.findall(r"[\w']+", text.lower())


def word_error_rate(reference, hypothesis):
    r, h = words(reference), words(hypothesis)
    d = list(range(len(h) + 1))
    for i, rw in enumerate(r, 1):
        prev, d[0] = d[0], i
        for j, hw in enumerate(h, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (rw != hw))
    return d[len(h)] / max(len(r), 1)


def run_on(call, tmp_path, clip, language="fr", with_speakers=True):
    audio = extract_wav(FIXTURES / clip, tmp_path / "vocals.wav", 16000, 1)
    models = {"main": "whisper-tiny"}
    if with_speakers:
        models["speaker"] = "speaker"
    out = call(transcribe.run, {"audio": str(audio), "language": language}, models)
    return json.loads((tmp_path / out["files"][0]).read_text(encoding="utf-8")), out


def test_words_match_the_reference_and_lines_are_well_formed(call, tmp_path):
    data, out = run_on(call, tmp_path, "clip-20s-fr.mp4")
    text = " ".join(l["text"] for l in data["lines"])
    assert word_error_rate(" ".join(EXPECTED["lines"]), text) <= EXPECTED["maxWordErrorRate"], text
    lines = data["lines"]
    assert lines and all(l["endMs"] > l["startMs"] for l in lines)
    assert lines == sorted(lines, key=lambda l: l["startMs"])
    assert all(0 <= l["confidence"] <= 1 and l["speakerHint"] for l in lines)
    speakers = out["result"]["speakers"]
    assert abs(len(speakers) - EXPECTED["speakerCount"]) <= 1
    # The fixture has a man and a woman; on this fixture diarization finds both (a missing
    # speaker model would silently give one speaker, which "within 1" would let through).
    assert len(speakers) == 2 and {s["gender"] for s in speakers} == {"male", "female"}, speakers
    assert data["language"] == "fr"


def test_without_a_speaker_model_there_is_one_speaker(call, tmp_path):
    _, out = run_on(call, tmp_path, "clip-20s-fr.mp4", with_speakers=False)
    assert [s["id"] for s in out["result"]["speakers"]] == ["spk1"]


def test_a_speech_free_clip_gives_zero_lines(call, tmp_path):
    data, out = run_on(call, tmp_path, "clip-no-speech.mp4", language="auto")
    assert data["lines"] == [] and out["result"]["lineCount"] == 0


def test_detect_language_finds_french(call, tmp_path):
    audio = extract_wav(FIXTURES / "clip-20s-fr.mp4", tmp_path / "a.wav", 16000, 1)
    out = call(detect_language.run, {"audio": str(audio), "startMs": 0, "endMs": 15000}, {"main": "whisper-tiny"})
    assert out["result"]["language"] == "fr" and out["result"]["confidence"] > 0.5
