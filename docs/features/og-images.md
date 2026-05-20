# OG-IMAGES — 卡片模式的網頁預覽快取

> v0.3 paper redesign 引入的 Browse 卡片模式視覺。讓每張 card 顯示頁面真正的 og:image
> 社交卡，而非僅僅 favicon 或 domain 色塊；圖片以 byte-identical dedup 儲存，使用者可
> 隨時關掉 fetch、清空快取，或設定 LRU / 大小 / 時間驅動的自動清理。

---

## 1. 為什麼有這層

- v0.3 paper Browse 的卡片模式需要更高訊息密度的視覺。Favicon 16 × 16 在 16:10 frame 裡
  始終顯得貧乏；og:image 是頁面作者自己選定的「這頁長什麼樣」官方表達。
- 但 og:image bytes 不能塞回瀏覽器歷史檔案（瀏覽器不存它）也不適合 backup 時即時抓
  （數據主權 / 隱私 / 速度都不允許）。所以這層是獨立的、lazy、user-controllable 的網路
  快取層。
- 使用者明確規定：**不能用 host 級別 cache**。GitHub 上每個 repo / Medium 上每篇文章
  的 og:image 都不同；只能 per-URL key。bytes 完全相同時可以 dedup（hash-addressed）。

## 2. 範圍

### 在範圍內

- 每個 page URL 一個 og:image 快取列（`og_images`）；二進位 bytes 內容定址 dedup
  在 `og_image_blobs`。
- HTTPS-only 的網路 fetch：解析 `<meta property="og:image:secure_url">` →
  `og:image` → `twitter:image` → `twitter:image:src`，HTTP body 上限 1 MiB、image
  body 上限 2 MiB、單一 redirect、12 秒 total timeout。
- 顯式 fetch 狀態（`ok` / `missing` / `http_error` / `parse_error` / `too_large` /
  `unsupported_mime` / `blocked`）+ 負緩存 + 退避 refetch_after，避免 retry storm。
- 使用者開關 `og_image.fetch_enabled`（預設 on）、per-domain blocklist、與四選一的清理
  策略（`Off` / `TimeTtl` / `SizeCap` / `Lru`，預設 `Off`）。
- Card mode 渲染優先級：og:image > favicon > domain swatch。List mode 仍只用 favicon。
- `last_shown_at` 由前端 debounced batched 命令更新，作為 LRU 驅逐的信號。

### 不在範圍

- og:image 之外的頁面 metadata（標題正規化、作者、發表時間、Open Graph 全集）—— 那是
  `WORK-READABLE-CONTENT-V03-A` 的範圍，本層只負責那一張預覽圖。
- 圖片解碼 / dimension 量測。Width / Height 留 NULL；瀏覽器 render 時自己量內在尺寸。
  避免拉進 libwebp / libavif 等需要 C 編譯的依賴，與 AGENTS.md 紅線一致。
- 跨設備同步。og:image 快取是 derived，每台機器自己重抓；backup / restore 不帶它。
- Refetch 排程器。第一版的 refetch 走「Settings 手動觸發」與「卡片 lazy 觸發」兩條路；
  排程化清理走每日 schedule.rs tick。

## 3. 後端

### 資料表（migration `012_og_images.sql`）

```sql
CREATE TABLE og_image_blobs (
  blob_hash   TEXT PRIMARY KEY,
  image_data  BLOB NOT NULL,
  mime        TEXT NOT NULL,
  byte_size   INTEGER NOT NULL,
  width       INTEGER,
  height      INTEGER,
  recorded_at TEXT NOT NULL
);

CREATE TABLE og_images (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  page_url          TEXT NOT NULL,
  page_host         TEXT,
  source_og_url     TEXT,
  image_blob_hash   TEXT REFERENCES og_image_blobs(blob_hash),
  fetch_status      TEXT NOT NULL,
  http_status       INTEGER,
  fetched_at        TEXT NOT NULL,
  last_shown_at     TEXT,
  refetch_after     TEXT,
  fetch_attempts    INTEGER NOT NULL DEFAULT 1,
  created_by_run_id INTEGER REFERENCES runs(id)
);

CREATE UNIQUE INDEX idx_og_images_page_url     ON og_images(page_url);
CREATE INDEX idx_og_images_blob_hash    ON og_images(image_blob_hash) WHERE image_blob_hash IS NOT NULL;
CREATE INDEX idx_og_images_refetch      ON og_images(refetch_after)   WHERE refetch_after   IS NOT NULL;
CREATE INDEX idx_og_images_last_shown   ON og_images(last_shown_at)   WHERE last_shown_at   IS NOT NULL;
```

