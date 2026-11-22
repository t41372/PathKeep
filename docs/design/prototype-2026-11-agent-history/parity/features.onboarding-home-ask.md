# Feature parity: onboarding, Home, Ask, full-window screens

Read-only audit of 2026-11-13. It starts from the real app (committed `HEAD`, not the working-tree edits in progress) and looks for each feature in the prototype code and in the spec. A message-string check cannot find a lost control, rule, state or shortcut, so this one lists them.

**Paths.** `src/…` is the real app. `proto/…` is `docs/design/prototype-2026-11-agent-history/src/…`. `ds` is `docs/features/data-spaces.md`, `to` is `docs/design/data-spaces-tradeoff.md`, and `parity:<file>` is a JSON file in this folder. Line numbers for files that `git status` shows as modified are the `HEAD` version.

**Verdicts.**

- `same`: the prototype has it and it works, or the page is not redrawn and a spec line keeps it.
- `moved`: it is somewhere else now.
- `changed`: it is there, but differs as described.
- `missing`: absent, drawn but inert, or unreachable.

**Decision column.**

- A quoted spec, trade-off or parity reason.
- "kept in words": a blanket spec sentence (「照現在的寫法」, 「照舊」, 「不動」) keeps the feature, but the rule is not written out. The real frontend must take it from today's code, not from the prototype.
- **UNDECIDED**: nothing covers it, or a spec sentence contradicts it. to:133 「原型要跟現在的 app 一樣完整」 is the requirement these rows fail, not a decision that covers them: it asks that each kept item be drawn, or listed as changed on purpose with a reason. A `same` row can be UNDECIDED: the prototype copies today's text, but another spec section moves the thing it points to.

## Needs a decision

They are ordered by area, most at risk first: Ask, where the prototype redraws the page but draws almost none of it; then Home and the full-window screens; then onboarding. Within each area, the most user-visible come first. A bullet may cover several table rows, so the bullets are fewer than the UNDECIDED rows.

### Ask

One sentence in ds §7, such as 「助理頁其他的都照現在的 app」, plus an Ask parity list, would settle most of these. These rows need their own answer even then:

- [35] / [36]: the no-AI state, and saved chats with AI off, against the new scope states. Which wins?
- [33]: the local vs online disclosure that §7's new body drops.
- [42]: the model picker, against §7's 「助理用你在「設定 › AI」裡選的模型」.
- [34]: example questions (count, source, click sends or fills).
- [22]: delete wording and confirm vs trash-with-undo.
- [56]: what an Agent 對話 citation looks like and where it links.
- [64]: the scope of chats saved before v0.6.
- [30]: an answer stops when ⌘1–9 switches space.

All UNDECIDED rows, most visible first:

- **[9] Chat list column** (`conversation-list.tsx:128-133`). The whole saved-chat column is gone, so the user cannot see or reopen past chats.
- **[40] Stop button** (`composer.tsx:104-113`). There is no way to stop an answer.
- **[50] Streaming markdown answer** (`assistant-turn.tsx:52-56`, `markdown.tsx:33-72`). There is no answer at all. Links opening in the system browser (`file://` only if visited) also go.
- **[35] No-AI-service state** (`ask-page.tsx:190-195`, `empty-states.tsx:55-77`). The user loses the explanation, 前往「設定 → AI」, and 「沒有助理，PathKeep 也完全可用」. No line says which wins over §7's scope states.
- **[36] Saved chats readable with AI off** (`ask-page.tsx:253-262`). The 要繼續提問，請先設定一個 AI 服務。 line and the hidden edit and retry are gone. A saved chat whose type is now closed or not readable is also unwritten.
- **[33] Local vs online disclosure** (`empty-states.tsx:29-33`). §7's new body drops 「{provider} 是線上服務，你提問時會把所需的片段傳送過去」, so the user loses the warning that snippets go to an online service.
- **[42] Model picker** (`model-picker.tsx:32-157`). The user loses picking a service and model per chat. §7's 「助理用你在「設定 › AI」裡選的模型」 pulls against keeping it.
- **[38] Enter / Shift+Enter / IME** (`composer.tsx:76-89`). The user cannot send by Enter, and an IME Enter could send half-composed text.
- **[39] Send button** (`composer.tsx:114-123`). It is drawn but does nothing.
- **[56] Sources chips** (`sources.tsx`, HEAD `citations.ts:12-25`). The user cannot see or open the pages an answer used. How an Agent 對話 citation looks and where it links is also unwritten.
- **[51] Thinking block with 搜尋了 N 次存檔** (`thinking-block.tsx:22-105`). The user cannot see how an answer was reached.
- **[54] Mid-stream error** (`assistant-turn.tsx:64-77`). 助理無法回答 and the reason are gone.
- **[55] Stopped note** (`assistant-turn.tsx:79-83`). 已停止，上面的回答可能不完整。 is gone.
- **[53] Seven answer notes** (`assistant-turn.tsx:58-62`). The user is not told why an answer is partial or missing (step limit, context too small, index still building, and the rest).
- **[59] 重試 / 重新回答** (`assistant-turn.tsx:108-117`). A failed answer cannot be asked again.
- **[58] 複製回答** (`assistant-turn.tsx:102-107`). Gone.
- **[57] 在歷史中開啟** (`assistant-turn.tsx:94-101`). The control is gone; only its route change is covered.
- **[60] Answer's model and token count** (`assistant-turn.tsx:118-134`). Gone.
- **[48] Edit an earlier question** (`user-turn.tsx:12-106`). Gone: the pencil, Esc to cancel, and the 這則訊息之後的回答會被新的回答取代。 warning.
- **[34] Example questions** (`empty-states.tsx:17-50`). Today there are 4, from the message file, and a click sends. The prototype has 3 per type, hard-coded, and a click fills the box. Neither the count nor the behaviour is decided.
- **[10] 新對話 button** (`conversation-list.tsx:134-142`). Gone.
- **[7] ⌘N / Ctrl+N new chat** (`ask-page.tsx:146-155`). The shortcut is gone and not kept by any v0.6 line.
- **[8] ⌘N hint** (`conversation-list.tsx:141`). It is hard-coded ⌘N on Windows and Linux. The platform rule is unwritten, and the hint is already wrong today off macOS.
- **[22] Delete with confirm, no undo** (`conversation-list.tsx:209-241`).
  - Gone.
  - The body says 「不會影響你的瀏覽歷史」, which is browsing-only.
  - Agent chats use trash with undo (agent-history §8.2), so nothing decides which way Ask chats go.
- **[19] Rename dialog** (`conversation-list.tsx:343-446`). Gone.
- **[21] Export as Markdown** (`conversation-list.tsx:93-126`). Gone: the save dialog, the path toast with 在 Finder 中顯示 (在資料夾中顯示 off macOS), and the gone and failed toasts.
- **[18] Row ⋯ menu** (`conversation-list.tsx:309-338`). Without it there is no way to reach rename, export or delete.
- **[11] Day grouping, newest first** (`conversations.ts:32-43`). Gone.
- **[12] Row title and time, active row marked** (`conversation-list.tsx:275-308`). Gone.
- **[13] Draft row for the unsaved open chat** (`conversation-list.tsx:144-153`). Gone. It always shows on a new chat, titled 新對話.
- **[14] List loading skeleton** (`conversation-list.tsx:155-161`). Gone.
- **[15] List error 無法載入你的對話。** (`conversation-list.tsx:163-167`). Gone.
- **[16] List empty 你的對話會顯示在這裡。** (`conversation-list.tsx:169-173`). Gone. Low stakes: it cannot be reached today (see the row note).
- **[17] Every chat reachable through paging by 100** (`conversations.ts:9-25`). Gone. `intelligence.md:121` requires following the cursors.
- **[23] Opening skeleton 正在開啟對話…** (`ask-page.tsx:188-189`). Gone.
- **[24] Open failure toast 無法開啟這段對話。** (`ask-page.tsx:124-125`). Gone.
- **[6] `?q=` from the Insights 問問助理 button** (`ask-page.tsx:133-144`). The question would no longer wait in a new chat. The browsing Insights page that sends it stays (§4.5).
- **[44] Context meter** (`context-meter.tsx:21-63`). The user cannot see how full the model's context is.
- **[45] Context near/over warning** (`context-meter.tsx:66-92`). The user is not told that a message will not be sent.
- **[47] Question bubble** (`user-turn.tsx:88-105`). Gone.
- **[49] Waiting indicator 處理中…** (`assistant-turn.tsx:45-50`). Gone.
- **[64] Scope of a saved chat** (`use-chat.ts:62-70`). Today's format has no scope. Unwritten:
  - the scope of chats saved before v0.6;
  - whether the list shows or filters by scope;
  - whether scope can change after the first question;
  - what a chat shows when its type is closed or no longer readable.
- **[30] Leaving Ask while answering stops and saves the answer** (`use-chat.ts:295-303`). The rule is unwritten. The prototype's ⌘1–9 also navigates away, so it would stop an answer too.
- **[29] Opening another chat, a new chat or `?new=1` while answering stops the answer** (`use-chat.ts:283-293`). The rule is unwritten.
- **[27] A reopened chat shows exactly what it showed before** (`chat-model.ts:130-160`). The rule is unwritten.
- **[28] An answer saved mid-stream reopens as stopped, with no spinning steps** (`chat-model.ts:139-140,154-158`). The rule is unwritten.
- **[26] Opening a chat restores its last model** (`ask-page.tsx:116-122`). The rule is unwritten.
- **[25] A stale chat load is dropped** (`ask-page.tsx:108-112`). The rule is unwritten.
- **[20] A renamed title survives the next save** (`use-chat.ts:30-31`). The rule is unwritten.
- **[31] Saved after every answer; a failed save is silent** (`use-chat.ts:62-73`). The rule is unwritten.
- **[61] Stick to bottom while streaming** (`use-stick-to-bottom.ts`). Gone.
- **[41] The box takes focus when a chat opens** (`composer.tsx:56`). Gone.
- **[43] Fallback to the Settings model when the pick is no longer offered** (`ask-page.tsx:58-68`). The rule is unwritten.
- **[46] Context re-read after each answer** (`ask-page.tsx:92-96`). The rule is unwritten.
- **[52] Text before a tool call moves into thinking** (`use-chat.ts:125-131`). The rule is unwritten.
- **[62] Title-bar strip and drag region in the Ask columns** (`ask-page.tsx:186`, `conversation-list.tsx:130`). Not drawn.
- **[65] Screen skeleton while Ask's code loads, and a per-screen error that keeps the rail** (HEAD `router.tsx:8-29,39-42`). The prototype router has neither. This applies to every screen, so the shell area may own it.

### Home and full-window screens

