"""Separation contract: stems add up to the input, a music-only clip has near-silent vocals."""

import pytest

np = pytest.importorskip("numpy")
sf = pytest.importorskip("soundfile")
pytest.importorskip("demucs")

from conftest import FIXTURES, extract_wav
from flickfluent_sidecar.workers import separate


def energy(x):
    return float((x.astype("float64") ** 2).mean())


def run_on(call, tmp_path, clip, start=0, end=0):
    audio = extract_wav(FIXTURES / clip, tmp_path / "in.wav", 44100, 2)
    out = call(separate.run, {"audio": str(audio), "startMs": start, "endMs": end}, {"main": "htdemucs"})
    vocals, _ = sf.read(tmp_path / "sec0001-vocals.wav", always_2d=True)
    music, _ = sf.read(tmp_path / "sec0001-music.wav", always_2d=True)
    source, _ = sf.read(audio, always_2d=True)
    return out, source, vocals, music


def test_stems_add_up_to_the_input_and_have_equal_length(call, tmp_path):
    out, source, vocals, music = run_on(call, tmp_path, "clip-20s-fr.mp4")
    assert sorted(out["files"]) == ["sec0001-music.wav", "sec0001-vocals.wav"]
    assert len(vocals) == len(music) == len(source)
    error = energy(vocals + music - source)
    assert error / energy(source) < 0.05, "stems must add up to the input"
    assert not list(tmp_path.glob("*.part")), "no leftover .part files"


def test_music_only_clip_has_near_silent_vocals(call, tmp_path):
    _, source, vocals, music = run_on(call, tmp_path, "clip-music-only.mp4")
    assert energy(vocals) < 0.1 * energy(source)
    assert energy(music) > 0.5 * energy(source)


def test_a_window_inside_the_file_has_the_window_length(call, tmp_path):
    _, _, vocals, music = run_on(call, tmp_path, "clip-20s-fr.mp4", start=2000, end=7000)
    assert len(vocals) == len(music) == 5 * 44100
