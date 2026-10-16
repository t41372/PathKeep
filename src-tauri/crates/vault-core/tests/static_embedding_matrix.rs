//! Responsibilities: exercise the production static loader, pooling and cache with small local
//! model bundles. Not responsible for downloads or real-model parity (static_embedding_e2e).

use vault_core::{
    EmbeddingProvider, EmbeddingRole, ModelFile, NoopDownloadProgress, StaticEmbeddingMatrix,
    StaticEmbeddingProvider, ensure_model_downloaded, model_dir_for_repo, project_paths_with_root,
};

const REPO: &str = "test/static-matrix";

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_le_bytes()).collect()
}

struct Tensor<'a> {
    name: &'a str,
    dtype: &'a str,
    shape: &'a [usize],
    bytes: Vec<u8>,
}

fn matrix(name: &str) -> Tensor<'_> {
    Tensor { name, dtype: "F32", shape: &[3, 2], bytes: f32_bytes(&[2., -4., 100., 100., 6., 8.]) }
}

/// Build files and their manifest independently of the loader. Hash verification and the cache
/// follow their real paths; only the vocabulary and tensor data are deliberately small.
fn bundle(root: &std::path::Path, tensors: &[Tensor<'_>]) -> Vec<ModelFile> {
    let paths = project_paths_with_root(root);
    let directory = model_dir_for_repo(&paths, REPO);
    std::fs::create_dir_all(&directory).expect("create model directory");
    let mut header = serde_json::Map::new();
    let mut data = Vec::new();
    for tensor in tensors {
        let start = data.len();
        data.extend_from_slice(&tensor.bytes);
        header.insert(
            tensor.name.into(),
            serde_json::json!({"dtype": tensor.dtype,
            "shape": tensor.shape, "data_offsets": [start, data.len()]}),
        );
    }
    let mut header = serde_json::to_vec(&header).expect("header");
    header.resize(header.len().next_multiple_of(8), b' ');
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend(header);
    bytes.extend(data);
    std::fs::write(directory.join("model.safetensors"), bytes).expect("write tensors");
    std::fs::write(directory.join("config.json"), br#"{"hidden_dim":2,"normalize":false}"#)
        .expect("write config");
    let model = tokenizers::models::wordlevel::WordLevel::builder()
        .vocab(
            [("alpha".into(), 0), ("[UNK]".into(), 1), ("gamma".into(), 2)].into_iter().collect(),
        )
        .unk_token("[UNK]".into())
        .build()
        .expect("word model");
    let mut tokenizer = tokenizers::Tokenizer::new(model);
    tokenizer.with_pre_tokenizer(Some(tokenizers::pre_tokenizers::whitespace::Whitespace));
    tokenizer.save(directory.join("tokenizer.json"), false).expect("write tokenizer");
    let files: Vec<_> = ["model.safetensors", "config.json", "tokenizer.json"]
        .into_iter()
        .map(|name| ModelFile {
            name,
            repo: REPO,
            sha256: Box::leak(
                vault_core::file_sha256_hex(&directory.join(name))
                    .expect("hash fixture")
                    .into_boxed_str(),
            ),
        })
        .collect();
    ensure_model_downloaded(&paths, REPO, &files, false, &mut NoopDownloadProgress)
        .expect("verify local fixture without network");
    files
}

// Failure cases: a tensor starting at an unaligned byte offset must use its own range; mapped values
// must preserve little-endian floats, optional weighting, repeated IDs, unk and missing IDs.
// All supported names must load. Invalid bundles must still fail before embedding.
#[tokio::test]
async fn static_matrix_mapped_pooling_matches_owned_rows() {
    for name in ["embeddings", "0", "embedding.weight"] {
        let root = tempfile::tempdir().expect("root");
        let files = bundle(
            root.path(),
            &[
                Tensor { name: "before", dtype: "U8", shape: &[1], bytes: vec![255] },
                matrix(name),
                Tensor {
                    name: "weights",
                    dtype: "F32",
                    shape: &[3],
                    bytes: f32_bytes(&[0.5, 2., 1.5]),
                },
            ],
        );
        let paths = project_paths_with_root(root.path());
        let provider = StaticEmbeddingProvider::load(&paths, REPO, &files).expect("load fixture");
        let texts = ["alpha unknown gamma gamma", "unknown", ""].map(String::from);
        let vectors = provider.embed(&texts, EmbeddingRole::Document).await.expect("embed fixture");
        let owned = StaticEmbeddingMatrix::new(
            vec![2., -4., 100., 100., 6., 8.],
            2,
            Some(vec![0.5, 2., 1.5]),
        )
        .expect("owned matrix")
        .with_unk_token(Some(1));
        assert_eq!(vectors, vec![owned.mean_pool(&[0, 1, 2, 2, 99]), vec![0., 0.], vec![0., 0.]]);
        // A cache hit must preserve the same backing and result after the first provider drops.
        drop(provider);
        let reused = StaticEmbeddingProvider::load(&paths, REPO, &files).expect("reuse engine");
        assert_eq!(
            reused.embed(&texts, EmbeddingRole::Query).await.expect("cached embed"),
            vectors
        );
    }
}

// Replacing a file must not change an engine already borrowed by a caller. A later load must
// evict the cached revision, refuse stale hashes and accept the newly verified bundle.
#[tokio::test]
async fn static_matrix_replacement_preserves_active_mapping_and_invalidates_cache() {
    let root = tempfile::tempdir().expect("root");
    let paths = project_paths_with_root(root.path());
    let files = bundle(root.path(), &[matrix("embeddings")]);
    let original = StaticEmbeddingProvider::load(&paths, REPO, &files).expect("original engine");
    let texts = ["alpha gamma".to_string()];
    let expected = vec![vec![4., 2.]];
    assert_eq!(
        original.embed(&texts, EmbeddingRole::Query).await.expect("original query"),
        expected
    );

    let replacement_root = tempfile::tempdir().expect("replacement root");
    let replacement_files = bundle(
        replacement_root.path(),
        &[Tensor { bytes: f32_bytes(&[4., -8., 100., 100., 12., 16.]), ..matrix("embeddings") }],
    );
    let replacement_dir =
        model_dir_for_repo(&project_paths_with_root(replacement_root.path()), REPO);
    let replacement_path = replacement_dir.join("model.safetensors");
    // Force a distinct revision even on filesystems whose timestamp resolution is coarse.
    std::fs::File::open(&replacement_path)
        .expect("replacement file")
        .set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1))
        .expect("distinct mtime");
    std::fs::rename(replacement_path, model_dir_for_repo(&paths, REPO).join("model.safetensors"))
        .expect("atomically replace matrix");
    assert!(StaticEmbeddingProvider::load(&paths, REPO, &files).is_err(), "reject stale digest");
    let replacement = StaticEmbeddingProvider::load(&paths, REPO, &replacement_files)
        .expect("load replacement revision");
    assert_eq!(
        replacement.embed(&texts, EmbeddingRole::Query).await.expect("new query"),
        vec![vec![8., 4.]]
    );
    assert_eq!(
        original.embed(&texts, EmbeddingRole::Query).await.expect("active old query"),
        expected
    );
}