- **Where Home's alerts show once Home is not the start screen.** `src/features/home/alerts-list.tsx:13-18`, `alerts.ts:33-118` (HEAD). Today the app opens on Home (`router.tsx:44-46` HEAD), so a failed automatic backup, a missing keychain or a password only the user knows is the first thing seen. In v0.6 `#/` opens the last space's first page (TO §4.2:50), the Agent space has no Home (TO §4.1:39), and browsing can be turned off (DS §2:47; the prototype then shows "「瀏覽紀錄」沒有開啟", `P/src/shell/frame.tsx:62`). A user who works in the Agent space, or keeps only Agent on, never sees these warnings. `archive.md:152,307` says they must not stay hidden.
- **What 立即備份 on the browsing Home backs up.** `src/components/app/backup-button.tsx:71-88` (HEAD), `src/app/backup-runner.tsx:135-161`. Today it is the one manual backup, shared with Backup, ⌘K and onboarding, disabled while it runs. The prototype makes it browsing-only with the toast 「開始備份「瀏覽紀錄」」/「只備份這個類型…」 (`P/src/pages/browsing-home.tsx:2-3,31`), copied from DS §4:238, which is about the backup pages. DS §6:348 makes ⌘K's 立即備份 back up every kind that is on, and the tray does the same (DS §4:275). Nothing says which one Home's button does, or whether it is disabled while an Agent backup runs.
- **Where the automatic-backup alert sends the user.** `src/features/home/alerts.ts:41,57` (HEAD) links to `/backup`, because the 自動備份 card is there today (`archive.md:152` 「Home 的連到 Backup」). In v0.6 the card moves to 設定 › 排程與背景工作 (TO §5:121, DS §8:386), and the browsing backup page keeps only a summary row (DS §4:256). The E2E is already being rewritten to `/browsing/backup` (working-tree diff of `home-alerts.spec.ts:119`), which is one click away from the fix.
- **"已暫停" for a browser that is not backed up.** `src/features/home/home-page.tsx:166-169` (HEAD; stat card 「{n} 個已暫停」) and `home-cards.tsx:443-447` (HEAD; dimmed row titled 「已暫停」). In v0.6 「暫停」 is a state of a whole data type (DS §2:46), and DS §4:250 says 「來源沒有另外的「暫停」…狀態寫 `不備份`」. On the browsing Home, a deselected profile would be called 已暫停, but on the backup page next to it, 不備份.
- **What 「存檔大小」 on the browsing Home measures.** `src/features/home/home-page.tsx:110-116,151-161` (HEAD) adds up every part of the archive's storage. Once the archive also holds Agent history, the card sits in the browsing space but would count the Agent databases too. DS §8:390 splits storage by type and DS §4:258 gives each backup page `存檔裡的{名稱}約 {大小}`. Nothing says what this card shows.
- **Which backup the Home status pill reports.** `src/components/app/backup-button.tsx:25-38` (HEAD) reads the archive's last successful backup and the latest run of any kind. Once each kind backs up as its own task (DS §5:276), a failed Agent backup would turn the browsing Home's dot red, and an Agent-only run would count as 「{ago}已備份」. DS §4:237 defines `上次備份` only for the per-type backup pages.
- **The finished-backup toast counts only visits.** `src/app/backup-runner.tsx:67-97,121-133` shows 「備份完成，新增 {n} 條記錄」 from `newVisits`, on every screen, including after a tray backup. In v0.6 the tray (DS §4:275) and the schedule back up every kind, so a run that saved 40 Agent chats and no visits would read 「沒有新增造訪」. Nothing in the spec says what that toast says.
- **"Closing the window keeps it going" against quit-on-close.** DS §3.2:135 「關掉這個視窗也會繼續存」 and DS §5.2:306 「{n} 項在跑，關掉視窗也會繼續。」. Today, with the menu bar icon off, closing the window quits PathKeep and the quit question appears (`src/app/shell/quit-prompt.tsx:2-5`, SN:127). The sentence is true only with the menu bar icon on.
- **The quit question doesn't know the new work.** `src/app/shell/quit-prompt.tsx:38-51` lists today's kinds of work and what quitting does to each. v0.6 adds deleting one type's data (DS §2:62-74, destructive like `data-wipe`), the first save when Agent is turned on (DS §3.2), per-type backups, and reparse, verify and rebuild (architecture §9). Nothing says which of these hold the quit and what the question says for each. SN:133 says background queue work doesn't ask, and DS §5 calls the Agent backup a 背景任務.
- **The quit question itself.** `src/app/shell/quit-prompt.tsx:53-186`: no spec line keeps it and the prototype doesn't draw it. Low risk, because it lives outside the shell, but nothing on record keeps it.
- **Recovery when only one type's database is damaged.** `src/app/shell/status-screens.tsx:305-394`, `session.tsx:219-229`. DS §9:414 keeps 「一個復原流程」, while `agent-history-product-design.md` §5 says 「damage to one kind does not take the other down」. The recovery screen says 「存檔打不開…存檔檔案看起來已損壞」 and takes the whole window. Nothing says what a user sees when only the Agent databases are damaged.
- **The schema-update screen and its text.** `src/app/shell/status-screens.tsx:55-90`, `src/app/index.tsx:51-58`: no spec line and not drawn. Its body names one particular change (「這個版本用不同的方式保存搜尋索引」, shell.ts zh-TW `upgrade.schema.body`). If the v0.6 archive change runs as an open-time migration (architecture "Why now": registered migrations), the screen would give the wrong reason.
- **The one-time archive conversion screen.** `src/app/shell/status-screens.tsx:121-303`: no spec line and not drawn. This includes the password check, the 「有些檔案用的是以前的密碼」 decision, progress phases, the wrong-password message and the hand-off to recovery.
- **The boot screen.** `src/app/shell/status-screens.tsx:42-52` (「正在打開存檔…」): no spec line and not drawn.
- **The boot error screen.** `src/app/shell/status-screens.tsx:92-114` (「PathKeep 無法啟動」, 再試一次, 查看日誌): no spec line and not drawn. A user would lose the way to retry or find the logs when startup fails.
- **Returning to unlock when the archive closes under the window.** `src/app/session.tsx:264-281`: no spec line. Without it, a closed archive leaves every screen empty instead of asking for the password or passcode again.
- **The screen-error view.** `src/app/shell/route-error.tsx:26-72` (「這個畫面出錯了」, 重新載入; the nav stays when only one screen failed): no spec line and not drawn.
- **Units on Home against DS §1.** `src/lib/i18n/messages/common.ts` zh-TW `visits` 「{n} 次瀏覽」, used by the year summary, cells, on-this-day and threads (`home-cards.tsx:250-253,179,398` HEAD). DS §1:33 says a browsing item is called 造訪 (`{n} 筆造訪`), and the prototype stand-in mixes 造訪 and 頁. It's unclear whether §1 renames Home's units or only names the switcher's status line.

### Onboarding

- **Storage tiles against the new per-type total.** `src/features/onboarding/storage-step.tsx:37-50,96-98`. ds:202 keeps the three browser-only tiles and the note 「依你選的瀏覽器估算」, and adds a per-type list with its own `合計`. The user then sees two "estimated size" figures that disagree, and 0 B / 0 browsers when only Agent 對話 is chosen.
- **The first-backup result count.** `src/features/onboarding/done-step.tsx:220-222` counts new visits (the E2E checks it equals the archived visits exactly). The prototype adds visits and chats into one 「共保存 {n} 條記錄」 (`proto/pages/onboarding/use-finish.ts:106`), so the number stops meaning anything a user can check.
- **Result lines when one type's first backup fails.** `done-step.tsx:211-212,229-233`. The real rule (some browsers saved and others not: setup finishes and says so) is browser-only, and its text sends the user to 「備份」, which with two backup pages names neither. Nothing says what shows when an Agent tool fails during onboarding's first backup.
- **Skip label for a failed schedule install: `跳過，之後在「備份」裡設定`.** `messages/onboarding.ts` `done.skip.schedule`. ds:210 「略過都照舊」 keeps it, but the schedule moves to 設定 › 排程與背景工作 (to:121, ds:386), so it sends the user to the wrong place.
- **`PathKeep 無法在這裡安裝排程` / `設定完成後，「備份」頁面會列出手動設定的步驟。`** `schedule-step.tsx:105-113`. This has the same conflict: the manual steps live with the 自動備份 card, which moves to Settings.
- **Which blocked profiles bring up the Full Disk Access notice.** `browsers-step.tsx:57-59`; `platform-guidance.ts:110-117`. Today it is only a blocked Safari or a macOS FDA scan result. ds:191 and the prototype show it for any blocked profile while ds:187 says 「規則不變」, so a Windows or Linux user with one unreadable profile would be told to grant a macOS permission for Safari.
- **The 連網 summary row when 瀏覽紀錄 isn't chosen.** `done-step.tsx:130-136`. An Agent-only user is warned about link previews that will never run, and nothing says whether the row hides, as its Settings counterpart does (ds:410).
- **Resuming the backup interval.** `use-schedule-preview.ts:24`; `draft.ts:76`. Today the interval a user picked comes back after closing an unfinished setup. The prototype always starts at 每小時, and only the source choice is written down as resumed.
- **Where a reopened setup starts.** `onboarding-page.tsx:58` always starts at 歡迎. ds:199 「下次從這裡接著」 can be read as reopening at 來源. Pick one.
- **`繼續` disabled while the source choice saves.** `onboarding-page.tsx:219`. Without it, a double press can move on while the save is still failing.
- **Window dragging, and the Windows / Linux frame.** `onboarding-page.tsx:324-328,386`. The macOS drag strips and the non-macOS spacer aren't drawn or written. Low risk, but a rebuilt page could lose the way to move the window.
- **The language menu inside setup.** `onboarding-page.tsx:397-425`. No spec line keeps it, and to:44 「只在設定裡」 can be read as removing it. A first-run user who opens in the wrong language would then have no way to change it before the end.
- **Route error screen for the onboarding page.** HEAD `src/app/router.tsx:100`. Low risk: a crash in setup today shows a reload screen.

---

## A. Onboarding

Real: `src/features/onboarding/*` (all unmodified), with `src/app/index.tsx`, `src/app/session.tsx` and `git show HEAD:src/app/router.tsx` for when the page shows. Prototype: `proto/pages/onboarding.tsx` and `proto/pages/onboarding/*`. Spec: `ds` §3.3 (lines 156–210).

### A.1 Page frame, moving between steps, saving and resuming

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
| --- | --- | --- | --- | --- | --- | --- |
| Step list on the left: number in a circle, label, check mark once done | control | onboarding-page.tsx:310-385 | same | proto onboarding.tsx:136-168 | ds:158 "現在的每一步都留著" | 8 steps instead of 7, as ds:158 says |
| Only steps already passed can be clicked; later steps are disabled | rule | onboarding-page.tsx:338-346 | same | proto onboarding.tsx:139-147 | — | |
| Current step `aria-current="step"`; finished steps add a screen-reader "(已完成)" | text | onboarding-page.tsx:345, 374-379 | same | proto onboarding.tsx:146, 162 | — | |
| Thin progress bar on top, `role=progressbar` with step n of N | state | onboarding-page.tsx:123-135 | same | proto onboarding.tsx:197-199 | — | |
| "NN / NN" counter, step title, step body above each step | text | onboarding-page.tsx:137-150 | same | proto onboarding/steps.tsx:26-41 | — | |
| On every step change: scroll to top, focus moves to the new heading (not on first load), step rises in, no animation with reduced motion | keyboard | onboarding-page.tsx:276-308 | same | proto onboarding.tsx:274-293 | — | the prototype widens the column to 960 px on 來源 only (ds:178) |
| `上一步`: hidden on the first step; disabled once finishing has started | control | onboarding-page.tsx:195-203 | same | proto onboarding.tsx:231-234 | — | |
| Everything that moves between steps is locked from the moment `開始第一次備份` is pressed, including after a stage fails (no going back to change a choice after a failure) | rule | onboarding-page.tsx:79, 198, 344 | same | proto onboarding.tsx:79, 145, 231 | — | |
| `跳過` only on 自動備份 and AI; sets 手動 / 先不用 and moves on | control | onboarding-page.tsx:109-112, 205-213 | same | proto onboarding.tsx:103-106, 236-240 | ds:158 | |
| Main button wording: `開始設定` on step 1, `繼續`, then on 完成 `開始第一次備份` → spinner + `正在設定…` → `進入 PathKeep` | control | onboarding-page.tsx:239-269 | same | proto onboarding.tsx:241-259 | ds:158 | |
| Main button disabled while the step is not ready, or while the run is going | rule | onboarding-page.tsx:217-221 | same | proto onboarding.tsx:242 | — | |
| Main button disabled while the source choice is being saved (`save.isPending`), so a second press cannot save twice or skip ahead of a failure | rule | onboarding-page.tsx:219 | missing | proto onboarding.tsx:242 (no pending state; nothing is saved) | **UNDECIDED** | ds:199 keeps the save and its failure, not this guard |
| Main button hidden after a stage fails; only that stage's `再試一次` / skip remain | rule | onboarding-page.tsx:214 | same | proto onboarding.tsx:241 | — | |
| Pressing `繼續` on the source step saves the choice to the config; on failure `無法儲存你的選擇` + reason, stays on the step | rule | onboarding-page.tsx:92-102, 187-192 | missing | proto onboarding.tsx:97-102 (saves nothing, no error) | ds:199 "按 `繼續` 時把選的類型和來源存下來…存不了寫 `無法儲存你的選擇` 和原因，留在這一步"; parity:kept-not-drawn.onboarding-ai.json `onboarding.saveFailed` | |
| Resume: an unfinished setup starts with the profiles it saved (unknown ids dropped); otherwise every readable profile | rule | draft.ts:61-71 | missing | proto onboarding/sources-data.tsx:172-174 (readable only, no saved choice) | ds:190 "上次沒做完的設定存過選擇的話，從那次的選擇開始" | not modelled, but kept in words |
| Resume: the backup interval comes back too. The schedule preview saves `dueAfterHours` when the user reaches 自動備份 and the first time each interval is picked (the query is cached per interval, so picking one again does not save), and `initialDraft` reads it back | rule | use-schedule-preview.ts:18-29; draft.ts:76 | missing | proto onboarding/draft.ts:28 (always `hourly`) | **UNDECIDED** | `screens-and-nav.md` §Onboarding says only the browser choice is written; the code also writes the interval |
| Resume position: a reopened setup always starts at 歡迎, with the saved choice applied | rule | onboarding-page.tsx:58 | changed | ds:199 "沒做完就關掉的話下次從這裡接著" | **UNDECIDED** | "從這裡接著" can be read as reopening at 來源, which today's app does not do. Say which |
| Once the archive is created, setup is over: closing the app during the later stages lands in the app next time, and the schedule, backup or AI stage that was not reached is not run | rule | session.tsx:131; use-finish-setup.ts:90-110 | missing | not modelled | kept in words: ds:210 "按下開始後照現在一步一步做" | v0.6 makes the first backup a background task per type (ds:210), so check that this still holds |
| The archive is created from the config already saved, merged with the choices: settings saved before setup are kept (E2E: background work stays paused), and the language in use is written as `preferredLanguage` | rule | use-finish-setup.ts:91-105; tests/e2e/first-run.spec.ts:28-31, 113-115 | missing | not modelled | kept in words: ds:210 | |
| `進入 PathKeep` opens the app; a language set in the config wins over the local one | rule | onboarding-page.tsx:104-105; session.tsx:116-131 | same | proto onboarding.tsx:92-96 | to:50 "`#/` 導到上次所在空間的第一頁" | |
| Language menu: 跟隨系統 plus the three languages; changes the whole window at once | control | onboarding-page.tsx:397-425 | missing | proto onboarding/appearance.tsx:19-35 (drawn; it only remembers the choice, its comment says it only speaks zh-TW) | **UNDECIDED** | No spec line keeps a language control in setup, and to:44 "語言切換不在導航列，只在設定裡" can be read as removing it. During setup there is no Settings to go to |
| Light / dark toggle; aria `切換到淺色` / `切換到深色` | control | onboarding-page.tsx:426-438 | same | proto onboarding/appearance.tsx:36-44 | — | |
| PathKeep mark and name above the step list | text | onboarding-page.tsx:329-334 | same | proto onboarding.tsx:132-135 | — | |
| Window dragging: macOS drags by the 52 px strip at the top-left and the empty space above the language / theme buttons (`data-tauri-drag-region`) | platform | onboarding-page.tsx:324-328, 386 | missing | proto onboarding.tsx:131, 169 (no drag regions) | **UNDECIDED** | low: the prototype can't drag, but nothing records that the regions stay |
| Windows / Linux: no overlay title bar, so a short 28 px spacer instead of the 52 px traffic-light clearance | platform | onboarding-page.tsx:324-328 | missing | proto onboarding.tsx:125-131 (always draws macOS traffic lights and the 52 px spacer) | **UNDECIDED** | low |
| `在這台 Mac 上` / `在這台電腦上` by platform (used in the AI step) | platform | onboarding-page.tsx:62-64 | same | proto onboarding.tsx:75 | — | |
| No ⌘K, ⌘L, back/forward shortcuts and no way out of setup (the page is outside the app frame) | keyboard | src/app/index.tsx:72-75; HEAD router.tsx:95-103 | same | proto main.tsx:28 (route outside `Frame`) | — | The prototype adds a `原型：離開新手引導` button; it is prototype-only |
| Any address shows setup (`path: '*'`); the address never selects a step | url | HEAD router.tsx:95-103 | same | — | — | The prototype adds `?step=` and `?many=1` for review; see Added |
| Setup shows only when the config is not initialized. An older archive gets the update screen instead (E2E upgrade). Deleting all data restarts into setup | rule | session.tsx:131; tests/e2e/upgrade.spec.ts:5-8 | same | not drawn | ds:84 "從 v0.5 升級上來時，Agent 對話是關閉的" (upgrade users are not sent through setup) | session logic, not redrawn |
| A crash or failed chunk load of the page shows the route error screen | state | HEAD router.tsx:100 | missing | not drawn | **UNDECIDED** | low: infrastructure the prototype can't show |

