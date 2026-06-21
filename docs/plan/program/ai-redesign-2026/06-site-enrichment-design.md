# AI Redesign 2026 — Site-Specific Content Enrichment (W-ENRICH) Design

> 狀態：2026-06-21 設計鎖定（獨立 subagent 評估產出）。承 [05-embedding-retrieval-revision.md](05-embedding-retrieval-revision.md)。**取代並解封** `BACKLOG.md` 的 `WORK-READABLE-CONTENT-V03-A`（其 4 個 blocker：privacy model / network policy / failure UX / real-site acceptance 由本設計解掉）。
>
> 目標：從特定網站抓取結構化內容（GitHub repo 描述/topics、YouTube/Bilibili 字幕+標題、X 帖文、通用網頁摘要），存為 enrichment metadata，**同時餵進 FTS5 全文搜尋 + embedding/AI 語義搜尋**。

## 0. 重用既有基礎（不要重造）

- **enrichment plane 已存在**：`vault-core/src/enrichment.rs` 的 `visit_content_enrichments`（per `(history_id, content_source)`：fetch_status / final_url / language / readable_title / content-addressed `readable_text_blob_path` + text_hash / snippet_json / extraction_json / pipeline_version）。`load_best_enrichment_map_by_history_ids`（優先序解析）+ `build_embedding_content_from_parts`（enrichment→embedding 唯一漏斗）。`READABLE_CONTENT_PLUGIN_ID` 目前是 `deferred` stub，但已有寫好卻 `#[allow(dead_code)]` 的 HTML extractor（`build_enrichment_result_from_html`）+ site-adapter 框架（`enrichment_site_adapters.rs` 按 domain 路由 YouTube/Vimeo、抽 JSON-LD VideoObject）。**= extractor 框架的骨架，只是沒餵真實 bytes。**
- **egress 模型已解決且實戰驗證**：`archive/history/og_images_fetch.rs`——desktop Chrome UA + Sec-Fetch/Sec-CH-UA client hints、Accept-Language、**無 Referer/cookies/fingerprinting**、redirect limited(8)、15s/10s timeout、https_only、body cap（1 MiB）、MIME guard、**negative-cache cadence**（按失敗類型）。**SSRF guard** `net_guard::url_target_is_blocked`（拒私有/loopback/cloud-metadata IP）。`og_images_synth.rs` 已呼叫 Bilibili 公開未授權 API——證明中等難度 side-channel API 在此 posture 內可行。
- **FTS5**：`archive/search_projection.rs`，`SEARCH_PROJECTION_SCHEMA_VERSION=2`（mismatch → drop+rebuild）。
- **content-hash 對齊**（doc 05）：`content_hash = hash(canonical_url + title + enrichment_summary)` → **enrichment 嚴格上游於 embedding 且是 dedup key 的一部分**。

## 1. Extractor 框架

把 site-adapter 升為一等 `Extractor` trait（新 `enrichment/extractors/`）：`id()`（"github-repo"/"youtube-captions"/"generic-readable"）、`version()`（bump → 有界重抓）、`matches(url)`（domain 路由）、`fetch_kind()`（Html/JsonApi/UrlSynth/None）、`extract(ctx)`。`Vec<Box<dyn Extractor>>` registry，first-match-wins，**generic-readable 為終極 fallback**。**job runner 透過唯一 shared `build_fetch_client()` 做所有 egress**——extractor 自己永不連網 → 隱私 posture 在一處強制、無法被繞過。輸出映回既有 `EnrichmentResult`，`content_source` = extractor id。

### 各源可行性（無 auth）

| 源              | 機制                                                                                                 | 難度 | 判定                                    |
| --------------- | ---------------------------------------------------------------------------------------------------- | ---- | --------------------------------------- |
| **GitHub repo** | 公開 REST `api.github.com/repos/{o}/{r}`（desc/topics）+ `/readme`；未授權 **60 req/hr/IP** 須硬限流 | 易   | **MVP**                                 |
| **通用網頁**    | 既有 readability extractor；可選 LLM 摘要                                                            | 易   | **MVP**                                 |
| **YouTube**     | oEmbed（標題/作者，可靠）+ `timedtext`（字幕**脆弱**，常需簽名/innertube 參數、常空）                | 中   | P2：metadata 可靠、字幕 best-effort     |
| **Bilibili**    | `x/web-interface/view`（標題/desc，og:image 已用）+ `x/player/v2` 字幕（無登入常空）                 | 中   | P2：metadata 可、字幕 best-effort       |
| **X/Twitter**   | `publish.twitter.com/oembed`（部分公開帖）；full API auth-walled/反爬/高成本                         | 難   | **延後**（oEmbed-only 或跳過，UI 標明） |

字幕/transcript 長 → **永不存原始**，cap + 摘要（§4）。

## 2. 隱私 / 網路政策（領頭；解封 WORK-READABLE-CONTENT-V03-A）

