#!/usr/bin/env python3
"""Generate the PINNED model2vec parity reference fixture (W-AI-4c static engine).

The hand-rolled static (model2vec) embedding engine is the 100%-coverage FOUNDATION tier; its
vectors MUST match the Python model2vec reference. This script dumps `{text -> vector}` for a small
corpus to a committed JSON fixture so the parity test
(`src-tauri/crates/vault-core/tests/static_embedding_e2e.rs`) runs against PINNED reference vectors
as a STANDING gate — no Python at test time.

The corpus deliberately includes CJK, emoji, URLs, percent-encoded paths, AND truly out-of-vocabulary
rows (ancient scripts / exotic symbol planes) that potion's Unigram tokenizer maps to its `[UNK]`
row. Those OOV rows prove the engine handles unk EXACTLY like model2vec: potion's Unigram tokenizer
has no string `unk_token`, so model2vec POOLS `[UNK]` (it only drops unk for BPE/WordPiece models).
Dropping the Unigram `[UNK]` would collapse those rows to ~0.80 cosine — the divergence the gate
catches.

Usage (from a venv with `pip install model2vec`):
    python scripts/gen_static_parity_fixture.py \
        src-tauri/crates/vault-core/tests/fixtures/static_parity_potion_multilingual.json
"""

import json
import sys

from model2vec import StaticModel

MODEL = "minishlab/potion-multilingual-128M"

# Keep every input WELL under STATIC_MAX_INPUT_TOKENS (2048) so the engine's DoS-guard truncation
# never diverges from POTION's unbounded seq_length on the fixture (parity is exact under the cap).
#
# The last rows are TRULY out-of-vocabulary for potion's Unigram tokenizer (ancient scripts, exotic
# symbol/emoji planes) — these map to the `[UNK]` row (id 1, byte_fallback=False). They are the rows
# that PROVE the unk-token-drop fix: before it, the hand-roll pooled the `[UNK]` row and these fell
# below 0.999 cosine; the reference DROPS `[UNK]` before pooling, so the fixed engine matches.
TEXTS = [
    "the quick brown fox jumps over the lazy dog",
    "一段关于中央银行宏观经济学的论述",
    "rust programming language memory safety without garbage collection",
    "https://example.com/path/to/article-42?utm_source=newsletter&ref=home",
    "https://zh.wikipedia.org/wiki/%E4%B8%AD%E5%A4%AE%E9%8A%80%E8%A1%8C",
    "xq3zk-asdf_qwerty-9981-zzzv-oovslug-not-a-real-word",
    "deploy 🚀 to prod 🔥 with 💯 confidence — ship it 🎉",
    "https://mail.google.com/inbox?fbclid=IwAR0xQ_eMoJiNiNcKvOOVTOKEN",
    "日本語のテキストと English mixed スクリプト 测试 🌏 émoji café",
    "C:\\Users\\test\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\History",
    # OOV → [UNK] rows (the unk-drop proof):
    "𐍈𐌰 gothic 𒀀 cuneiform 🜀 alchemical symbols mixed with words",
    "🜂🜃🜄🜅 alchemy 𓀀𓀁 hieroglyph ᚠᚢᚦᚨᚱ runes ⠁⠃⠉ braille",
]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <output.json>")
    dst = sys.argv[1]
    model = StaticModel.from_pretrained(MODEL)
    vectors = model.encode(TEXTS)
    out = {
        "model": MODEL,
        "note": (
            "Reference vectors from Python model2vec (StaticModel.encode). Standing parity gate for "
            "the hand-rolled static engine. Regenerate via scripts/gen_static_parity_fixture.py."
        ),
        "texts": TEXTS,
        "vectors": [[float(x) for x in v] for v in vectors],
    }
    with open(dst, "w", encoding="utf-8") as handle:
        json.dump(out, handle)
    print(f"wrote {dst}: {len(TEXTS)} rows, dim {len(out['vectors'][0])}", file=sys.stderr)


if __name__ == "__main__":
    main()