### A.2 歡迎

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
| --- | --- | --- | --- | --- | --- | --- |
| Title `你的瀏覽器正在刪掉你的歷史` | text | onboarding-page.tsx:146 (`onboarding.steps.welcome.title`) | changed | proto onboarding/steps.tsx:63 | parity:changed-on-purpose.onboarding-ai.json "§3.3 歡迎: the title becomes 「留住會被自動刪掉的東西」" | |
| Body, with `{where}` (在這台 Mac 上 / 在這台電腦上) | text | onboarding-page.tsx:149 | changed | proto onboarding/steps.tsx:63 (no platform phrase) | parity:changed-on-purpose.onboarding-ai.json `onboarding.steps.welcome.body`; ds:162 | The platform phrase is gone with the old body |
| Figure title `瀏覽器預設保留多久` | text | welcome-step.tsx:29-31 | changed | proto onboarding/steps.tsx:65 | parity:changed-on-purpose.onboarding-ai.json `retentionTitle` | |
| Rows Chrome / Arc `約 90 天`, Safari `1 年`, Firefox `看容量`, with browser icons and bars | text | welcome-step.tsx:10-15, 33-51 | same | proto onboarding/steps.tsx:43-50 | ds:163 "現在的四列照舊" | Row order changed: see Added |
| PathKeep `永久` row in the brand colour | text | welcome-step.tsx:52-59 | same | proto onboarding/steps.tsx:77-82 | ds:163 | |
| Three promises: 只在本機 / 不碰原檔 / 可以搜尋 | text | welcome-step.tsx:17-22, 62-77 | same | proto onboarding/steps.tsx:52-56, 85-93 | ds:164 | |

### A.3 瀏覽器 → 要保存什麼 + 來源

The real step is replaced by two (ds:158). The rows below are the real step's features; the new 要保存什麼 step has no real counterpart.

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
| --- | --- | --- | --- | --- | --- | --- |
| Step label `選擇瀏覽器`, title `要備份哪些瀏覽器？`, body `我們{where}找到了以下瀏覽器。之後可以在「備份」裡隨時調整。` | text | messages/onboarding.ts (zh-TW `steps.browsers`) | changed | proto onboarding/sources-step.tsx:212; onboarding/steps.tsx:101 | parity:changed-on-purpose.onboarding-ai.json `onboarding.steps.browsers`; ds:169, ds:177 | "之後可以在「備份」裡隨時調整" is gone; 要保存什麼 says choices can change in 設定 instead |
| Only profiles with a history file are listed, plus Safari when macOS hides it; browsers that are not installed are left out | rule | draft.ts:49-55 | same | ds:188 | ds:188 "規則不變" | The prototype uses fixed data, so the rule exists only in the spec |
| Row: checkbox, browser icon, browser name, profile name, history-file size with hover `歷史檔案：{size}`; the whole row is the click target | control | browsers-step.tsx:150-200 | same | proto onboarding/sources-step.tsx:84-109 | ds:189 | |
| A blocked profile shows `· 需要權限`, no size, starts unticked, and can still be ticked | state | browsers-step.tsx:162, 184, 187; draft.ts:66-71 | same | proto onboarding/sources-step.tsx:95, 98; onboarding/sources-data.tsx:172-174 | ds:189 | |
| Full Disk Access notice (`Safari 需要「完整磁碟取用權限」`, `打開設定`, `重新檢查`). It shows only when discovery reports macOS FDA or a **Safari** profile is blocked | rule | browsers-step.tsx:57-59; platform-guidance.ts:110-117 | changed | proto onboarding/sources-step.tsx:171 (any blocked source in the open type); ds:191 "有設定檔被系統擋住…時" | **UNDECIDED** | ds:187 says "規則不變", while ds:191 and the prototype widen the rule. A blocked Chrome or Firefox profile on Windows or Linux would then get a macOS Safari notice |
| `打開設定` opens the macOS FDA pane; on failure `無法打開系統設定` + reason | control | browsers-step.tsx:68-75, 120-125 | same | proto onboarding/sources-step.tsx:187-190, 302 | ds:191 | |
| `重新檢查` reloads the whole snapshot; while checking it is disabled and pulses; on failure `重新檢查失敗` + reason | control | browsers-step.tsx:31-37, 77-87, 126-131 | same | proto onboarding/sources-step.tsx:192-208, 303 | ds:191 | Side finding: a failed 重新檢查 from the empty or scan-failed notice shows nothing in either version (the error renders only inside the FDA notice) |
| A profile that becomes readable after 重新檢查 is not ticked automatically | rule | browsers-step.tsx:31-37 (draft untouched) | same | proto onboarding/sources-step.tsx:191, 200 | ds:192 | |
| When the system hides everything, only the FDA notice shows (no empty or error notice) | state | browsers-step.tsx:102-110 | same | proto onboarding/sources-step.tsx:263-265 | ds:193 | |
| `找不到瀏覽器` and `PathKeep 無法掃描瀏覽器` notices, each with `重新檢查` (outline button) | state | browsers-step.tsx:103-109, 202-229 | same | proto onboarding/sources-step.tsx:267-281 | ds:193 | |
| Hint `至少選一個 PathKeep 能讀取的瀏覽器。` shown only while the step is not ready **and** something is listed | text | browsers-step.tsx:141-145 | changed | proto onboarding/sources-step.tsx:354-358 (per type; also shown when nothing was found) | ds:198; parity:changed-on-purpose.onboarding-sources.json `needOne` | |
| Continue needs at least one ticked profile PathKeep can read; a ticked blocked profile does not count | rule | draft.ts:91-102 | same | proto onboarding/sources-step.tsx:368-370 | ds:198 | applied to every chosen type |
| The list grows with the page; the page scrolls | state | onboarding-page.tsx:299 | changed | proto onboarding/sources-step.tsx:213, 283 (fixed-height card, the list scrolls inside) | ds:178 | layout still awaiting the user (ds:16) |

### A.4 存放位置

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
| --- | --- | --- | --- | --- | --- | --- |
| `存檔位置`: the real app data folder (per platform), truncated with the full path on hover | text | storage-step.tsx:25, 55-64 | same | proto onboarding/storage-step.tsx:14, 52-56 | kept in words: ds:202 "照現在的寫法" | The prototype shows a macOS path even when its Windows switch is on |
| Button `在 Finder 中顯示` (macOS) / `打開資料夾` (others) | platform | storage-step.tsx:65-76 | same | proto onboarding/storage-step.tsx:57-60 | ds:202 | |
| The button opens the folder | control | storage-step.tsx:28-35 | missing | proto onboarding/storage-step.tsx:57 (no handler) | README "原型裡很多操作只是示意（…在 Finder 顯示…）" | |
| Opening fails: `無法打開資料夾` + reason | state | storage-step.tsx:78-83 | missing | not drawn | parity:kept-not-drawn.onboarding-ai.json `onboarding.storage.revealFailed` | |
| Three tiles `瀏覽器歷史` / `預計大小` / `瀏覽器`, and the note `依你選的瀏覽器估算。之後每次備份，存檔會再大一點。` | text | storage-step.tsx:37-50, 85-98 | changed | proto onboarding/storage-step.tsx:35-39, 63-71, 72-89 | **UNDECIDED** | ds:202 keeps the tiles and adds a per-type list with its own `合計`. `預計大小` is then a second "total" that covers browsing only: it disagrees with `合計`, and it reads 0 B / 0 when only Agent 對話 is chosen |
| The estimate counts each profile's history, favicons and supporting files, only for ticked readable profiles; archive ≈ 4/5 of that, plus manifest and snapshots | rule | src/lib/onboarding-estimates.ts:42-95 | changed | proto onboarding/storage-step.tsx:17-27 (history-file size only) | kept in words: ds:202 | prototype shortcut; reuse `estimateOnboardingStorage` |

