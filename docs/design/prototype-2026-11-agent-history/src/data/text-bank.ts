/** Invented conversation text. Nothing here comes from real transcripts. */

/** [short title, the first thing the user typed]. */
export const openings: readonly (readonly [string, string])[] = [
  ['修正匯入時區錯位', 'Safari 匯入之後，凌晨的紀錄都跑到前一天去了。先找出時區是在哪一層被丟掉的，不要直接改，先跟我說原因。'],
  ['搜尋改成串流回傳', '搜尋現在只回前 100 筆，我要它邊搜邊回、可以一直往下捲到全部結果。先看現在 query 層怎麼寫的。'],
  ['重新加密流程加進度條', 'rekey 跑大檔的時候 UI 完全沒反應，幫我加進度回報，事件要能取消。'],
  ['E2E 在 CI 偶發逾時', 'E2E 的 search-index 那組有時候會超時，看一下最近三次失敗的 log，找出是不是同一個原因。'],
  ['寫 release notes', '幫我根據 CHANGELOG 寫這版的 release notes，給一般使用者看的，不要寫內部代號。'],
  ['設定頁拆成分頁', '設定頁太長了，拆成左側導航加分頁，每個分頁一個檔案。'],
  ['升級 Tauri 到最新版', '把 Tauri 升到最新版，列出 breaking changes 跟我們要改的地方，先不要動 code。'],
  ['清掉沒用到的依賴', '找出 package.json 裡沒被用到的依賴，確認之後再刪。'],
  ['Refactor tokenizer for CJK', 'The tokenizer splits CJK text into single characters. Make phrase search require adjacency without breaking the existing index format.'],
  ['Add retry to the sync worker', 'The sync worker gives up after one network error. Add exponential backoff with a cap, and make it cancellable.'],
  ['Why is the build 40% slower', 'Release builds got about 40% slower since last week. Bisect the dependency bumps and tell me which one.'],
  ['資料夾結構重整', '把 src/features 底下的檔案照功能重新分，超過 400 行的先列出來。'],
  ['列表虛擬化卡頓', '一萬筆以上的時候往下捲會掉幀，用 profiler 看是哪個元件在重算。'],
  ['匯出 Markdown 格式', '對話匯出成 Markdown 時，工具呼叫要摺起來，程式碼區塊要保留語言標記。'],
  ['資料庫 migration 寫法', '新增一個欄位要怎麼寫 migration 才不會動到 v1 schema？先看現有的 migration 範例。'],
  ['修 dark mode 對比不足', '深色模式下次要文字對比不夠，掃過所有用到 muted-foreground 的地方，列出低於 4.5:1 的。'],
  ['圖表 tooltip 被裁掉', '卡片邊緣的圖表 tooltip 會被切掉，改成 portal 到 body。'],
  ['論文圖表重畫', '把 figure 3 和 4 用同一套配色重畫，字型要跟內文一致，輸出 PDF 和 PNG。'],
  ['整理讀書筆記', '把這週的讀書筆記整理成一份大綱，每章三到五個重點，引用要保留頁碼。'],
  ['報稅資料整理', '把這個資料夾裡的收據整理成表格，依類別加總，金額對不上的標出來。'],
  ['寫一個備份腳本', '寫一個 shell 腳本，每天把 ~/notes 打包到外接硬碟，保留最近 14 份。'],
  ['nvim 設定除錯', 'nvim 開大檔會卡，看一下是哪個 plugin，給我最小的設定改法。'],
  ['Investigate memory leak', 'The dev server memory grows to 6 GB after an hour. Take two heap snapshots and diff them.'],
  ['Write tests for the parser', 'Before touching the parser, write down every way the date parsing can fail, then write tests for each.'],
  ['Explain this crash', 'Here is a crash report from a user. Explain what happened and where in our code it starts.'],
  ['Port the scraper to Rust', 'Port the Python scraper to Rust. Keep the same CLI flags and output format.'],
  ['API 分頁改 cursor', '列表 API 現在用 offset 分頁，資料一多就慢。改成 cursor，前端也要跟著改。'],
  ['登入流程改用 passkey', '研究一下把登入改成 passkey 要改哪些地方，先寫一份計畫。'],
  ['把設計稿轉成元件', '把這張設計稿轉成 React 元件，用我們現有的 shadcn 元件，不要新增依賴。'],
  ['檢查 PR 的邊界情況', '幫我 review 這個 PR，重點看邊界情況和錯誤處理，不用管風格。'],
  ['部落格文章校對', '校對這篇部落格文章，語氣不要變，只改錯字跟不通順的句子。'],
  ['旅行行程規劃', '幫我把這份行程整理成每天的時間表，交通時間要算進去。'],
  ['履歷英文版', '把中文履歷翻成英文版，專案描述改成成果導向的寫法。'],
  ['Benchmark the new index', 'Run the search benchmark on 14M rows with the new index and compare p50/p95 with main.'],
  ['Fix flaky date test', 'The date test fails around midnight UTC. Find out why and make it independent of the machine time zone.'],
  ['收據 OCR 準確度', 'OCR 對手寫的收據常常讀錯金額，試試先做影像前處理，比較前後準確度。'],
  ['WebSocket 斷線重連', 'WebSocket 斷線之後前端不會自動重連，補上重連跟狀態提示。'],
  ['把 CLI 加上 --json', 'CLI 的每個子命令都加上 --json 輸出，格式寫進文件。'],
  ['快捷鍵衝突', '⌘K 在輸入框裡會跟瀏覽器預設衝突，整理一下所有快捷鍵的優先順序。'],
  ['Docker 映像瘦身', 'Docker 映像現在 1.8 GB，看能不能用 multi-stage 壓到 300 MB 以下。'],
]

