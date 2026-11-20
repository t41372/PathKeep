# Conversation semantic index

The build reads current normalized records in key order, renders the same typed text parts as lexical search and divides each part into passages of at most 1,024 Unicode scalars. A SHA-256 key lets repeated text share a vector across records and conversations. The in-app static model and the user's OpenAI-compatible provider both receive document inputs for indexing and a query input for retrieval. On an update, unchanged distinct texts reuse the preceding model generation's quantized vectors.

The generation has two page families. Vector pages hold the content key, an f32 scale and signed int8 coordinates. Address pages map each event/chunk pair to its vector number. Pages are compressed before the Agent search role writes them. An encrypted archive authenticates and encrypts each page and the serving manifest. Search scores all serving vectors, fans positive scores back out to passages and checks current visibility, filters and content before delivery. The renderer's scalar offsets are relative to a typed part. The reader opens the physical record carried by the result. [Command and job contracts](../../desktop-command-surface.md#agent-conversation-ai-access)

## Linux release calibration

The existing Records search fixture has 120,000 Pi roots with 120 records each. Its user prompts repeat the same text, as do its ordinary replies. There is one distinctive reply. A second fixture adds the root and turn identifiers to each user prompt. Both fixtures have exactly 14,400,000 nonempty passages. The second fixture exercises embedding throughput and vector storage with 3,600,002 unique inputs. Neither distribution is a prediction of a person's archive.

The machine had four Linux x86-64 vCPUs and 16 GiB RAM. Each build used a fresh plaintext archive and a new process loading the verified production `minishlab/potion-multilingual-128M` files. The clock starts before model selection/loading and ends after publishing the generation. Fixture construction, native discovery, downloading the model and compilation are outside that interval. Size sums the logical file lengths of the serving manifest, vector pages and address pages. Model weights are shared with browsing and excluded.

From `src-tauri`, with `AH_MODELS_DIR` pointing at the app's models directory:

```sh
AH_SEARCH_ROOTS=120000 cargo test --release --locked -p vault-core --test conversation_embeddings build_conversation_embeddings -- --ignored --nocapture --test-threads=1
AH_SEARCH_ROOTS=120000 AH_DISTINCT_PROMPTS=1 cargo test --release --locked -p vault-core --test conversation_embeddings build_conversation_embeddings -- --ignored --nocapture --test-threads=1
cargo test --release --locked -p vault-core --test conversation_embeddings model2vec_ranks -- --ignored --nocapture
```

The ranking fixture compares “medical care for an ill dog” with two captured messages, one about veterinary treatment and one about SQL query planning. It checks the actual reader address, returned ranking and the unrelated document's unquantized score. [Raw measurement lines](validation/semantic-index-linux.jsonl)

| Fixture                |   Passages | Distinct vectors | Build seconds | Published bytes |
| ---------------------- | ---------: | ---------------: | ------------: | --------------: |
| Existing search corpus | 14,400,000 |                3 |       124.847 |      16,216,522 |
| Distinct user prompts  | 14,400,000 |        3,600,002 |       574.915 |     970,026,069 |

Peak resident memory for the distinct-prompt process, including fixture setup and model loading, was 488,804 KiB. The veterinary passage scored 0.6486 after quantization. The unrelated SQL passage had a float cosine of -0.0056 and fell below the positive-match threshold.