### A.5 加密

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
| --- | --- | --- | --- | --- | --- | --- |
| Two cards 加密 / 不加密 as one radio group (arrow keys); 加密 is the default | control | encryption-step.tsx:48-68; draft.ts:72 | same | proto onboarding/encryption-step.tsx:35-38; onboarding/draft.ts:24 | ds:204 | ds:204 also adds a sentence under the title and one to 不加密, both drawn (proto :31, :37) |
| Password fields shown only when encrypting | state | encryption-step.tsx:70 | same | proto onboarding/encryption-step.tsx:40 | — | |
| Password (new-password autofill, placeholder `至少 10 個字元`) | control | encryption-step.tsx:73-84 | same | proto onboarding/encryption-step.tsx:43-52 | — | |
| Strength meter (4 bars, colours, `role=meter`, `密碼強度：{level}`) and label | state | encryption-step.tsx:86-108; draft.ts:83-89 | same | proto onboarding/encryption-step.tsx:54-59; onboarding/draft.ts:35-41 | — | |
| Confirm field; `兩次輸入的密碼不一樣。` with `aria-invalid` | state | encryption-step.tsx:109-130 | same | proto onboarding/encryption-step.tsx:60-76 | — | |
| Continue needs strength ≥ 2 and matching fields, or 不加密 | rule | draft.ts:103-107 | same | proto onboarding/draft.ts:44 | — | |
| Keychain switch: `存到 macOS 鑰匙圈` / `存到系統鑰匙圈`; on by default when a keychain is available | platform | encryption-step.tsx:132-161; draft.ts:75 | same | proto onboarding/encryption-step.tsx:78-86; onboarding/draft.ts:27 | — | |
| No keychain: different explanation, switch off and disabled, row dimmed | state | encryption-step.tsx:136, 147-159 | same | proto onboarding/encryption-step.tsx:78-85 | — | |
| Typed password kept when switching to 不加密 and back | rule | onboarding-page.tsx:82-83 | same | proto onboarding.tsx:74 | — | |

### A.6 自動備份

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
| --- | --- | --- | --- | --- | --- | --- |
| 每小時 / 每 6 小時 / 每天一次 / 手動 as a radio group; `推薦` on 每小時 | control | schedule-step.tsx:27, 49-70 | same | proto onboarding/schedule-step.tsx:23-30, 69-80 | ds:206 "照現在的寫法和選項" | ds:206's added sentence is drawn (proto :65) |
| Warning when encrypted without keychain and not 手動: background backups can't open the archive | state | schedule-step.tsx:72-77; draft.ts:116-118 | same | proto onboarding/schedule-step.tsx:82-87 | — | |
| Preview loading skeleton (`正在準備排程…`) | state | schedule-step.tsx:80-88 | same | proto onboarding/schedule-step.tsx:90-94 | — | |
| Preview fails: `無法準備排程。` + reason + `再試一次` | state | schedule-step.tsx:89-102 | same | proto onboarding/schedule-step.tsx:95-110 | — | |
| A failed preview does not block `繼續`; the finishing run then tries the install anyway | rule | draft.ts:99-112; onboarding-page.tsx:74 | same | proto onboarding.tsx:77 | — | |
| Plan preview: `將要安裝的內容`, `完成設定之前不會安裝任何東西。`, scheduler name · label | text | schedule-step.tsx:119-147 | same | proto onboarding/schedule-step.tsx:123-136 | — | |
| Scheduler name on Linux is `systemd` (macOS `launchd`, Windows `Task Scheduler`, otherwise the raw platform) | platform | schedule-step.tsx:29-33, 146 | missing | proto onboarding/schedule-step.tsx:135 (macOS / Windows only) | kept in words: ds:206 | |
| The `檔案` row shows only when a path is known; `查看檔案` only when a generated file exists | state | schedule-step.tsx:122-125, 148, 157 | changed | proto onboarding/schedule-step.tsx:137-148 (always both) | kept in words: ds:206 | |
| `查看檔案` / `收起檔案` expands the exact file contents | control | schedule-step.tsx:157-182 | same | proto onboarding/schedule-step.tsx:140-148 | — | |
| Plans cached per interval, so switching back is instant | rule | use-schedule-preview.ts:21-28 | same | proto onboarding/schedule-step.tsx:47-55 | — | |
| Cannot install here: `PathKeep 無法在這裡安裝排程` / `設定完成後，「備份」頁面會列出手動設定的步驟。`; the schedule stage is then dropped from the run | state | schedule-step.tsx:105-113; onboarding-page.tsx:74; use-finish-setup.ts:69 | same | proto onboarding/schedule-step.tsx:113-117; onboarding.tsx:77 | **UNDECIDED** | The text survives but now points to the wrong place: in v0.6 the 自動備份 card and its manual steps move to 設定 › 排程與背景工作 (ds:386, to:121), while ds:206 says "照現在的寫法" |

### A.7 AI

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
| --- | --- | --- | --- | --- | --- | --- |
| 先不用 (default) / 本機語意搜尋 / 連接 AI 服務 as a radio group | control | ai-step.tsx:30-61; draft.ts:77 | same | proto onboarding/ai-step.tsx:26-40 | ds:208 "照現在的寫法" | |
| Local option shows the model's approximate size | text | ai-step.tsx:49-57 | same | proto onboarding/ai-step.tsx:36 | — | |
| Local option body uses `{where}` | platform | ai-step.tsx:48 | same | proto onboarding/ai-step.tsx:20 | — | |
| 連接 AI 服務 opens the Settings form inline: service, address, model, name, API key with its hint | control | ai-step.tsx:63-69; ai-setup/provider-form.tsx:26-144 | same | proto onboarding/ai-step.tsx:42-44; proto settings/ai/provider-form.tsx:13-82 | — | The model field's list states are shared with Settings › AI (`listCase`); audited with Settings, not here |
| Under the form, the local or online note depending on the address | text | ai-step.tsx:70-74 | same | proto onboarding/ai-step.tsx:45 | — | |
| Continue needs a complete service (model, and a key when the online service needs one) | rule | draft.ts:108-109; providers.ts:250-260 | same | proto onboarding/draft.ts:45; proto settings/ai/providers.ts:117-121 | — | |
| Note `不用 AI，PathKeep 照樣完整可用。之後可以在「設定 → AI」更改。` | text | ai-step.tsx:78-80 | same | proto onboarding/ai-step.tsx:49 | — | The step does not say that Agent 對話 stays closed to AI by default (agent-history.md §7); ds:208 keeps the wording as is |

### A.8 完成, the run, retry and skip

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
| --- | --- | --- | --- | --- | --- | --- |
| Title `都設定好了` / body | text | onboarding-page.tsx:146-150 | same | proto onboarding/done-step.tsx:57 | ds:210 | |
| Summary rows 瀏覽器, 存放位置, 加密, 自動備份, AI, 連網 | text | done-step.tsx:101-137 | changed | proto onboarding.tsx:109-121 | ds:210 adds `保存` first and `Agent 工具` after `瀏覽器` | |
| The 瀏覽器 row names every ticked profile (`Chrome (Personal), …`), including ticked blocked ones | text | done-step.tsx:93-99 | changed | proto onboarding.tsx:111-115 (count of readable ones) | ds:200 "寫選了幾個讀得到的來源"; ds:210 | Users no longer see which profiles they chose, by name, before starting |
| 加密 row: `已開啟 · AES-256 · 密碼在鑰匙圈中` / `不加密` | text | done-step.tsx:108-119 | same | proto onboarding.tsx:117 | — | |
| 連網 row: link previews download from each site; where to turn it off | text | done-step.tsx:130-136 | changed | proto onboarding.tsx:120 | parity:changed-on-purpose.onboarding-ai.json `onboarding.done.onlineValue` | |
| 連網 row shown whatever was chosen | rule | done-step.tsx:130-136 | same | proto onboarding.tsx:120 | **UNDECIDED** | With only Agent 對話 chosen, the row warns about link previews that will never run. ds:410 hides the matching Settings row when 瀏覽紀錄 is off; nothing says what this row does |
| Long summary values truncate, full text on hover | text | done-step.tsx:152 | same | proto onboarding/done-step.tsx:63 | — | |
| Which stages run: 建立存檔 always; 把密碼存到鑰匙圈 only if encrypting with an available keychain; 安裝排程 only if not 手動 and installable here; 第一次備份 always; 設定 AI only if not 先不用 | rule | use-finish-setup.ts:61-75 | same | proto onboarding/use-finish.ts:41-48 | ds:210 | |
| Stage list appears on start; each row pending / spinner / green check / red × / dash + strikethrough when skipped; screen-reader state | state | done-step.tsx:52-61, 161-205 | same | proto onboarding/done-step.tsx:69-75, 110-143 | ds:210 | |
| A failed stage: inline error with that stage's title and the reason; `再試一次` resumes from that stage | state | done-step.tsx:65-84; use-finish-setup.ts:145-187 | same | proto onboarding/done-step.tsx:77-94; onboarding/use-finish.ts:109-138 | ds:210 "每一步的狀態、重試、略過都照舊" | |
| Every stage except 建立存檔 can be skipped, with its own wording | control | use-finish-setup.ts:35-40; done-step.tsx:74-80 | same | proto onboarding/done-step.tsx:30-35, 86-90 | ds:210 | |
| Skip label for 安裝排程: `跳過，之後在「備份」裡設定` | text | messages/onboarding.ts (zh-TW `done.skip.schedule`) | same | proto onboarding/done-step.tsx:32 | **UNDECIDED** | Points to 備份, but v0.6 puts the schedule in 設定 › 排程與背景工作 (to:121; ds:386). ds:210 "略過都照舊" keeps the stale wording |
| Skipping 把密碼存到鑰匙圈 also saves "don't remember the key" to the config; if that save fails, the error is replaced in place and nothing moves on | rule | use-finish-setup.ts:193-206 | missing | proto onboarding/use-finish.ts:141-146 (skip saves nothing, cannot fail) | kept in words: ds:210 "略過都照舊" | |
| Archive created but not opened: `存檔已建立，但沒能打開。` | state | use-finish-setup.ts:107-108 | same | proto onboarding/use-finish.ts:52 | — | |
| Schedule install refused: the scheduler's message, translated where the app knows it (Windows refusing the task) | platform | use-finish-setup.ts:119-120 | same | proto onboarding/use-finish.ts:54 (macOS message only) | kept in words: ds:210 | |
| First backup blocked by FDA: the dedicated FDA message instead of the raw error | state | use-finish-setup.ts:166-168 | same | proto onboarding/use-finish.ts:55 | — | |
| First backup can't start because another backup runs: `已經有一次備份在運行，結束後再試。` | state | use-finish-setup.ts:126 | missing | not drawn | parity:kept-not-drawn.onboarding-ai.json `onboarding.done.alreadyRunning` | |
| The first backup here raises no toast | rule | use-finish-setup.ts:125 | missing | not modelled | kept in words: ds:210 | Per-type background tasks (ds:210) must not notify either |
| Live first-backup progress: `正在備份第 {done} / {total} 個來源 · {source}`, `{processed} / {records}`, `正在準備…` and an indeterminate bar until totals are known; fed live by the backup runner | live | done-step.tsx:62-64; backup/progress-card.tsx:5-63 | changed | proto onboarding/done-step.tsx:146-190 (one line per type) | ds:210 "只有「第一次備份」那一步每種選了的類型各一行進度" | Per type, the prototype keeps the counts and source name but has no indeterminate state: a type whose total isn't known yet shows a 2 % bar |
| Result: `完成，共保存 {count} 條記錄。`, where count = new **visits** (E2E asserts it equals the archived visits exactly) | text | done-step.tsx:220-222; tests/e2e/first-run.spec.ts:91-98 | changed | proto onboarding/done-step.tsx:99; onboarding/use-finish.ts:106 (sums visits **and** chats into one 條記錄 count) | **UNDECIDED** | One number then mixes visits and conversations. ds:210 doesn't say how the result reads with several types |
| Result: `部分所選瀏覽器讀取失敗。讀得到的都已儲存，是哪些、為什麼，請看「備份」。` when some browsers were saved and others not; setup still finishes | state | done-step.tsx:211-212, 229-233 | same | proto onboarding/done-step.tsx:102 | **UNDECIDED** | The real rule is browser-only. With two 備份 pages, 「備份」 no longer names one. Nothing says what shows when an Agent tool fails during the first backup (ds:154 covers it only for the enable flow) |
| Result: `Safari 已略過：PathKeep 還沒有「完整磁碟取用權限」。` | state | done-step.tsx:210, 224-228 | same | proto onboarding/done-step.tsx:101 | — | |
| Result when the backup was skipped: `設定已完成。準備好了就點「立即備份」。` | state | done-step.tsx:220-222 | same | proto onboarding/done-step.tsx:99 | — | |
| Result when local AI started: the model downloads in the background, see 設定 → AI | state | done-step.tsx:234-240; use-finish-setup.ts:134-138 | same | proto onboarding/done-step.tsx:103 | — | |
| Connecting an AI service fails: `無法設定 AI` + reason, skip `跳過，之後在「設定」裡設定` | state | use-finish-setup.ts:132-133 | same | proto onboarding/use-finish.ts:56 | — | |

### A.9 Added or invented in onboarding

