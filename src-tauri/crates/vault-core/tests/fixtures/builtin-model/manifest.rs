// Pinned SHA-256 of the local HTTP fixtures. Both versions have the production output width.
pub(super) const V1: &[ModelFile] = &[
    ModelFile {
        name: "config.json",
        sha256: "9e3a4e619140e6943303f5fccdd93943de4efba7092807bcb7741f96f7ea1c7b",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "tokenizer.json",
        sha256: "e87722c865f4dd17519c09470edda629aec4032b50af34287ab087863c5d011d",
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
        sha256: "9e3a4e619140e6943303f5fccdd93943de4efba7092807bcb7741f96f7ea1c7b",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "tokenizer.json",
        sha256: "e87722c865f4dd17519c09470edda629aec4032b50af34287ab087863c5d011d",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
    ModelFile {
        name: "model.safetensors",
        sha256: "24f81c9e6f04c99329664bd7c0af1598e87e99d9b8c7d432387ed7fcab53b401",
        repo: DEFAULT_STATIC_MODEL_REPO,
    },
];