- `page_url` 是 exact lookup key。**讀路徑沒有 host fallback** — 兩個同 host 不同
  page 不會互相讀到彼此的預覽。`page_host` 只用於診斷 / 與 blocklist 比對。
- `og_image_blobs.blob_hash` 是 `utils::sha256_hex(bytes)`。多個 page 共用同一張 og:image
  時只佔一份儲存。
- `fetch_status` 是字串列舉，避免 schema 寫死 enum；常數定義在 `og_images::fetch_status`
  module 裡，所有寫 / 讀 / 統計路徑同字面值。

### vault-core API（`src-tauri/crates/vault-core/src/archive/history/og_images.rs`）

| Function                                | 用途                                                                            |
| --------------------------------------- | ------------------------------------------------------------------------------- |
| `upsert_og_image(conn, insert)`         | 寫一筆 fetch 結果。bytes 走 dedup；row 採 delete-then-insert 確保欄位完全替換。 |
| `load_og_images(paths, config, key, e)` | 依 `HistoryOgImageLookupEntry[]` 批次取回 `HistoryOgImageLookupResult[]`。      |
| `mark_og_images_shown(conn, urls)`      | 批次更新 `last_shown_at`（LRU 驅逐信號）。空輸入 no-op。                        |
| `storage_stats(conn)`                   | 回 `OgImageStorageStats { rows, blobs, total_bytes, oldest_fetched_at }`。      |
| `clear_cache(conn)`                     | 清空兩表，回 `OgImageCleanupReport`。                                           |
| `run_cleanup(conn, mode)`               | 跑一次 user-chosen 驅逐 + orphan blob GC。                                      |

### Fetch 流程（`og_images_fetch.rs`）

- `build_fetch_client()`：reqwest blocking client。UA `PathKeep/0.3 (link-preview;
data-sovereignty)`、無 Referer、connect 8 s / total 12 s、最多 1 次 redirect、
  https-only 由我們在進入點手動強制（http:// page URL 直接 `parse_error`）。
- `fetch_og_image_for(client, page_url)` → `FetchedOgImage`：執行整條 pipeline，永遠
  回一個可持久化的結果（成功或負緩存）。
- `extract_og_image_url(html, page_url)`：scraper 解析、selector 優先順序如上節，
  相對 URL 對 page URL 解析、http→https 升級。
- `is_host_blocked(blocked_hosts, page_url)`：blocklist 比對大小寫無關。
- `blocked_outcome(page_url)`：blocklist 命中時的 zero-network outcome，仍寫一筆
  `fetch_status='blocked'` 的 row。

### Tauri commands

| Command                      | Worker bridge                                | Vault worker                           |
| ---------------------------- | -------------------------------------------- | -------------------------------------- |
| `load_history_og_images`     | `worker_bridge::load_history_og_images_impl` | `vault_worker::load_history_og_images` |
| `mark_og_images_shown`       | `worker_bridge::mark_og_images_shown_impl`   | `vault_worker::mark_og_images_shown`   |
| `trigger_og_image_refetch`   | `worker_bridge::refetch_og_images_impl`      | `vault_worker::refetch_og_images`      |
| `get_og_image_storage_stats` | `worker_bridge::og_image_storage_stats_impl` | `vault_worker::og_image_storage_stats` |
| `clear_og_image_cache`       | `worker_bridge::clear_og_image_cache_impl`   | `vault_worker::clear_og_image_cache`   |
| `run_og_image_cleanup`       | `worker_bridge::run_og_image_cleanup_impl`   | `vault_worker::run_og_image_cleanup`   |