| what | where | note |
| --- | --- | --- |
| Disabled `規劃中` cards for the planned types (終端機指令, 聊天訊息, …) on 要保存什麼 | proto onboarding/steps.tsx:103-135; proto kinds.tsx:94-112 (`real: false`) | **Contradicts** ds:37 "還沒做的類型不出現在任何地方：導航列、切換選單、新手引導、設定裡都沒有" |
| `?step=<id>` opens a step directly; `?many=1` loads the stress set | proto onboarding.tsx:50-57 | Prototype-only review aids. Today the address never picks a step, so the app must not let it skip validation |
| `原型：…` switches, `原型：離開新手引導`, scan/button selects under 來源 | proto onboarding.tsx:170-193; onboarding/proto-switches.tsx; onboarding/sources-step.tsx:333-353 | Prototype-only |
| The 本機語意搜尋 stage can fail | proto onboarding/use-finish.ts:122 with `failAt: 'ai'` | Today the local choice starts the download without waiting, so this stage never fails for it (use-finish-setup.ts:134-138) |
| Welcome rows reordered by how long each keeps data: Claude Code first, Codex last before PathKeep | proto onboarding/steps.tsx:43-50 | ds:163 lists the browser rows and the two new ones but gives no order |
| `沒有符合「{filter}」的來源。` when the source filter matches nothing | proto onboarding/sources-step.tsx:284 | new text, not in the spec |
| Two-pane 來源 layout, `全選` / `全不選`, `篩選`, group checkboxes | proto onboarding/sources-step.tsx | asked by ds:178-186; still awaiting the user (ds:16) |
| Spec gap, not a loss: 來源 has no state for an Agent tool folder that can't be read (permission, disk not mounted). The enable flow has one (ds:152) | ds:194-197 | |

### A.10 Side findings in today's app (not prototype losses)

- On Windows and Linux the system title bar is off (`src-tauri/src/desktop_integration/window.rs:96-102`), but window buttons are drawn only by `AppFrame` (`src/app/shell/app-frame.tsx:50, 153`). Onboarding seems to have no minimize, maximize or close button there. Not run on those platforms.
- A failed `重新檢查` from the `找不到瀏覽器` or `PathKeep 無法掃描瀏覽器` notice shows no error: `browsers-step.tsx:126-131` renders it only inside the FDA notice.
- `screens-and-nav.md` §Onboarding says the first six steps write nothing except the browser choice. The schedule preview also writes `dueAfterHours` (`use-schedule-preview.ts:24`).
- That write happens only the first time each interval is picked (`use-schedule-preview.ts:22,28`, cached with `staleTime: Infinity`). Picking 每小時, then 每天一次, then 每小時 again leaves 24 h in the config. A setup closed at that point resumes on 每天一次, not on the last choice. Creating the archive writes the draft's interval (`use-finish-setup.ts:96-99`), so a finished setup is not affected.

### A.11 Could not check (onboarding)

- Nothing was run: not the app, not E2E, not `parity/check.ts` (it imports the message files being edited). Windows and Linux behaviour comes from reading the code.
- The model field inside the AI step's service form (its list, empty and failed states) is shared with Settings › AI and was left to that audit.

Counts: same 75 / moved 0 / changed 16 / missing 16; UNDECIDED 14.

---

## B. Home, Ask, and the full-window screens

### B.1 Home and the full-window screens

Real code read at HEAD for every file `git status` shows modified (`home-page.tsx`, `home-cards.tsx`, `alerts.ts`, `backup-button.tsx`, `run-link.ts`, `insights/links.ts`, `router.tsx`, `app-frame.tsx`, `nav-rail.tsx`, `messages/shell.ts`, `tests/e2e/home-alerts.spec.ts`); line numbers for those are HEAD line numbers. `session.tsx`, `index.tsx`, `backup-runner.tsx`, `lock-screen.tsx`, `status-screens.tsx`, `forgot-passcode.tsx`, `touch-id-unlock.tsx`, `quit-prompt.tsx`, `route-error.tsx`, `alerts-list.tsx`, `building.tsx`, `home-data.ts`, `queries.ts` are unmodified (working tree = HEAD).

Abbreviations: **DS** = `docs/features/data-spaces.md`, **TO** = `docs/design/data-spaces-tradeoff.md`, **P** = `docs/design/prototype-2026-11-agent-history/`, **SN** = `docs/design/screens-and-nav.md`.

A note on how the browsing Home is kept: the prototype draws it as a stand-in (`P/src/pages/browsing-home.tsx:20`, banner "做進 app 時以現在的頁面為準，一項都不少"), and the stand-in leaves out the alerts, all four stat cards, "反覆回來看的主題" and "來源". The only spec sentence that keeps today's Home is TO §4.5:90 「瀏覽紀錄的頁面這次不動，之後另外重做。」, which sits in the page-width section. DS has no "沿用現在的首頁" sentence of its own (compare DS §4:269, which spells it out for Backup). The banner is in the prototype, which is temporary. So every "same" row below for Home rests on that one sentence. It is treated as a blanket keep, as the brief says, but nothing lists the Home's contents.

#### Home and full-window screens

| # | feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
|---|---|---|---|---|---|---|---|
| 1 | Eyebrow: today's weekday and date | text | home-page.tsx:183 (HEAD) | same | TO §4.5:90; stand-in P/browsing-home.tsx:22 hard-codes a date | | |
| 2 | Title greeting by hour (<5 晚安, <12 早安, <18 午安, else 晚安) + 「，這是你走過的路」 | text | home-page.tsx:33-38,184 (HEAD); home.ts zh-TW 209-212 | same | TO §4.5:90 | | Stand-in shows only 「午安」 and adds a subtitle (see Added) |
| 3 | Backup status pill links to Backup | url | backup-button.tsx:61-62 (HEAD) | moved | to `#/browsing/backup`; P/browsing-home.tsx:27 | TO §4.2:50 | |
| 4 | Pill dot: pulsing while running, red if latest run failed, green if last success < 2× due interval, else brand | state | backup-button.tsx:25-38 (HEAD) | changed | inputs become per type (DS §5:276); P draws a fixed green dot | UNDECIDED | Which kind's last backup and latest run the browsing Home reports is not said |
| 5 | Pill text 「{ago}已備份」 / 「還沒有備份過」 + 「 · 下次 {time}」 or 「 · 自動備份已關閉」; relative time refreshes every 60 s | text / live | backup-button.tsx:40-58 (HEAD); use-now.ts:4-10 | same | TO §4.5:90 | | Stand-in writes 「上次備份 今天 15:02」 (P/browsing-home.tsx:29), stand-in only |
| 6 | Pill while backing up: 「正在備份第 {done} / {total} 個來源」 or 「備份中…」 | live | backup-button.tsx:43-53 (HEAD) | same | TO §4.5:90 | | |
| 7 | 立即備份: one manual run, disabled with a spinning icon and 「備份中… {n}%」 while running; a second press does nothing; run shared with Backup, ⌘K, onboarding | control | backup-button.tsx:71-88 (HEAD); backup-runner.tsx:135-161 | changed | P/browsing-home.tsx:31 backs up browsing only, toast only, no running state | UNDECIDED (scope); running state covered by README 「只是示意（立即備份…）」 | DS §6:348 and §4:275 back up all kinds; DS §4:238 (browsing only) is for backup pages |
| 8 | Done toast 「備份完成，新增 {n} 條記錄」 / 「沒有新增造訪」; partial: warning toast, 「{n} 則警告…」, 詳情 → `#/backup?run=ID`, 10 s | dialog / url | backup-runner.tsx:67-97; run-link.ts:7-9 (HEAD) | moved | run link to `#/browsing/backup?run=` | TO §4.2:50; README 只是示意 (立即備份) | |
| 9 | Failed toast 「備份失敗」 + Safari-without-Full-Disk-Access line, or no-profiles line, or the error; 10 s | state | backup-runner.tsx:100-118 | same | TO §4.5:90 | | If row 7 makes Home back up all kinds, 「找不到可以備份的瀏覽器設定檔」 needs rethinking |
| 10 | A tray-started backup shows the same toast and refreshes Home | live / text | backup-runner.tsx:121-133 | changed | DS §4:275 (tray backs up all kinds) | UNDECIDED | Toast counts only `newVisits` |
| 11 | After any backup, import or restore: snapshot re-read, all archive queries refreshed, Home cards update | live | backup-runner.tsx:53-58,146; queries.ts:16-20 | same | TO §4.5:90 | | |
| 12 | Alerts region 「需要你處理」 under the header on the screen the app opens on; nothing when there are no alerts | state | alerts-list.tsx:13-18; router.tsx:44-46 (HEAD) | changed | only on `#/browsing/home`, which `#/` no longer always opens (TO §4.2:50); not on the Agent space (TO §4.1:39); gone when browsing is off (DS §2:47, P/frame.tsx:62) | UNDECIDED | archive.md:152,307 |
| 13 | Automatic-backup alerts: one per scheduler issue (not info, not dismissible), error or warning tone, title and consequence from schedule copy, fallback 「自動備份需要處理」 | state | alerts.ts:44-59 (HEAD) | same | TO §4.5:90 | | Wake-up grace period lives in the backend (SN:69) |
| 14 | Automatic-backup alert action 「打開備份」 → `/backup` | url | alerts.ts:41,57 (HEAD) | changed | the schedule card moves to 設定 › 排程與背景工作 (TO §5:121; DS §8:386); backup page keeps a row (DS §4:256) | UNDECIDED | |
| 15 | Linux: issues that only say the platform can't check are not alerts | platform | alerts.ts:45-47 (HEAD) | same | TO §4.5:90 | | |
| 16 | 「讀不到 Safari 的歷史」 (macOS, Full Disk Access) → 打開備份 | state / platform | alerts.ts:61-75 (HEAD); platform-guidance.ts:95-103 | moved | link to `#/browsing/backup` | TO §4.2:50 | |
| 17 | 「PathKeep 找不到你的瀏覽器」 → 打開備份 | state | alerts.ts:61-75 (HEAD) | moved | link to `#/browsing/backup` | TO §4.2:50 | |
| 18 | 「鑰匙圈無法使用」 (encrypted, remember on, no keychain), no link | state | alerts.ts:79-86 (HEAD); e2e home-alerts.spec.ts:126-138 | same | TO §4.5:90 | | Reach: row 12 |
| 19 | 「存檔的密碼已經不在鑰匙圈裡」 → 打開安全性設定 | state | alerts.ts:87-94 (HEAD) | same | TO §4.5:90; `#/settings/:section` unchanged (TO §4.2:50) | | |
| 20 | 「存檔的密碼只有你知道」; detail differs without a keychain; 「存到鑰匙圈」 link only when a keychain exists | state | alerts.ts:95-107 (HEAD); e2e home-alerts.spec.ts:140-173 | same | TO §4.5:90 | | |
| 21 | 「我有妥善保管」 saves `passwordKeptElsewhere`; the alert is gone for good, also after reload | rule | alerts.ts:108-112 (HEAD); e2e home-alerts.spec.ts:175-183 | same | TO §4.5:90 | | |
| 22 | Narrow window: alert buttons stack (<640 px); two-column card rows collapse (<900 px) | state | alerts-list.tsx:45; home-page.tsx:194,199 (HEAD) | same | TO §4.5:90 | | |
| 23 | Stat 「已保存記錄」: total, 「本週新增 {n}」 (last 7 days of the trend), 「—」 on failed read, skeleton while loading or building; → History | state / url | home-page.tsx:122-134 (HEAD) | moved | link to `#/browsing/history` | TO §4.2:50 | Not in stand-in; kept by §4.5 |
| 24 | Stat 「時間跨度」: years+months / months / days, 「最早 {month year}」; → Insights | text / url | home-page.tsx:40-59,135-150 (HEAD) | moved | link to `#/browsing/insights` | TO §4.2:50 | |
| 25 | Stat 「存檔大小」 (sum of all storage), 「已加密」/「未加密」; → Settings › Storage | text | home-page.tsx:110-116,151-161 (HEAD) | changed | archive now also holds Agent history (DS §1:35, §8:390) | UNDECIDED | Whole archive or browsing part |
| 26 | Stat 「來源」: 「{n} 個瀏覽器」, 「{n} 個已暫停」 / 「全部在備份」; → Backup | text / url | home-page.tsx:117-118,162-171 (HEAD) | changed | link to `#/browsing/backup` (TO §4.2); 「已暫停」 vs DS §2:46 and §4:250 「狀態寫 `不備份`」 | UNDECIDED | |
| 27 | 30-day trend 「近 30 天瀏覽」 / 「每日瀏覽次數，所有瀏覽器」, area chart with tooltip | control | home-cards.tsx:42-123 (HEAD) | same | TO §4.5:90 | | Stand-in title 「最近 30 天」 (P/browsing/home-cards.tsx:31) |
| 28 | Change vs previous 30 days badge (title 「與前 30 天相比」), hidden when not a number | state | home-cards.tsx:53,65-74 (HEAD) | same | TO §4.5:90 | | Not in stand-in |
| 29 | Each card on failure: 「沒能載入這部分。」 + 再試一次 | state | home-cards.tsx:77-78,144-148,300-301,360-364 (HEAD); section-card.tsx:16-35 | same | TO §4.5:90 | | |
| 30 | Insights still building (first run after onboarding, or after a backup): skeleton + 「正在第一次整理你的洞察…」 / 「正在更新你的洞察…」, re-read every 5 s | state / live | building.tsx:16-35; home-data.ts:75-80; queries.ts:28 | same | TO §4.5:90 | | This is the just-onboarded state |
| 31 | On this day: up to 4 earlier years with visits; year, summary (else top 2 domains, else date), 「{n} 次瀏覽 · domains」 | text | home-cards.tsx:125-191 (HEAD) | same | TO §4.5:90 | | |
| 32 | On this day row → `/history?date=YYYY-MM-DD` | url | home-cards.tsx:165 (HEAD) | moved | `#/browsing/history?date=` | TO §4.2:50 | Stand-in links without `date` (P/browsing/home-cards.tsx:77) |
| 33 | On this day empty 「往年的今天還沒有記錄。明年再來看看。」 | state | home-cards.tsx:156-159 (HEAD) | same | TO §4.5:90 | | |
| 34 | Year heatmap: Monday-first weeks up to today, cell labels 「{date} · {n} 次瀏覽」 / 「沒有瀏覽」, legend 少/多 | control | home-cards.tsx:193-223,294-311 (HEAD) | same | TO §4.5:90 | | |
| 35 | Year stepper 上一年/下一年, bounded by the earliest year with data and this year; previous year stays dimmed while the next loads | control | home-cards.tsx:231-235,266-299 (HEAD); queries.ts:47 | same | TO §4.5:90 | | |
| 36 | Click a day, or arrows + Enter/Space (one tab stop) → `/insights/day/YYYY-MM-DD` | keyboard / url | home-cards.tsx:308 (HEAD); heatmap.tsx:11,138-167 | moved | under `#/browsing/insights/…` | TO §4.2:50 (drill-in routes not listed; same rule) | Stand-in grid has no click (P/browsing/home-cards.tsx:98) |
| 37 | Year summary 「{year} 年共 {n} 次瀏覽 · 最活躍：{date}」 | text | home-cards.tsx:312-327 (HEAD) | same | TO §4.5:90 | | |
| 38 | 「反覆回來看的主題」: 4 most recent reopened topics in 90 days; icon, label, 「{n} 天 · {n} 次瀏覽」, time | text | home-cards.tsx:334-414 (HEAD) | same | TO §4.5:90 | | Not in stand-in |
| 39 | Topic → search or page drill-in with a 90-day scope in the URL | url | home-cards.tsx:380-385 (HEAD); links.ts:30-46 (HEAD) | moved | under `#/browsing/insights/…` | TO §4.2:50 | |
| 40 | 「全部 →」 → Insights | url | home-cards.tsx:352-357 (HEAD) | moved | `#/browsing/insights` | TO §4.2:50 | |
| 41 | Topics empty 「連續幾天都在看的主題會出現在這裡。」 | state | home-cards.tsx:372-375 (HEAD) | same | TO §4.5:90 | | |
| 42 | 「來源」 card: one row per browser (only profiles with a history file), backed-up first then by visits, count + bar; skeleton; 「—」 on failure | state | home-cards.tsx:416-482 (HEAD); home-data.ts:34-64 | same | TO §4.5:90 | | Not in stand-in |
| 43 | Browser not backed up: row dimmed, hover 「已暫停」 | text | home-cards.tsx:443-447 (HEAD) | changed | DS §2:46, §4:250 「來源沒有另外的「暫停」」 | UNDECIDED | Same issue as 26 |
| 44 | 「管理 →」 → Backup | url | home-cards.tsx:426-431 (HEAD) | moved | `#/browsing/backup` | TO §4.2:50 | |
| 45 | 「還沒有在備份任何瀏覽器。」 | state | home-cards.tsx:434-437 (HEAD) | same | TO §4.5:90 | | |
| 46 | Every Home date and "today" uses Insights' time zone | rule | queries.ts:1-4; home-page.tsx:103 (HEAD); home-cards.tsx:45 (HEAD) | same | TO §4.5:90 | | |
| 47 | Home is `#/`; unknown routes go to `#/` | url | router.tsx:44-46,87 (HEAD) | moved | `#/browsing/home`; `#/` → last space's first page | TO §4.2:50 | Consequence: row 12 |
| 48 | Units: 「{n} 次瀏覽」, 「已保存記錄」 | text | common.ts zh-TW visits; home.ts zh-TW 248 | changed | DS §1:33 「一筆資料叫 造訪」 | UNDECIDED | Unclear whether §1 renames these |
| 49 | Session gate order: app lock → crash journals/old format → keychain once → archive password → plaintext launch recovery → onboarding / ready; applies the language saved in config | rule | session.tsx:116-134,143-237 | missing | not drawn | kept in words: DS §9:414 「一次鎖定…一個復原流程」 (lock and recovery only; rule not written out) | |
| 50 | Boot screen 「正在打開存檔…」, spinner, window drags by the backdrop | state | status-screens.tsx:42-52; index.tsx:60-61 | missing | not drawn | UNDECIDED | |
| 51 | Boot error 「PathKeep 無法啟動」, error text, 再試一次, 查看日誌 | state | status-screens.tsx:92-114 | missing | not drawn | UNDECIDED | |
| 52 | App lock screen 「PathKeep 已鎖定」/「輸入密碼繼續。」, focused field, arrow button (off when empty or busy, spinner), wrong → shake, clear, refocus, 「密碼不對，請檢查後再試一次。」 | dialog | lock-screen.tsx:25-126 | missing | not drawn; P/settings/security-section.tsx:214 toast 「原型：這裡視窗會換成鎖定畫面。」 | kept in words: DS §9:414 「一次鎖定」 (rule not written out) | Which space tint it uses (TO §4.3) is not said |
| 53 | Archive password screen 「存檔已加密」/「輸入存檔密碼解鎖。」, 「存到系統鑰匙圈」 checked by default (saved only after a correct password; a failed save is ignored) | dialog | lock-screen.tsx:69-113; session.tsx:306-321 | missing | not drawn | kept in words: DS §1:35 「同一把密碼、同一次解鎖」, §9:414 | |
| 54 | Keychain unlock tried once at launch; falls back to the password screen | rule | session.tsx:90-98,198-207 | missing | not drawn | kept in words: DS §1:35 | |
| 55 | 「用 Touch ID 解鎖」 (macOS, only when on in Settings and supported); disabled with 「Touch ID 現在無法使用，請輸入密碼。」; failure note; cancelling is not a failure | control / platform | touch-id-unlock.tsx:21-73 | missing | not drawn | kept in words: DS §9:414 | |
| 56 | 「忘記密碼？」: hint or 「這組密碼沒有儲存提示。」, 「App 鎖定只擋住這個視窗…」, how to turn it off in config.json, config path, 「在 Finder 中顯示」 (macOS) / 「開啟資料夾」 (Windows, Linux), failure toast 「無法開啟資料夾」; deliberately no reset | dialog / platform | forgot-passcode.tsx:30-95 | missing | not drawn | kept in words: DS §9:414 | |
| 57 | Locking drops every cached read; the lock screen reads lock status itself | rule | session.tsx:136-140; lock-screen.tsx:33-38 | missing | not drawn | kept in words: DS §9:417 「鎖定時所有類型都看不到」 | |
| 58 | Lock from ⌘L / Ctrl+L, rail button, ⌘K, Settings 立即鎖定; with no passcode: toast 「先設定一組密碼，PathKeep 才能鎖定。」 and go to Settings › 安全 | keyboard / control | app-frame.tsx:39-45,84-91 (HEAD); nav-rail.tsx:195 (HEAD) | missing | rail button drawn without an action (P/shell/space-rail.tsx:286-288); ⌘L not bound (P/shell/frame.tsx:123 handles only K) | kept in words: TO §4.1:42; DS §6:341,348; parity kept-not-drawn.settings.json:35 | |
| 59 | Idle auto-lock after the configured minutes (pointer, key, wheel; checked every 15 s; a changed timeout applies at once) | rule | session.tsx:376-412 | missing | row drawn in P/settings/security-section.tsx:193 | kept in words: DS §9:414 | |
| 60 | Archive closes under the window → back through keychain / password / passcode | rule | session.tsx:264-281 | missing | not drawn | UNDECIDED | |
| 61 | Schema update while the archive opens: 「正在更新存檔」, body about the search index, progress 「{done} / {total}」; only while booting or locked | state | status-screens.tsx:55-90; index.tsx:51-58; schema-update.ts:15-28 | missing | not drawn | UNDECIDED | Body names one specific change |
| 62 | Old-format conversion: size, free space, files kept, resumable; password when encrypted; 「正在檢查存檔…」; 「有些檔案用的是以前的密碼」 with 返回 / 不帶上它們，繼續更新; phases; wrong password; 「更新沒有完成，存檔沒有被改動。」; unrecoverable → recovery | dialog | status-screens.tsx:121-303 | missing | not drawn | UNDECIDED | File label 「助理對話」 stays right: new roles are never named plain "agent" (architecture §5) |
| 63 | Recovery 「存檔打不開」: verified snapshots (date, size), password when one is encrypted, 「還原這個快照」 / 「正在還原…」, none → text, error, 查看日誌 | dialog | status-screens.tsx:305-394; session.tsx:219-229 | missing | not drawn | kept in words: DS §9:414 「一個復原流程」 | |
| 64 | Recovery when only one type's database is damaged | rule | status-screens.tsx:330-333 | changed | architecture §5 「damage to one kind does not take the other down」 vs one whole-window recovery | UNDECIDED | |
| 65 | Screen error 「這個畫面出錯了」, error text, 重新載入, logged; the nav stays when one screen failed | state | route-error.tsx:26-72; router.tsx:35-41 (HEAD) | missing | not drawn | UNDECIDED | |
| 66 | Quit question over every screen (lock, onboarding included): running work + what quitting does; 不結束 / 現在結束 / 做完再結束; Esc = 不結束; focus on the dialog; waiting state with spinner; 「工作都做完了」 with 不結束 / 結束; ignores stale replies; reads a pending question on reload | dialog / keyboard | quit-prompt.tsx:53-186; index.tsx:93 | missing | not drawn | UNDECIDED | |
| 67 | Quit question's list of work kinds | text | quit-prompt.tsx:38-51; shell.ts zh-TW quit.work / ifQuit | changed | v0.6 adds deleting one type (DS §2:62-74), first save (DS §3.2), per-type backups, reparse/verify/rebuild (architecture §9) | UNDECIDED | |
| 68 | "Closing the window keeps it running" | rule / platform | quit-prompt.tsx:2-5; SN:127,133 | changed | DS §3.2:135, §5.2:306 | UNDECIDED | Not true with the menu bar icon off |
| 69 | After 「刪除所有資料」 the app boots from nothing and lands on onboarding | rule | session.tsx:323-332 | missing | not drawn | kept in words: DS §9:414 「一個「刪除所有資料」」 | |

#### Added or invented (Home and full-window screens)

