"""Translation with CTranslate2 and an M2M100 model (int8)."""

from __future__ import annotations

from typing import Any, Dict, List

from ..protocol import Context, WorkerError
from .common import model_dir, normalise_code, require

BATCH = 200


def lang_token(code: str) -> str:
    """M2M100 writes a language as `__fr__`. Chinese and Portuguese variants lose the region."""
    return f"__{code.split('-')[0]}__"


def batches(items: List[Any], size: int = BATCH):
    for i in range(0, len(items), size):
        yield items[i : i + size]


def run(ctx: Context, req: Dict[str, Any]) -> Dict[str, Any]:
    ct2 = require("ctranslate2")
    spm = require("sentencepiece")
    inp = req["input"]
    mapping = ctx.params.get("codeMap")
    source, target = normalise_code(inp["source"], mapping), normalise_code(inp["target"], mapping)
    if source == target:
        raise WorkerError("SAME_LANGUAGE", "source and target are the same")
    folder = model_dir(ctx, req)
    sp_file = next(folder.glob("*.model"), None)
    if sp_file is None:
        raise WorkerError("MODEL_MISSING", f"no sentencepiece model in {folder}")
    sp = spm.SentencePieceProcessor()
    sp.load(str(sp_file))
    translator = ct2.Translator(
        str(folder),
        device=ctx.params.get("device", "cpu"),
        compute_type=ctx.params.get("computeType", "int8"),
        inter_threads=1,
        intra_threads=int(ctx.params.get("threads", 0)),
    )
    lines = inp["lines"]
    out: List[str] = []
    done = 0
    for chunk in batches(lines):
        ctx.check_cancel()
        sources = [[lang_token(source)] + sp.encode(l["text"], out_type=str) + ["</s>"] for l in chunk]
        results = translator.translate_batch(
            sources,
            target_prefix=[[lang_token(target)]] * len(sources),
            beam_size=4,
            max_decoding_length=256,
        )
        for r in results:
            tokens = r.hypotheses[0][1:]  # drop the target language token
            out.append(sp.decode(tokens))
        done += len(chunk)
        ctx.progress(done / len(lines))
    return {"result": {"translated": out}}