#[test]
fn static_matrix_rejects_unsupported_or_inconsistent_tensors() {
    let cases = [
        (vec![matrix("wrong_name")], "has no embedding tensor"),
        (vec![Tensor { shape: &[6], ..matrix("embeddings") }], "not a 2-D"),
        (vec![Tensor { dtype: "I32", ..matrix("embeddings") }], "not f32"),
        (vec![Tensor { shape: &[0, 2], bytes: vec![], ..matrix("embeddings") }], "zero rows"),
        (vec![Tensor { shape: &[3, 0], bytes: vec![], ..matrix("embeddings") }], "non-zero"),
        (
            vec![
                matrix("embeddings"),
                Tensor { name: "weights", dtype: "F32", shape: &[1], bytes: f32_bytes(&[1.]) },
            ],
            "weights length",
        ),
        (
            vec![
                matrix("embeddings"),
                Tensor { name: "weights", dtype: "I32", shape: &[3], bytes: vec![0; 12] },
            ],
            "not f32",
        ),
        (vec![Tensor { shape: &[2, 3], ..matrix("embeddings") }], "hidden_dim"),
    ];
    for (tensors, expected) in cases {
        let root = tempfile::tempdir().expect("root");
        let files = bundle(root.path(), &tensors);
        let error =
            StaticEmbeddingProvider::load(&project_paths_with_root(root.path()), REPO, &files)
                .err()
                .expect("reject inconsistent model");
        assert!(format!("{error:#}").contains(expected), "expected {expected}: {error:#}");
    }
    for name in ["mapping", "token_mapping"] {
        let root = tempfile::tempdir().expect("root");
        let files = bundle(
            root.path(),
            &[matrix("embeddings"), Tensor { name, dtype: "I32", shape: &[3], bytes: vec![0; 12] }],
        );
        let error =
            StaticEmbeddingProvider::load(&project_paths_with_root(root.path()), REPO, &files)
                .err()
                .expect("reject remapping");
        assert!(error.to_string().contains("token-remapping"), "{error:#}");
    }
}