- **A cross-type Home that the chosen shell no longer has.** `P/src/pages/home.tsx` and `P/src/pages/home/cards.tsx` include 「保存了什麼」 (one card per type, with a 「示意」 badge and an 「只剩 PathKeep 有」 tag), 「接著做」 (latest chats with a copy-resume-command button), 「這週的 Agent」, multi-type 往年今日, a year card with a type toggle, per-type 來源, and 「還沒有開啟任何資料類型」 → `/settings/kinds`. It is reachable only with the prototype option 導航 = 依類型（上一輪） (`P/src/pages/root.tsx:9`, `P/src/shell/knobs.tsx:42-45`), which is option B, rejected in TO §2:17. No spec line asks for it, and the Agent space has no Home (TO §4.1:39). 「還沒有開啟任何資料類型」 also contradicts DS §2:54 (at least one type stays on). Anyone building from the prototype could pick these cards up by mistake.
- **Stand-in browsing Home text that isn't today's Home.** A header subtitle 「已保存 182 萬筆瀏覽紀錄，從 2019 年 1 月開始；這週新增 8,412 筆。」 (`browsing-home.tsx:24`), the title without 「，這是你走過的路」, 「最近 30 天 / 每天的造訪，共 {n} 筆」, an on-this-day that lists sample pages, and 「這一年 / 每天瀏覽的頁數」 counted in 頁 (`P/src/pages/browsing/home-cards.tsx:31,67-86,97`). The banner says the real page wins. It is listed here so nobody copies the text.
- **A start-of-backup toast on Home.** 「開始備份「瀏覽紀錄」」 (`browsing-home.tsx:31`). Today there is no toast when a backup starts. DS §4:238 asks for it only on the backup pages.
- **The Agent intro card (asked for by DS §3.1), where the prototype differs from the spec.**
  - It shows whenever Agent is off, with no check that a tool folder exists (`browsing-home.tsx:38`; `cards.tsx:47-50`). DS §3.1:86 requires at least one found folder.
  - 以後再說 and × hide it only for the component's lifetime (`cards.tsx:49,70-79`), so it comes back on the next visit. DS §3.1:93 says it doesn't come back by itself, and nothing says where that choice is saved.
  - It adds a paragraph 「Claude Code 預設 30 天後刪掉舊對話…」 (`cards.tsx:58-60`) that isn't in §3.1.
  - It lists a fixed first three tools (`cards.tsx:63`) instead of the ones found.
  - Its order relative to the alerts isn't specified.

#### Could not check (Home and full-window screens)

- Runtime behaviour. This was a read-only audit: no app run, no E2E run.
- Whether the backend blocks assistant and MCP reads of Agent history under App Lock (DS §9:417; SN:160 says history MCP is blocked today). That is backend work, outside this frontend audit.
- Whether v0.6 runs an open-time schema migration, which decides whether row 61's text would be shown at all.
- Touch ID has no E2E (headless Chrome; `tests/e2e/lock.spec.ts:12-14`). Row 55 comes from code only.
- Each scheduler issue's own title and consequence (`scheduleCopy` keys) is not listed one by one. Row 13 covers them as a group (SN:69 lists the cases).
- The lock, rail and ⌘K entry points belong to the shell area. Only their link to the lock screen is checked here (row 58).

Counts: same 27 / moved 12 / changed 12 / missing 18; UNDECIDED 19.

### B.2 Ask (the assistant page)

Real code was read at HEAD for the modified files (`src/app/router.tsx`, `src/app/shell/command-palette.tsx`, `src/app/shell/nav-rail.tsx`, `src/features/ask/citations.ts`); line numbers for those are HEAD lines. Every other file in `src/features/ask/` is unmodified. The prototype's Ask is `docs/design/prototype-2026-11-agent-history/src/pages/ask.tsx` (127 lines, the whole page), with `src/shell/palette.tsx`, `src/shell/space-rail.tsx` and `src/main.tsx`. Prototype paths below are relative to `docs/design/prototype-2026-11-agent-history/`. zh-TW strings come from the message files: `ask.ts`, `common.ts`, `insights.ts:978+`, `settings-storage.ts:358+`.

**Root cause behind most rows:**

- **The prototype's Ask page is a redraw, not a stand-in.**
  - It has no `NotRedrawn` banner. Only `browsing-home.tsx:20`, `browsing.tsx:68` and `browsing-insights.tsx:39` carry one.
  - There is no `parity/*.json` for Ask.
  - It imports nothing from `@/features/ask`.
- **It draws only §7's scope row and an empty starter.** Its composer does nothing: the send button has no `onClick` and the textarea has no key handling.
- **No spec line says the rest of Ask stays as it is.**
  - `data-spaces.md` §7 lists only the new scope states.
  - Tradeoff §4.5 「瀏覽紀錄的頁面這次不動」 covers the browsing space's own pages, not the shared `#/ask`.
  - `screens-and-nav.md:72` and `intelligence.md` §2 describe today's Ask, and tradeoff §5 does not overturn them. They are descriptions of today, not v0.6 decisions.
- **Tradeoff :133 「原型要跟現在的 app 一樣完整」** requires each kept item to be drawn, or listed as 刻意改掉 with a reason. Ask has neither.
- **Result: almost every row is `missing` and UNDECIDED.** One §7 sentence (for example 「助理頁其他的都照現在的 app」) plus an Ask parity list would settle most of them.

#### Ask

