"""Worker logic that needs no model: line splitting, speaker assignment, pitch, fades, codes."""

import math

import pytest

np = pytest.importorskip("numpy")

from flickfluent_sidecar.workers.common import normalise_code
from flickfluent_sidecar.workers.separate import fade_weights
from flickfluent_sidecar.workers.synthesize import MAX_SPEEDUP
from flickfluent_sidecar.workers.transcribe import assign_speakers, estimate_gender, split_lines
from flickfluent_sidecar.workers.translate import batches, lang_token


def w(start, end, word, p=0.9):
    return {"start": start, "end": end, "word": word, "probability": p}


def test_split_lines_cuts_at_sentence_ends_and_long_pauses():
    words = [w(0.0, 0.4, " Bonjour"), w(0.4, 0.8, " tout"), w(0.8, 1.2, " le"), w(1.2, 1.6, " monde."), w(1.7, 2.0, " Merci"), w(3.5, 3.9, " beaucoup")]
    lines = split_lines(words)
    assert [l["text"] for l in lines] == ["Bonjour tout le monde.", "Merci", "beaucoup"]
    assert all(l["end"] > l["start"] for l in lines)
    assert lines == sorted(lines, key=lambda l: l["start"])


def test_split_lines_never_overlaps_neighbours_and_drops_empty_text():
    lines = split_lines([w(0.0, 1.05, " Un."), w(1.0, 2.0, " Deux."), w(2.0, 2.0, " ")])
    assert len(lines) == 2
    assert lines[0]["end"] <= lines[1]["start"]


def test_split_lines_empty():
    assert split_lines([]) == []


def test_assign_speakers_picks_the_longest_overlap():
    lines = [{"start": 0.0, "end": 4.0}, {"start": 5.0, "end": 6.0}, {"start": 20.0, "end": 21.0}]
    turns = [
        {"start": 0.0, "end": 1.0, "speaker": "spk1"},
        {"start": 1.0, "end": 4.5, "speaker": "spk2"},
        {"start": 5.0, "end": 7.0, "speaker": "spk1"},
    ]
    assign_speakers(lines, turns)
    assert [l["speaker"] for l in lines] == ["spk2", "spk1", None]
    assert lines[0]["speakerShare"] == pytest.approx(0.75)


def tone(hz, seconds=1.0, rate=16000):
    t = np.arange(int(seconds * rate)) / rate
    # A few harmonics so autocorrelation sees a voice-like signal.
    return sum(np.sin(2 * math.pi * hz * k * t) / k for k in (1, 2, 3)).astype("float32") * 0.2


def test_estimate_gender_low_pitch_male_high_pitch_female_silence_unknown():
    assert estimate_gender(tone(110), 16000) == "male"
    assert estimate_gender(tone(220), 16000) == "female"
    assert estimate_gender(np.zeros(16000, dtype="float32"), 16000) == "unknown"


@pytest.mark.parametrize("length,overlap", [(1000, 100), (500, 100)])
def test_fade_weights_blend_to_one_across_the_overlap(length, overlap):
    a = fade_weights(length, overlap, first=True, last=False)
    b = fade_weights(length, overlap, first=False, last=True)
    assert np.allclose(a[-overlap:] + b[:overlap], 1.0)
    assert a[0] == 1.0 and b[-1] == 1.0


def test_codes_and_tokens():
    assert normalise_code("IW", {"iw": "he"}) == "he"
    assert normalise_code("pt_BR", None) == "pt-br"
    assert lang_token("zh-CN") == "__zh__"
    assert [len(b) for b in batches(list(range(450)), 200)] == [200, 200, 50]
    assert MAX_SPEEDUP > 1
