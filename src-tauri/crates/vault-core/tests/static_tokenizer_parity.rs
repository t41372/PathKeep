//! Responsibilities: compare the production tokenizer with tokenizers 0.22.2 on local model data.
//! Not responsible for downloads, embedding math, or timing measurements.
//!
//! Failure modes recorded before implementing the compact model:
//! - Prefix lookup can miss a terminal piece, a longer piece, or a UTF-8 byte boundary.
//! - Viterbi can change tie order, score precision, or the minimum-score-minus-ten unk penalty.
//! - Unknown characters can consume bytes instead of characters, or fail to fuse into one token.
//! - Normalization, Metaspace, or added-token extraction can change IDs before segmentation.
//! - Empty input, whitespace-only input, and long unsplit input can break backtracking or offsets.
//! - Sorting the vocabulary can accidentally replace original model IDs with sorted positions.
//! - Byte fallback can spell a known piece, spell only part of an unknown one, give byte tokens
//!   their own offsets, or emit bytes when one `<0xNN>` token is missing instead of falling to unk.
//! - Empty or duplicate pieces can corrupt the sorted prefix search instead of failing to load.
//!
//! Run with PATHKEEP_STATIC_PARITY=1; reuses PATHKEEP_STATIC_MODELS_DIR or artifacts/static-models.

use std::path::PathBuf;
use vault_core::ai::StaticTokenizer;

/// A small scored vocabulary makes ties and unk penalties observable without model downloads.
/// The expected IDs describe upstream's contract, independently of the sorted storage layout.
#[test]
fn compact_unigram_preserves_scored_paths_and_fused_unknowns() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "version": "1.0",
        "model": {"type": "Unigram", "unk_id": 0, "byte_fallback": false,
            "vocab": [["<unk>", 0.0], ["a", -1.0], ["b", -1.0], ["ab", -2.0],
                ["bc", -0.5], ["c", -1.0], ["abc", -9.0], ["é", -1.0],
                ["éa", -0.5], ["xy", -20.0], ["💥", -1.0], ["💥a", -0.5]]}
    }))
    .unwrap();
    let reference = tokenizers::Tokenizer::from_bytes(&bytes).unwrap();
    let compact = StaticTokenizer::from_bytes(&bytes).unwrap();
    for (text, ids) in [
        ("", vec![]),
        ("ab", vec![3]),     // Equal scores retain the whole piece encountered first.
        ("abc", vec![1, 4]), // Greedy longest-piece matching would choose the wrong path.
        ("xy", vec![9]),     // The unk row's zero score must not beat a known negative-score piece.
        ("ΩЖ🫠aΩЖ", vec![0, 1, 0]),
        ("éa💥a", vec![8, 11]),
        ("💥ΩЖé", vec![10, 0, 7]),
    ] {
        let expected = reference.encode(text, false).unwrap();
        assert_eq!(expected.get_ids(), ids, "reference contract for {text:?}");
        let actual = compact.encode(text, false).unwrap();
        assert_eq!(actual.get_ids(), expected.get_ids(), "IDs for {text:?}");
        assert_eq!(actual.get_offsets(), expected.get_offsets(), "offsets for {text:?}");
    }
}

/// Unknown runs are spelled byte by byte only when every byte has a token; otherwise they stay unk.
#[test]
fn compact_unigram_spells_unknown_runs_with_byte_fallback() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "version": "1.0",
        "model": {"type": "Unigram", "unk_id": 0, "byte_fallback": true,
            "vocab": [["<unk>", 0.0], ["a", -1.0], ["<0xC3>", -5.0], ["<0xA9>", -5.0],
                ["<0xE2>", -5.0], ["<0x82>", -5.0], ["b", -1.0]]}
    }))
    .unwrap();
    let reference = tokenizers::Tokenizer::from_bytes(&bytes).unwrap();
    let compact = StaticTokenizer::from_bytes(&bytes).unwrap();
    for (text, ids) in [
        ("aéb", vec![1, 2, 3, 6]),
        ("éé", vec![2, 3, 2, 3]), // Fused first, then spelled: one run of four byte tokens.
        ("a€", vec![1, 0]),       // € is E2 82 AC; <0xAC> is missing, so the run stays unk.
        ("é€", vec![0]),          // A fused run is spelled only if all of its bytes are known.
    ] {
        let expected = reference.encode(text, false).unwrap();
        assert_eq!(expected.get_ids(), ids, "reference contract for {text:?}");
        let actual = compact.encode(text, false).unwrap();
        assert_eq!(actual.get_ids(), expected.get_ids(), "IDs for {text:?}");
        assert_eq!(actual.get_offsets(), expected.get_offsets(), "offsets for {text:?}");
        assert_eq!(actual.get_tokens(), expected.get_tokens(), "token strings for {text:?}");
    }
}

/// Upstream accepts these silently; the sorted search would misread them, so loading fails.
#[test]
fn compact_unigram_rejects_empty_and_duplicate_pieces() {
    for vocab in [
        serde_json::json!([["<unk>", 0.0], ["", -1.0], ["a", -1.0]]),
        serde_json::json!([["<unk>", 0.0], ["a", -1.0], ["a", -2.0]]),
    ] {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "version": "1.0",
            "model": {"type": "Unigram", "unk_id": 0, "vocab": vocab}
        }))
        .unwrap();
        assert!(StaticTokenizer::from_bytes(&bytes).is_err(), "loaded {vocab}");
    }
}