| feature | kind | real file:line | verdict | prototype/spec location | decision or UNDECIDED | note |
|---|---|---|---|---|---|---|
| 1. Route `#/ask` | url | HEAD `src/app/router.tsx:74-75` | same | `src/main.tsx:46` | tradeoff §4.2 「共用的畫面不帶空間：`#/settings/:section`、`#/ask`、`#/tasks`」 | Skeleton and error wrapping split off into row 65 |
| 2. Rail item 助理 | control | HEAD `src/app/shell/nav-rail.tsx:52` | moved | `src/shell/space-rail.tsx:282`, the shared group below the line | tradeoff §4.1 item 5 「共用的項目…助理」 | |
| 3. ⌘K 前往 › 助理 | control | HEAD `src/app/shell/command-palette.tsx:47-53` | missing | `src/shell/palette.tsx:51-137` has no 助理 item and no 前往 group | kept in words: §6 「另有 `助理`、`背景任務`、`設定`」 | |
| 4. ⌘K 操作 › 新對話 → `/ask?new=1` | control | HEAD `command-palette.tsx:128-133` | missing | `palette.tsx` has no 操作 group | kept in words: §6 「`新對話`，照現在的寫法」 | |
| 5. `?new=1` starts a fresh chat (stopping any running answer) and clears the URL | url | `src/features/ask/ask-page.tsx:137-144` | missing | `pages/ask.tsx` reads no search params | kept in words: §6, 新對話 「照現在的寫法」 | Not written: that it stops an answer in progress |
| 6. `?q=<question>` (Insights 問問助理, `insights.status.ask`) opens a new chat with the question in the box, unsent, and clears the URL | url | `ask-page.tsx:133-144`; `composer.tsx:42,46`; `src/features/insights/parts.tsx:265-270`; `tests/e2e/ask.spec.ts:73-91` | missing | `pages/ask.tsx` ignores params | UNDECIDED | The sender side is kept by tradeoff §4.5; the receiving side is not written. §7 scopes such a chat to the current space (browsing), which works |
| 7. ⌘N / Ctrl+N starts a new chat (only while on Ask; either modifier on every platform) | keyboard | `ask-page.tsx:146-155` | missing | none | UNDECIDED | `screens-and-nav.md:72` documents it; tradeoff §4.6 lists only ⌘1–9 and ⌘K |
| 8. 新對話 button shows the hint `⌘N` | platform | `conversation-list.tsx:141` | missing | none | UNDECIDED | Hard-coded ⌘ on Windows and Linux (already wrong today) |
| 9. Chat list column (aside, aria 對話, window drag region) | control | `conversation-list.tsx:128-133`; `ask.ts:242` | missing | none | UNDECIDED | §7 assumes saved chats but never describes the list |
| 10. 新對話 button | control | `conversation-list.tsx:134-142`; `ask.ts:239` | missing | none | UNDECIDED | |
| 11. Grouped by day of last update (今天 / 昨天 / medium date), newest first | rule | `conversations.ts:32-43`; `conversation-list.tsx:84-89,175-194` | missing | none | UNDECIDED | |
| 12. Row: title (新對話 when untitled) and time; the open chat is highlighted (`aria-current`) | control | `conversation-list.tsx:275-308` | missing | none | UNDECIDED | |
| 13. Draft row for the unsaved open chat under 今天, titled with the first 60 characters of the first question (新對話 before any question) | state | `ask-page.tsx:162-171`; `conversation-list.tsx:79-80,144-153` | missing | none | UNDECIDED | `draftTitle` is always a string, so a new, empty chat already shows a draft row |
| 14. List loading: 3 skeleton rows | state | `conversation-list.tsx:155-161` | missing | none | UNDECIDED | |
| 15. List error 無法載入你的對話。 | state | `conversation-list.tsx:163-167`; `ask.ts:244` | missing | none | UNDECIDED | |
| 16. List empty 你的對話會顯示在這裡。 | state | `conversation-list.tsx:169-173`; `ask.ts:243` | missing | none | UNDECIDED | Unreachable today: it needs `!showDraft`, but with no saved chats the open chat is never in the list, so the draft row (row 13) always shows instead |
| 17. Every chat reachable: pages of 100 load as the list scrolls, with a skeleton while the next page loads | live | `conversations.ts:9-25`; `conversation-list.tsx:196-202,246-265` | missing | none | UNDECIDED | `intelligence.md:121` requires following the cursors |
| 18. Row ⋯ menu (aria 對話選項), shown on hover or focus | control | `conversation-list.tsx:309-338` | missing | none | UNDECIDED | |
| 19. Rename: dialog 重新命名對話, field 標題 (autofocus, at most 120 characters, trimmed), 取消 / 儲存 (disabled while empty or saving), toast 無法重新命名這段對話。 | dialog | `conversation-list.tsx:343-446` | missing | none | UNDECIDED | The README's 「改名…只是示意」 covers drawn controls; nothing is drawn here |
| 20. A renamed title survives the next save | rule | `use-chat.ts:30-31,62-70`; `ask-page.tsx:91` | missing | none | UNDECIDED | |
| 21. 匯出為 Markdown…: the system save dialog; cancel does nothing; toast 已存成 Markdown with the path and 在 Finder 中顯示 (macOS) / 在資料夾中顯示 (others); 這段對話已經不在了。 and the list refreshes; 無法匯出這段對話。 with the reason | dialog | `conversation-list.tsx:93-126,326-331`; `src/features/settings/reveal.ts:7-11`; `tests/e2e/ask-chats.spec.ts:438` | missing | none | UNDECIDED | The README's 「匯出…只是示意」 does not cover an undrawn control |
| 22. Delete: confirm 要刪除這段對話嗎？ / 「{title}」會從這台電腦移除，不會影響你的瀏覽歷史。 / 取消 / 刪除; no undo; toast 無法刪除這段對話。; deleting the open chat starts a new one | dialog | `conversation-list.tsx:209-241`; `ask-page.tsx:176-178`; `ask-chats.spec.ts:292` | missing | none | UNDECIDED | The body is browsing-only once a chat can read Agent 對話. Agent chats use trash with undo and no confirm (agent-history §8.2) |
| 23. Opening a saved chat shows a skeleton (status 正在開啟對話…) | state | `ask-page.tsx:106-131,188-189,268-280` | missing | none | UNDECIDED | |
| 24. Open failure: toast 無法開啟這段對話。 | state | `ask-page.tsx:124-125`; `ask.ts:283` | missing | none | UNDECIDED | |
| 25. Click A then B: A's late load is dropped | rule | `ask-page.tsx:84,108,112,127` | missing | none | UNDECIDED | |
| 26. Opening a chat restores the model it last answered with, when that model is still offered | rule | `ask-page.tsx:116-122` | missing | none | UNDECIDED | |
| 27. A reopened chat shows the same question, thinking, searches, answer, model, tokens and sources, in the same order | rule | `chat-model.ts:130-160`; `ask-chats.spec.ts:368` | missing | none | UNDECIDED | |
| 28. An answer saved mid-stream (after a crash) reopens as stopped; pending tool steps no longer spin | rule | `chat-model.ts:139-140,154-158` | missing | none | UNDECIDED | |
| 29. Opening another chat, starting a new one or `?new=1` while answering stops the answer and saves it as stopped | rule | `use-chat.ts:283-293` | missing | none | UNDECIDED | |
| 30. Leaving Ask while answering stops the answer and saves it as stopped | rule | `use-chat.ts:295-303` | missing | none | UNDECIDED | The prototype's space switch navigates to that space's first page (`src/shell/space-rail.tsx:97-107`), so ⌘1–9 from Ask would also stop an answer |
| 31. Saved after every finished, failed or stopped answer; a failed save is silent | rule | `use-chat.ts:62-73,96-111` | missing | none | UNDECIDED | |
| 32. Starter heading 問問你走過的路 with the brand mark | text | `empty-states.tsx:24-28`; `ask.ts:264` | changed | `pages/ask.tsx:89` 想找什麼？, no brand mark | §7 「能問的時候：`想找什麼？`」 | The brand mark is dropped silently (minor) |
| 33. Starter body says whether the model is local (nothing leaves the computer) or online (「{provider} 是線上服務，你提問時會把所需的片段傳送過去」) | text | `empty-states.tsx:29-33`; `ask-page.tsx:70`; `ask.ts:265-268` | changed | `pages/ask.tsx:90-92`; §7 body 「只會讀「{類型}」。助理用你在「設定 › AI」裡選的模型…」 | UNDECIDED | §7's text drops it, and no line records the drop. `screens-and-nav.md:72` documents the disclosure |
| 34. Four example questions (`ask.empty.example1-4`, group 可以這樣問); a click sends at once | control | `empty-states.tsx:17-50`; `ask-page.tsx:192` | changed | `pages/ask.tsx:16-19,38,93-99`: up to 3 hard-coded strings per type; a click only fills the box | UNDECIDED | §7 says nothing about examples |
| 35. No AI service (AI off, assistant off, provider disabled, no model, or remote without a key): 用日常語言詢問你的歷史 / body / 前往「設定 → AI」 / 沒有助理，PathKeep 也完全可用…; no composer | state | `ask-page.tsx:190-195,227`; `empty-states.tsx:55-77`; `src/features/ai-setup/providers.ts:144-170` | missing | `pages/ask.tsx:22`, where `readable()` ignores AI setup | UNDECIDED | No precedence between this and §7's scope states |
| 36. Saved chats stay readable with no AI: the box becomes 要繼續提問，請先設定一個 AI 服務。 前往「設定 → AI」, and edit and retry are hidden | state | `ask-page.tsx:36-37,203,214-218,253-262`; `ask.ts:280` | missing | none | UNDECIDED | Also unwritten: a saved chat whose type is closed or no longer readable |
| 37. Composer box: aria 訊息, grows to 200 px, placeholder 問問你的瀏覽歷史 | control | `composer.tsx:71-91`; `ask.ts:292` | changed | `pages/ask.tsx:105-112`: placeholder `問問你的{類型}…`, disabled with 先選一個助理能讀的類型 | §7 「輸入框 `問問你的{類型}…`；沒有能讀的類型時 `先選一個助理能讀的類型`」 | The text is covered; the tooltip hint is in row 38 |
| 38. Enter sends, Shift+Enter breaks a line, Enter during IME composition does not send; tooltip 按 Enter 傳送，Shift+Enter 換行 | keyboard | `composer.tsx:76-89`; `ask.ts:295` | missing | `pages/ask.tsx:105-112` has no `onKeyDown` | UNDECIDED | |
| 39. Send button (aria 傳送), disabled when empty or while answering | control | `composer.tsx:58,114-123` | missing | `pages/ask.tsx:113-115`: drawn with a disabled rule, no `onClick` | UNDECIDED | Drawn but does nothing |
| 40. Stop button replaces send while answering (aria 停止) | control | `composer.tsx:104-113`; `use-chat.ts:249-255` | missing | none | UNDECIDED | |
| 41. The box takes focus when a chat is opened or started | rule | `composer.tsx:56` | missing | none | UNDECIDED | |
| 42. Model picker: `{service} · {model}`, aria 回答用的模型, disabled while answering; menu grouped by service; models read when it opens; 已載入 · 讀 {size} / 最多 {size}; 正在讀取模型…; 無法讀取 {service} 的模型。; the default and the current pick stay listed when the service can't be reached; embedding models left out | control | `model-picker.tsx:32-157` | missing | none | UNDECIDED | §7 「助理用你在「設定 › AI」裡選的模型」 pulls against it |
| 43. Which model answers: the pick if still offered, otherwise the one chosen in Settings | rule | `ask-page.tsx:58-68` | missing | none | UNDECIDED | |
| 44. Context meter `{used} / {limit}` or `{used} · 上限未知`, with tooltips; brand colour from 80 %, red when over | state | `context-meter.tsx:21-63`; `context-usage.ts:35-55` | missing | none | UNDECIDED | |
| 45. Context warning above the box: near (status) or over, 不會送出 (alert) | state | `context-meter.tsx:66-92`; `ask.ts:307-310`; `ask-agent.spec.ts:149` | missing | none | UNDECIDED | |
| 46. Context length re-read after each answer (the service may have loaded the model) | live | `ask-page.tsx:71-80,92-96` | missing | none | UNDECIDED | |
| 47. Question bubble | state | `user-turn.tsx:88-105` | missing | none | UNDECIDED | |
| 48. Edit a question: pencil (aria 編輯訊息) on hover or focus, hidden while answering or with no AI; inline box; Esc cancels; Enter, Shift+Enter and IME work as in the composer; 這則訊息之後的回答會被新的回答取代。; 取消 / 傳送; later answers dropped, also from the saved chat | control | `user-turn.tsx:12-106`; `use-chat.ts:257-272`; `ask-agent.spec.ts:212` | missing | none | UNDECIDED | |
| 49. Waiting indicator 處理中… | state | `assistant-turn.tsx:27-31,45-50` | missing | none | UNDECIDED | |
| 50. The answer renders as markdown while it streams (frame-batched); links open in the system browser, `file://` only if visited | live | `assistant-turn.tsx:52-56`; `markdown.tsx:33-72`; `use-chat.ts:75-94` | missing | none | UNDECIDED | |
| 51. Thinking block: 思考 / 思考 · 搜尋了 N 次存檔 / 搜尋了 N 次存檔; open while thinking, before any text; the user's toggle sticks; reasoning plus each call as `name(args)`, then 執行中…, the result, or 失敗; a 腳本 disclosure | control | `thinking-block.tsx:22-105`; `ask.ts:315-322` | missing | none | UNDECIDED | |
| 52. Text the model writes before a tool call moves into thinking; only the last turn is the answer | rule | `use-chat.ts:125-131`; `ask-agent.spec.ts:70` | missing | none | UNDECIDED | |
| 53. Seven answer notes: maxStepsReached, tokenBudgetReached, contextTooSmall, sourcesNotSaved, toolCallingUnavailable, searchIndexIncomplete, noAnswer | state | `assistant-turn.tsx:58-62`; `ask.ts:337-351` | missing | none | UNDECIDED | |
| 54. Error mid-stream: alert 助理無法回答 with the reason or 這個回答沒有完成。; the actions sit below it | state | `assistant-turn.tsx:64-77,91`; `ask-agent.spec.ts:303` | missing | none | UNDECIDED | |
| 55. Stopped: 已停止，上面的回答可能不完整。 | state | `assistant-turn.tsx:79-83`; `ask.ts:329` | missing | none | UNDECIDED | |
| 56. Sources: group 用到的頁面; chips with site icon, title or host, and tooltip `{title} · {date}`; one chip per canonical URL; 6 shown, then 另外 N 個 / 收合; each links to History `?q=…&visit=` | control | `sources.tsx:9-59`; HEAD `citations.ts:12-25` | missing | none | UNDECIDED | §7 「標出引用了哪幾筆紀錄」 does not say how an Agent 對話 citation looks or where it links |
| 57. 在歷史中開啟 (the first source) | control | `assistant-turn.tsx:94-101` | missing | none | route: tradeoff §4.2 (`#/browsing/history`); the control itself: UNDECIDED | |
| 58. 複製回答 → 已複製 for 1.5 s | control | `assistant-turn.tsx:35-39,102-107` | missing | none | UNDECIDED | |
| 59. 重試 (failed or stopped) / 重新回答 (done): last answer only, hidden while answering or with no AI | control | `assistant-turn.tsx:108-117`; `ask-page.tsx:214-218`; `use-chat.ts:274-281` | missing | none | UNDECIDED | |
| 60. The model that wrote the answer (title 寫這個回答的模型) and tokens 輸入 {prompt} · 輸出 {completion}, summed over the answer's requests | text | `assistant-turn.tsx:118-134`; `use-chat.ts:159-167` | missing | none | UNDECIDED | |
| 61. Stick to bottom: follows while answering; scrolling up more than 64 px stops it and scrolling back resumes it; pinned on send, open and edit | rule | `use-stick-to-bottom.ts:7-43`; `ask-page.tsx:123,158,206` | missing | none | UNDECIDED | |
| 62. Title-bar strip at the top of the chat column, and a drag region on the list | platform | `ask-page.tsx:186`; `conversation-list.tsx:130` | missing | none in `pages/ask.tsx` | UNDECIDED | See Could not check |
| 63. Locked (⌘L, ⌘K 鎖定, idle auto-lock): the lock screen replaces the whole app, so Ask unmounts, a running answer stops and is saved as stopped, and the typed text is lost | state | `src/app/index.tsx:64-67`; `src/app/session.tsx:373-403`; `use-chat.ts:295-303` | missing | the lock button at `src/shell/space-rail.tsx:286-288` has no handler | kept in words: §9 「鎖定時所有類型都看不到，助理和 MCP 也讀不到」 | Not written: that a running answer stops |
| 64. Saved chat format: id, title, provider, messages; no scope | rule | `use-chat.ts:62-70`; `chat-model.ts:102-113` | changed | §7 「每段助理對話記著它自己的範圍，回到那段對話時範圍還是一樣」; `pages/ask.tsx:27` holds the scope only in local state per mount | partly: §7 for new chats; UNDECIDED for chats saved before v0.6, list display or filter by scope, changing scope mid-chat, and a chat whose scope is closed or no longer readable | |
| 65. Skeleton while Ask's code loads; a per-screen error that keeps the rail and clears on navigation | state | HEAD `router.tsx:8-29,39-42` | missing | `src/main.tsx:27-53`: no `Suspense`, no `errorElement` | UNDECIDED | Applies to every screen; the shell area may own it |
| 66. No chat id in the URL: arriving at Ask (rail, ⌘K, reload, unlock, back) always opens a new empty chat; the last open chat is not restored | url | `use-chat.ts:44`; `ask-page.tsx:82-84,133-144` (only `new` and `q` are read) | same | `pages/ask.tsx:122-126`: fresh state on every mount, no params | not needed (no change) | Under §7, every arrival is a new chat scoped to the current or last space |

#### Added or invented (Ask)

- **`pages/ask.tsx:22`:** `readable()` hard-codes Agent 對話 as never readable. The allowed state (Agent ticked and askable) cannot be reached, so §7's 「能讀的打勾」 for Agent is never drawn.
- **`pages/ask.tsx:76-86`:** with nothing ticked, the heading is 選一個類型來問, but the Agent paragraph and the 到設定允許 button still show. §7 gives only the heading for that state.
- **`pages/ask.tsx:25-26`:** chips come only from `enabledKinds`. Paused exists only as local state in `pages/settings/kinds-section.tsx:34`, so Ask never draws a paused type's chip, although §7 asks for 「每種開著或暫停的類型一個標籤」.
- **`pages/ask.tsx:52,59,64`:** planned kinds appear as disabled chips with （規劃中）. No spec line asks for planned types in the scope row.
- **`pages/ask.tsx:124-125`:** in the `nav === 'types'` shell, a new chat starts with every readable type. This contradicts §7 「開新的對話時，範圍是使用者當下所在的空間」. The default `nav` is `spaces` (`src/proto-state.tsx:35`).
- **`pages/ask.tsx:16-19`:** per-type example questions are hard-coded, not taken from the message files. Tradeoff :133 says 「文字一律用 app 的訊息檔」.
- **`pages/ask.tsx:66-70,84`:** 在設定裡允許 and 到設定允許 go to `/settings/agent`, not to its 「AI 存取」 group. §7 says 「到「設定 › Agent 對話 › AI 存取」」; this is a small difference.

#### Could not check (Ask)

- **The prototype frame's title-bar strip and drag region.** I did not check whether `src/shell/frame.tsx` supplies them on shared pages. A grep finds no `TitleBar` or `drag`, and I stopped there because it is the shell area's job.
- **Backend side of scope.** Which tools and system prompt a chat scoped to Agent 對話 gets, and the citation contract for agent records: no spec or architecture text covers them. `agent-history-product-design.md` has nothing on Ask scope.
- **Windows and Linux behaviour.** The ⌘N hint, the reveal label and the Ctrl+N handler were read from code only; nothing was run.
- **Composer growth.** The prototype's shadcn `Textarea` grows with `field-sizing-content`, while the real composer caps at 200 px. I did not compare them in a browser.

same 2 / moved 1 / changed 5 / missing 58 — UNDECIDED 57 (rows 6–31, 33–36, 38–62, 65, and part of 64)