export const followUps: readonly string[] = [
  '跑一下測試',
  '好，照這個方向改',
  '這個 diff 第 42 行為什麼要 clone？',
  '先不要 commit，我想再看一下',
  '可以，commit 吧',
  '改用 spawn_blocking 試試',
  'README 也要一起更新',
  '錯誤訊息要寫給使用者看的，不要露出內部名稱',
  '這樣大資料量的時候會不會卡？',
  '再跑一次 E2E',
  '把剛剛那段抽成一個函式',
  '為什麼這裡要用 unwrap？',
  'Looks good, open the PR',
  'Can you double-check the edge case when the list is empty?',
  'Revert that last change',
  'What does the profiler say now?',
  '繼續',
  '先停一下，你改到不該改的檔案了',
  '把結果整理成表格',
  '這個 log 太吵，降到 debug',
]

export const assistantReplies: readonly string[] = [
  '找到原因了：匯入時把時間戳當成 UTC 解析，但 Safari 存的是本地時間。改動只在 `import/safari.rs` 一處，其他地方不受影響。',
  '測試都過了（412 passed）。另外我注意到 `query/sessions.rs` 有一個沒被用到的參數，先不動它，記在待辦裡。',
  '我把分頁改成 cursor 了。前端的列表改用 `useInfiniteQuery`，捲到底會自動載下一頁。\n\n```ts\nconst query = useInfiniteQuery({\n  queryKey: keys.list(filter),\n  queryFn: ({ pageParam }) => client.list({ filter, cursor: pageParam }),\n  getNextPageParam: (page) => page.nextCursor,\n})\n```',
  '| 指標 | main | 這個分支 |\n| --- | --- | --- |\n| p50 | 38 ms | 21 ms |\n| p95 | 140 ms | 64 ms |\n\n兩次都在同一台機器上輪流跑，差距穩定。',
  'Done. The retry now backs off 1s, 2s, 4s… up to 60s, and cancelling the job stops it between attempts.',
  '我先列出會受影響的地方，還沒改任何檔案：\n\n1. `settings-page.tsx` 拆成 6 個分頁元件\n2. 路由加上 `/settings/:section`\n3. 左側導航沿用現有的 `SettingsNav`',
  'The leak comes from the file watcher: every reload adds a listener and nothing removes it. Fix is a cleanup in the effect.',
  '改好了，PR 在這裡。CI 還在跑，大概是平常的時間。',
  '這個寫法在一千四百萬筆時會整表掃描，我改成先用索引縮小範圍再排序，實測從 9.2 秒降到 140 毫秒。',
  '我沒有找到這個檔案，可能已經被移走了。要我在整個專案裡找同名的檔案嗎？',
]

export const thinkingTexts: readonly string[] = [
  '使用者要的是原因，不是修法。先找出時間戳在哪裡被解析，再確認 Safari 的欄位是本地時間還是 UTC。',
  'The failing test only fails near midnight, so it probably builds "today" from the machine clock. Check the fixture.',
  '這裡有兩種做法：一是在 query 層加 cursor，二是前端自己分批。第一種比較乾淨，大資料量時也不會整表掃描。',
  '先別急著改，看一下 git log 最近誰動過這個檔案，可能是有意為之。',
]