所有 command 都吃 session database key；archive 鎖住時不回資料。所有命令跑在
`run_blocking_command` 裡，網路 / SQLite 都不阻塞 Tauri command thread。

### Refetch 安全性

- `refetch_og_images` 在進入點檢查 `config.og_image.fetch_enabled`。關閉時整批
  no-op，回 `Ok(0)`。
- 每個 URL 個別檢查 blocklist；命中即寫 blocked row，不發任何 HTTP。
- 第一版採嚴格序列 fetch（單一 client、無 parallelism）。未來可加 per-host token
  bucket / parallelism cap，呼叫合約不變。

## 4. 前端

- `src/lib/backend-client/explorer.ts` + `src/lib/backend.ts`：6 個 typed methods
  `loadHistoryOgImages` / `markOgImagesShown` / `triggerOgImageRefetch` /
  `getOgImageStorageStats` / `clearOgImageCache` / `runOgImageCleanup`。
- `src/pages/explorer/hooks/use-explorer-og-images.ts`：lazy hydration hook。
  - 鏡像 `useExplorerFavicons` 的 dedup + inflight + cache-token 行為，但 key 只用 URL
    （og:image 是 page-level，沒有 visit-time / profile scope）。
  - debounced（1 s）批次呼叫 `markOgImagesShown`，把可見 URL 推進 LRU 訊號裡。
  - `enabled` prop 讓 list mode 可以完全跳過整個 pipeline。
- `src/components/explorer-paper/paper-contact-frame.tsx`：渲染順位 og:image > favicon
  > domain swatch。og:image 在時，背景轉黑、`<img>` `object-cover` 填滿 16:10 區域、
  > 加上頂底 scrim 讓 index / transition token 在任何圖上都可讀。
- `src/pages/settings/link-previews-section.tsx`：Settings → Link previews
  subsection。
  - Fetch toggle 透過 `saveConfig` 寫 `AppConfig.ogImage.fetchEnabled`。
  - 即時顯示 `getOgImageStorageStats` 結果。
  - Run-cleanup / Clear-all（後者有 `window.confirm` guard）。
  - 三語 i18n keys（`settings.linkPreviews*`）位於
    `src/lib/i18n/catalog/settings-core-and-platform.ts`。

## 5. 與其他系統的互動

- **Backup / restore**：og:image 快取是 derived（從第三方網路狀態重生），**不進入
  backup export**。Migration 012 在 restore 後跑出空表，正常 lazy 重抓。
- **App lock**：所有 og:image 路徑共用 session database key；解鎖前命令直接 reject。
- **Schedule tick**：未來在 `schedule.rs` daily tick 接上 `run_og_image_cleanup`，
  在使用者選了 Time / Size / LRU 模式時自動回收；目前 `Off` 模式下 tick 仍跑一次
  orphan-blob GC，量級忽略不計。
- **List mode**：列表模式只渲染 favicon，不觸發 og:image 抓取。`useExplorerOgImages`
  在 list mode 接到 `enabled=false`。

## 6. 後續 backlog

- **Settings 完整版**：blocklist textarea、eviction-mode 分段選擇器、TimeTtl/SizeCap/Lru
  的數值輸入。後端 + AppConfig 已備好，只缺 UI。
- **Parallelism + per-host rate limit**：`refetch_og_images` 改成多 worker + per-host
  token bucket（每 host ≥ 500 ms 間距），減少對單一 host 的脈衝。
- **Schedule tick**：把 `run_og_image_cleanup` 接進 daily schedule.rs 自動觸發。
- **Width / height 量測**：若未來需要在 Settings stats 顯示「平均尺寸」，可加
  pure-Rust `image` crate（features = ["png", "jpeg"]），保持 vcpkg 紅線。
- **Negative-cache TTL 重抓**：worker 自動掃 `refetch_after < now` 的 row 並重試一次。
- **匯入時的離線抓取**：與 `WORK-READABLE-CONTENT-V03-A` 對齊，將 og:image 抓取納入
  該 work block 的 batch import 流程。
