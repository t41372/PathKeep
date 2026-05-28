# ANNOTATIONS — 每網址筆記與標籤

> v0.3 paper redesign 引入的使用者可手寫資料。讓 Browse detail panel 寫下的 notes 與 tags
> 進入 canonical archive，跨 session、跨備份、跨 restore 保存下來。

---

## 1. 為什麼有這層

- v0.3 設計裡 Browse detail panel 有一段 Newsreader textarea 「Why did this matter? What
  were you looking for?」與一條 tag chip 列。沒有後端時，這些輸入只能塞 localStorage —
  換電腦、reinstall、清 cache 就消失，違反 PathKeep 的 data sovereignty 承諾。
- Notes / tags 是使用者親手寫的東西，跟 history visits（瀏覽器自己產生）不同，必須有
  audit timestamps，並且不能因為 derived state rebuild 而被清掉。
- 後續 Recall / 搜尋 / 匯出流程都會把 annotations 視為可搜尋的一級資料；單獨建一個 table
  讓欄位職責清晰，避免硬塞進 `urls` / `visits`。

## 2. 範圍

### 在範圍內

- 每網址一段 notes 文字（最長 16 KiB），與一組 tags（最多 64 個，每個最長 64 bytes）。
- Notes / tags 都跟 URL 綁定，不是 visit ID — 同一個頁面下次訪問仍會帶出上次寫的內容。
- Audit 時間戳（`created_at` / `updated_at`）與寫入時的 `source_profile`（純記錄用途）。
- Tags 在持久化前會被 trim、case-insensitive de-dupe，並依寫入時間升冪排序回傳。
- 提供 list / search API 給未來的 annotations browse / 匯出流程。

### 不在範圍

- 跨設備同步、衝突解決 — 同 PathKeep data sovereignty 原則一致，annotations 永遠是 local
  authoritative；多 device 場景由使用者自己用 export / import 處理。
- 標籤命名空間、tag aliasing、tag suggestion — 此版本只保留純字串，UI 不主動推薦。
- Rich-text 或 markdown — `notes` 是純文字。
- Full-text 排名、向量 / embedding — `search_annotations` 目前用 case-insensitive LIKE，
  之後若加上 FTS 索引，呼叫合約不變。

## 3. 後端

### 資料表（migration `011_notes_tags.sql`）

```sql
CREATE TABLE url_annotations (
  url            TEXT PRIMARY KEY,
  notes          TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL,
  source_profile TEXT
);

CREATE TABLE url_tags (
  url            TEXT NOT NULL,
  tag            TEXT NOT NULL,
  created_at     TEXT NOT NULL,
  source_profile TEXT,
  PRIMARY KEY (url, tag)
);

CREATE INDEX idx_url_annotations_updated_at ON url_annotations(updated_at DESC);
CREATE INDEX idx_url_tags_tag                ON url_tags(tag, url);
```

- `url` 直接是 primary key — 不依賴 `urls.id`，因為 annotations 是 user-authored、要在
  history table rebuild / repair 流程中保留，不能被 derived rebuild 影響。
- 寫入時若 notes 是空字串（trim 後），整個 `url_annotations` row 會被刪除；只剩 tags 的
  URL 仍會在 `url_tags` 裡。`get_annotation` 在兩個 table 都查不到時回傳 `None`。
- 一個 URL 的 tag 集合是「替換式」寫入（`replace_tags`），UI 不暴露 add-single / remove-single
  affordance，避免 race-y 的 partial mutation。

### vault-core API

| Function                                               | 用途                                        |
| ------------------------------------------------------ | ------------------------------------------- |
| `get_annotation(paths, config, key, url)`              | 讀單一 URL 的 `UrlAnnotation` 或 `None`。   |
| `set_notes(paths, config, key, request)`               | 寫 / 清 notes；空 body 自動刪 row。         |
| `replace_tags(paths, config, key, request)`            | 替換整組 tags；空 list 移除全部。           |
| `list_annotations(paths, config, key, limit)`          | 列出有 annotation 的 URL，updated_at desc。 |
| `search_annotations(paths, config, key, query, limit)` | notes 子字串搜尋（大小寫無關）。            |

寫入時的硬性限制：

- `notes` trim 後超過 16 KiB → 回傳 error，不寫入。
- `tag` trim 後超過 64 bytes → 回傳 error。
- 同一 URL 超過 64 個 tag → 回傳 error。

### Tauri commands

| Command                  | Worker bridge                            | Vault core                       |
| ------------------------ | ---------------------------------------- | -------------------------------- |
| `get_url_annotation`     | `worker_bridge::get_annotation_impl`     | `vault_core::get_annotation`     |
| `set_url_notes`          | `worker_bridge::set_notes_impl`          | `vault_core::set_notes`          |
| `replace_url_tags`       | `worker_bridge::replace_tags_impl`       | `vault_core::replace_tags`       |
| `list_url_annotations`   | `worker_bridge::list_annotations_impl`   | `vault_core::list_annotations`   |
| `search_url_annotations` | `worker_bridge::search_annotations_impl` | `vault_core::search_annotations` |

所有 command 都吃 session database key（與 archive 的 App Lock session 同層），加密 archive
鎖住時不會回傳資料。

## 4. 前端

- `src/lib/backend-client/annotations.ts` 提供 typed client：`backend.getUrlAnnotation`、
  `backend.setUrlNotes`、`backend.replaceUrlTags`、`backend.listUrlAnnotations`、
  `backend.searchUrlAnnotations`。
- `src/pages/explorer/use-desktop-annotations.ts`：backend-backed hook，optimistic
  cache + write-through。形狀與 `useLocalAnnotations` 一致，所以 Browse detail panel
  完全不知道後面接誰。
- `src/pages/explorer/index.tsx` 用 `hasDesktopCommandTransport()` 在 `useDesktopAnnotations`
  與 `useLocalAnnotations` 之間切換。Browser-preview build 仍走 localStorage。
- 「Saved · local」這條 mono 文字目前還是維持的；之後 surface 真實的 backend save state
  時會由 detail panel 自行調整。

## 5. 與其他系統的互動

- **Backup / restore**：annotations 跟 `urls` / `visits` / `import_batches` 同住 canonical
  archive，所以 backup / snapshot restore 自然把它們一起搬走。Backup 不會把 annotations
  視為 derived state 刪掉（不在 `clear_derived_intelligence` 範圍）。
- **Audit ledger**：寫入 annotations 不會產生新的 backup run，但 `source_profile` 欄位讓
  後續 audit 想 trace「這條筆記是誰在哪個 profile 寫的」時有來源 hint。
- **Search 入口**：未來 Recall 與 paper Search 都會把 `search_annotations` 接成第三條
  signal（與 history、semantic 並列），但目前的 paper Search 仍只用 history。

## 6. 後續 backlog

- `search_annotations` 接上 FTS 索引（單獨 migration），保留現有 API。
- Notes export / import（含 CSV 與 markdown），由 maintenance 流程觸發。
- Detail panel 用 backend save-state 取代「Saved · local」字串，配合 toast / inline
  status pill。
- Tag aliasing / canonical tag list 由 Settings 管理（先做使用者觀察再設計）。