- **(a) privacy model**：**新增 hard-default-off `AiSettings.content_fetch_enabled`**（獨立於只管離線 title plugin 的 `enrichment_enabled`）+ per-extractor enable + per-domain allow/block。egress 邊界（02 §H）→ 預設關、逐目標 consent、PME 預覽目的 host。目的站只學到：IP、desktop UA、Accept-Language、URL path（使用者本就訪問過）——**無 cookies/Referer/帳號**。
- **(b) network policy**：https-only；每個 page URL + 每個 API 子資源都過 SSRF guard；redirect limited(8)；timeout；body cap（HTML 1 MiB + JSON cap）；**per-host token-bucket 限流**（GitHub 60/hr 關鍵）。**離線優先硬規則**：永不在 backup/import 關鍵路徑；搜尋/explorer **永不阻塞於網路**（讀已抓的 enrichment 列；沒抓過 → 退 title/URL）。
- **(c) failure UX**：重用失敗分類 + negative-cache cadence（不 retry-storm）；失敗永不阻塞搜尋；login/PDF/非-HTML/限流 → 誠實狀態、不假裝。
- **(d) real-site acceptance**：比照 og:image 的 mockito fixture（GitHub JSON、有/無字幕影片、可讀文章、paywall/非-HTML、redirect chain、429）；可選 LLM 摘要走本機 LM Studio 驗收。

## 3. 存儲 + content-hash 對齊

擴 `visit_content_enrichments`（derived/可重建，非 canonical），加：

- `extractor_version INTEGER`——extractor 升級只重抓自己的列（有界）。
- `enrichment_summary TEXT`（cap ~280 char）——**參與 `content_hash` 的 canonical 短欄位**，inline（小）以免讀 blob。

content-hash 流：新 summary → hash 變 → embedding 層（`indexing.rs` `needs_embedding` diff）自動標記重嵌。**fetch 以 URL 為鍵**（5000 gmail visit 抓一次，fan 到所有 visit）= doc 05 §1 的「最大近免費槓桿」。**export**：排除原始 caption/body blob（可重抓/可能大/可能 stale），**含 capped `enrichment_summary`**（離線仍可搜）。

## 4. FTS5 + embedding 整合

- **FTS5**：`SEARCH_PROJECTION_SCHEMA_VERSION` 2→3（auto drop+rebuild）；`search_documents` + FTS 表加 `enrichment_text`，rebuild/refresh 時 LEFT JOIN best enrichment per URL。**只存 capped summary + 關鍵 metadata（GitHub topics、channel、repo desc）——不存全 caption/body**（14.4M perf：多 KB transcript 灌進 FTS 會炸 index size + BM25）。全 body 留 blob，AI agent 按需取。
- **embedding**：`build_embedding_content_from_parts` 加 `readable_summary`/metadata 參數；**守 ~512 token cap**（doc 05）——短結構源（topics、標題+channel+desc）逐字；長字幕**摘要/cap**，絕不把 1 小時 transcript 餵進 512-token 窗。

## 5. Job / async + 14.4M 優先序

新 **`content-fetch` job type** 走既有 lease queue（off-thread/resumable/cancelable）：claim → SSRF guard → per-host 限流 → shared client fetch → route extractor → store → 失敗設 `refetch_after`。並發低（UI 流暢硬指標）。**永不全抓**：優先 heavy 工作集（= embedding selector：**starred（最高權重）∪ 近 12-24 月 ∪ tagged/noted ∪ 高頻 refind**），以 unique-URL 為單位，**不重抓未變**（negative-cache + extractor_version gate）。

## 6. UX（i18n ×3）

- Settings：master `content_fetch_enabled`（預設關）+ 網路政策揭露 + per-extractor toggle（GitHub/影片 metadata/通用摘要；X 標「受限/不可用」）+ per-domain allow/block + 限流說明。
- Detail panel：「Enriched content」區（GitHub desc+topic chips / 影片 channel+字幕摘錄 / 通用摘要）+ fetched-at/source + 手動「fetch now」PME。
- Search results：命中 `enrichment_text` 時顯示 enrichment 摘錄。全狀態 build 時三語。

## 7. 分期

- **MVP（W-ENRICH-1）**：框架 + **GitHub** + **generic-readable**（確定性、無 LLM）+ `content_fetch_enabled` consent + SSRF/限流/negative-cache + FTS5 v3 + `enrichment_summary` 進 content_hash + settings/detail UI。
- **P2（W-ENRICH-2）**：YouTube + Bilibili **metadata**（字幕 best-effort、明標）+ 通用頁可選 LLM 摘要。
- **P3 / 或不做**：X oEmbed-only，明標 experimental，可能無限延後。

## 8. 排序與風險

- **依賴**：上游於 embedding tiers（W-AI-4c/5/6）。**`enrichment_summary` 進 content_hash 須與 W-AI-4c（content-hash dedup）同時落地**，否則事後引入會使既有 embedding 全失效（re-hash blast radius）——或接受只對已 enrich 的 URL 有界重嵌。→ W-ENRICH-1 與 W-AI-4c 協同。
- GitHub 60/hr 是最緊約束（可選 user PAT 提到 5000/hr，consent-gated、`SecretString`）。
- 字幕 2026 普遍不可靠（YouTube timedtext 漸 gated、Bilibili 常登入牆）：ship metadata，transcript 當 bonus，不承諾。
- LLM 摘要 strictly opt-in + provider-gated；確定性摘錄為底線。

**Anchors**：`enrichment.rs`、`enrichment_site_adapters.rs`、`archive/history/{og_images_fetch,net_guard,og_images_synth}.rs`、`archive/search_projection.rs`、`models/intelligence.rs`、`ai/indexing.rs`、`migrations/012_og_images.sql`；docs `02 §A/§H`、`05 §1/§8`、`BACKLOG.md:266`。