#[test]
fn static_tokenizer_matches_upstream_ids() {
    if std::env::var("PATHKEEP_STATIC_PARITY").as_deref() != Ok("1") {
        eprintln!("skipping token-ID parity: set PATHKEEP_STATIC_PARITY=1");
        return;
    }
    let root =
        std::env::var_os("PATHKEEP_STATIC_MODELS_DIR").map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/static-models")
        });
    let bytes =
        std::fs::read(root.join("models/minishlab%2Fpotion-multilingual-128M/tokenizer.json"))
            .expect("reuse the downloaded tokenizer.json");
    let reference = tokenizers::Tokenizer::from_bytes(&bytes).expect("upstream tokenizer");
    let compact = StaticTokenizer::from_bytes(&bytes).expect("compact tokenizer");
    let mut texts: Vec<String> = [
        "",
        "a",
        "漢",
        "字",
        "中文繁體與简体、日本語カタカナとひらがな、한국어",
        "\u{10ffff}",
        "\u{10ffff}\u{10fffe}\u{10fffd}",
        "known\u{10ffff}\u{10fffe}word\u{10fffd}",
        "𓀀𓀁𓀂𓀃𓀄 𒀀𒀁𒀂𒀃",
        "👩🏽‍💻👨‍👩‍👧‍👦🇹🇼❤️🏳️‍🌈",
        "e\u{301} a\u{30a} \u{301}\u{308} क्‍ष",
        "é Å café naïve résumé",
        "العَرَبِيَّة لغة جميلة، مرحباً بالعالم!",
        "ภาษาไทยไม่มีเว้นวรรคระหว่างคำทดสอบ",
        "हिन्दी भाषा में खोजें। देवनागरी क्षत्रिय",
        "Rust中文العربيةภาษาไทยहिन्दी한글123🙂",
        "0123456789 00000000000000000 3.141592653589793 -1.2e+30",
        "!!!???...---___///\\\\::;;,,()[]{}<>@#$%^&*+=|~`",
        " ",
        "   ",
        "\t\r\n\t\n",
        " leading",
        "trailing ",
        "  both  ",
        "a  b\tc\nd\r\ne\u{a0}f\u{3000}g",
        "\u{200b}\u{feff}a\u{200d}b",
        "Ｆｕｌｌｗｉｄｔｈ １２３ ①②Ⅳ ﬁﬃ K Å ㍿ ㎏ ²",
        "[PAD]",
        "[UNK]",
        "[PAD][UNK][PAD]",
        "before[UNK]after [PAD] tail",
        "[unk] [pad] ［ＵＮＫ］",
        "https://example.com/a%20b?q=中文&n=123#fragment",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    // Runs exceed the longest (186-character) piece; no pre-tokenizer split may hide this case.
    texts.push("漢字中文日本語한글".repeat(80));
    texts.push("\u{10ffff}".repeat(256));
    texts.push("1234567890!?,.".repeat(80));
    let body: String =
        "Rust資料庫🙂 العربية ภาษาไทย हिन्दी e\u{301} \n".chars().cycle().take(2048).collect();
    assert_eq!(body.chars().count(), 2048);
    texts.push(body);

    let parsed: serde_json::Value = serde_json::from_slice(&bytes).expect("tokenizer fixture");
    let vocab = parsed["model"]["vocab"].as_array().expect("Unigram vocab");
    let longest =
        vocab.iter().max_by_key(|piece| piece[0].as_str().unwrap().chars().count()).unwrap();
    texts.push(longest[0].as_str().unwrap().to_owned());
    // Sample vocabulary IDs across the whole sorted range, both alone and amid competing prefixes.
    for piece in vocab.iter().step_by(499) {
        let piece = piece[0].as_str().unwrap();
        texts.push(piece.to_owned());
        texts.push(format!("前 {piece} after\u{10ffff}\u{10fffe}"));
    }
    let mut mismatches = Vec::new();
    for (index, text) in texts.iter().enumerate() {
        let expected = reference.encode(text.as_str(), false).expect("reference encode");
        let actual = compact.encode(text.as_str(), false).expect("compact encode");
        if actual.get_ids() != expected.get_ids() || actual.get_offsets() != expected.get_offsets()
        {
            mismatches.push(serde_json::json!({"index": index, "text": text,
                "expected": expected.get_ids(), "actual": actual.get_ids(),
                "expected_offsets": expected.get_offsets(), "actual_offsets": actual.get_offsets()}));
        }
    }
    let report = serde_json::json!({"texts": texts.len(), "mismatches": mismatches.len(),
        "failures": mismatches});
    let artifact = root.join("tokenizer-parity.json");
    std::fs::write(&artifact, serde_json::to_vec_pretty(&report).unwrap())
        .expect("write parity artifact");
    eprintln!(
        "Token-ID parity: {} texts, {} mismatches; {}",
        texts.len(),
        mismatches.len(),
        artifact.display()
    );
    assert!(mismatches.is_empty(), "token IDs or offsets differ: {mismatches:?}");
}
