"""Translation contract: count, order, numbers and names kept."""

import pytest

pytest.importorskip("ctranslate2")
pytest.importorskip("sentencepiece")

from flickfluent_sidecar.protocol import WorkerError
from flickfluent_sidecar.workers import translate

LINES = [
    {"text": "Bonjour, je m'appelle Thomas."},
    {"text": "Nous avons 3 chats et 12 chiens."},
    {"text": "Merci beaucoup."},
]


def run(call, source="fr", target="en", lines=LINES):
    return call(translate.run, {"source": source, "target": target, "lines": lines}, {"main": "m2m100"})["result"]["translated"]


def test_same_count_and_order_non_empty(call):
    out = run(call)
    assert len(out) == len(LINES) and all(t.strip() for t in out)
    assert "Thomas" in out[0], out
    assert "Thank" in out[2] or "thank" in out[2], out


def test_numbers_are_kept(call):
    out = run(call)
    assert "3" in out[1] and "12" in out[1], out


def test_more_lines_than_one_batch_keep_their_order(call):
    lines = [{"text": f"Ligne numéro {i}."} for i in range(205)]
    out = run(call, lines=lines)
    assert len(out) == 205 and all(str(i) in out[i] for i in (0, 99, 204)), out[:3]


def test_same_language_is_refused(call):
    with pytest.raises(WorkerError) as e:
        run(call, source="fr", target="fr")
    assert e.value.code == "SAME_LANGUAGE"