export const bashCommands: readonly string[] = [
  'cargo test -p agent-history',
  'bun run check:base',
  'git log --oneline -15',
  'rg -n "open_archive" src-tauri',
  'bun run test:e2e -- --grep search',
  'ls -la src/features',
  'git diff --stat',
  'cargo clippy --workspace -- -D warnings',
  'du -sh var/playwright',
  'gh pr view --json title,state',
]

export const filePaths: readonly string[] = [
  'src/features/history/history-page.tsx',
  'src/lib/backend-client/search.ts',
  'src-tauri/crates/vault-core/src/query/sessions.rs',
  'src-tauri/crates/vault-core/src/import/safari.rs',
  'src/features/settings/settings-page.tsx',
  'src/components/app/heatmap.tsx',
  'docs/features/search.md',
  'README.md',
  'package.json',
  'src-tauri/Cargo.toml',
  'src/lib/i18n/messages/zh-TW.ts',
  'tests/e2e/search.spec.ts',
]

export const subagentTasks: readonly (readonly [string, string])[] = [
  ['Explore', '找出所有呼叫 open_archive 的地方'],
  ['Explore', 'Map the settings page and its sections'],
  ['general-purpose', '跑 E2E 並整理失敗的測試'],
  ['Plan', '規劃 cursor 分頁的改法'],
  ['code-reviewer', 'Review the diff for edge cases'],
  ['Explore', '查 Safari 匯入的時間戳來源'],
  ['general-purpose', 'Bisect the dependency bump'],
]

export const codexSubagentTasks: readonly (readonly [string, string])[] = [
  ['auto-review', '自動審查這一輪的修改'],
  ['worker', '平行跑 lint 與單元測試'],
]

export const projectWords = {
  prefix: [
    'orbit', 'lumen', 'tide', 'kiln', 'quartz', 'fern', 'atlas', 'ember', 'pixel', 'cobalt',
    'harbor', 'sable', 'willow', 'delta', 'nimbus', 'rune', 'pebble', 'vela', 'cinder', 'moss',
    'aurora', 'basalt', 'cirrus', 'dune', 'echo', 'flint', 'grove', 'halo', 'iris', 'juniper',
    'koi', 'lark', 'maple', 'nova', 'onyx', 'prism', 'quill', 'reef', 'sage', 'talon',
    'umber', 'vapor', 'wren', 'yarrow', 'zephyr', 'birch', 'comet', 'drift', 'fable', 'glyph',
  ],
  suffix: [
    'notes', 'api', 'charts', 'cli', 'web', 'bot', 'sync', 'lab', 'kit', 'docs',
    'site', 'app', 'scraper', 'agent', 'ui', 'core', 'server', 'tools', 'bench', 'viz',
  ],
  folders: [
    '論文圖表', '報稅 2025', '旅行規劃', '履歷', '讀書筆記', '收據掃描', '婚禮名單', '投影片',
    '家庭照片整理', '研究計畫', '課程講義', '採訪逐字稿', '翻譯稿', '提案草稿', '會議紀錄',
    'reading-list', 'invoices', 'scratch', 'design-review', 'talk-slides', 'mockups', 'datasets',
  ],
} as const

/** Everyday replies, so a search for one technical word doesn't hit every conversation. */
export const plainReplies: readonly string[] = [
  '好，我先看一下現在的寫法再動手。',
  '改好了，跑過一次測試都通過。',
  '這裡我不太確定你的意思：是要整個換掉，還是只改這個函式？',
  '我照你說的把它抽出來了，原本三個地方都改成呼叫新的函式。',
  '測試有一個沒過，是時區相關的，我看一下是不是本來就會偶發。',
  '已經還原到上一個版本。',
  '我列了三個做法，各自的取捨寫在下面：\n\n1. 直接改現有函式，改動最小\n2. 新增一層包裝，舊的呼叫不用動\n3. 整個重寫，最乾淨但要改十幾個檔案',
  'Done — the change is small, two files.',
  'I could not reproduce it locally. Can you paste the exact command you ran?',
  '這個警告可以先不管，它來自依賴本身，不影響我們的程式。',
  '我把結果整理成表格了：\n\n| 項目 | 之前 | 之後 |\n| --- | --- | --- |\n| 啟動時間 | 2.4 秒 | 0.9 秒 |\n| 記憶體 | 610 MB | 420 MB |',
  'README 也一起更新了，範例改成新的參數名稱。',
  '這段我先不 commit，等你看過再說。',
  '找到了，是設定檔裡的路徑寫死了，換台電腦就會壞。',
  'lint 有兩個錯，都是沒用到的 import，已經清掉。',
  '這樣改在資料很多時也不會卡：我把排序移到資料庫那邊做了。',
]
