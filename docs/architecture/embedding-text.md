# What the embedding model reads

`WORK-AI-EMBED-INAPP` item 2 (2026-10-15). No schema changes.

## The page text

One vector stands for a page: every visit of the same canonical URL, title and summary shares
it (`ai/dedup.rs`). So the text describes the page and nothing about a visit
(`enrichment/embedding_document.rs`):

```text
{title}
{site} · {words of the address path}
{readable title, when it differs from the title}
{summary, when it differs from both titles}
{readable body}
```

- Each line before the body is cut at 256 characters, and the whole text at 2,048 (about 512
  tokens); the body gets what the other lines leave. Real titles are far shorter, but a page can
  put a whole article in its `<title>`, and one overlong text fails the whole batch at a service
  whose context window is small. Repeats are judged on the full texts; only what is sent is cut.

- The title is the browser's; without one, the readable title; without either, the text starts
  at the site line.
- The site is the host without `www.`. The path words are the percent-decoded path split at
  `/ - _ . + ~ ,`, without numbers, hex and UUID-like IDs or a page extension (`html`, `php`, …),
  at most 16. The query string and fragment are never included.
- Until 2026-10-15 the text began with `Profile:`, `Visited at:`, `URL:`, `Domain:` and
  `Title:` lines. That put the first visit's time and profile into a vector shared by every
  visit, and spent the static model's averaged tokens on labels and URL noise.

Fetched page bodies are read without the text of `script`, `style`, `noscript`, `template`
and `svg` elements. Before, a page without paragraph blocks fell back to the whole body's text,
CSS and JavaScript included. The generic extractor is version 2, so the next content fetch the
user runs reads those pages again; nothing fetches by itself.

## Model prompts

Some embedding models are trained with a prefix on queries, documents or both. The prompt
profile is chosen from the model ID alone, never the provider kind (`ai/embedding_prompt.rs`):

| Family                                 | Query                                                          | Document                                 |
| -------------------------------------- | -------------------------------------------------------------- | ---------------------------------------- |
| Qwen3-Embedding                        | `Instruct: {task}\nQuery:{q}`                                  | as is                                    |
| EmbeddingGemma                         | `task: search result \| query: {q}`                            | `title: {title or none} \| text: {body}` |
| nomic-embed-text                       | `search_query: {q}`                                            | `search_document: …`                     |
| E5 (instruct)                          | `Instruct: {task}\nQuery: {q}`                                 | as is                                    |
| E5                                     | `query: {q}`                                                   | `passage: …`                             |
| mxbai-embed-large                      | `Represent this sentence for searching relevant passages: {q}` | as is                                    |
| anything else, the in-app static model | as is                                                          | as is                                    |

The task is `Given a search query, retrieve the web pages from the user's browsing history that
match it`, in English as both Qwen3 and E5 recommend for multilingual use. Each query gets its
prefix once, on every path that embeds a query; documents are rendered where they are built,
since they need the title on its own.

## Telling an index made from older text apart

The vector file's header (`ai/vector_store.rs`) records `embeddedText`:
`{EMBEDDED_TEXT_VERSION}/{document format}`, where the document format is `plain` for families
whose documents are sent as is. A query-only change does not touch it. It is written when the
file is created (first build or rebuild); appending never rewrites it, so it names the format of
the oldest vectors in the file. Files written before this change have no field.

It is kept out of the embedding fingerprint on purpose. A fingerprint mismatch means another
vector space: search refuses the index (`ConfigDriftFingerprint`) and an incremental build stops.
An index made from the older text is the same model's vector space and still finds pages, only
less well. So it keeps serving and keeps taking new visits after backups; its status is `stale`
with reason `textFormat`, which comes before the watermark and enrichment reasons because only a
rebuild clears it. Settings says so and offers Rebuild index (Start over for an unfinished
index); History's semantic note says what it was made from.

Feature acceptance: `tests/e2e/semantic-index.spec.ts`, second test.
