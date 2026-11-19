// Pinned SHA-256 of the local HTTP fixtures. Both versions have the production output width.
pub(super) const V1: &[ModelFile] = &[
    ModelFile {
        name: "config.json",
        sha256: "6fd9706819056f2b6126589399f3c5940a5706824f4bd125f73de55340778600",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "tokenizer.json",
        sha256: "cf9c5e1afe0640d92851765a1739e79fc544f14aacd8d4d34299206802d85c1f",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "model.safetensors",
        sha256: "57af61e570eb07749a31401c519bed2f39d8d1df883aaff21a64fd5f8e6f7b54",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
];
pub(super) const V2: &[ModelFile] = &[
    ModelFile {
        name: "config.json",
        sha256: "6fd9706819056f2b6126589399f3c5940a5706824f4bd125f73de55340778600",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "tokenizer.json",
        sha256: "cf9c5e1afe0640d92851765a1739e79fc544f14aacd8d4d34299206802d85c1f",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "model.safetensors",
        sha256: "24f81c9e6f04c99329664bd7c0af1598e87e99d9b8c7d432387ed7fcab53b401",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
];
