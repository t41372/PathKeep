/**
 * What a run or an import recorded besides its counts, as zh-TW sentences.
 * Mirrors `src/features/backup/run-codes.ts` (which codes are warnings and
 * which are notes, the fix each points to, which raw text is evidence) and
 * `run-entries.ts` (one sentence per code; a parser code or a code it does
 * not know shows a lead-in with the recorded text under it). Every code the
 * app knows is here, so the run sheet and the import previews can show any.
 */
import { bytes, number } from './format'
import { profileName } from './sources-data'

export type RunAction = 'full-disk-access' | 'ai-settings' | 'background-settings'

/** A recorded entry (`BackupWarning` / `TakeoutNote`). An empty code is an entry from before codes. */
export interface Recorded {
  code: string
  message: string
  count?: number
  succeeded?: number
  bytes?: number
  profileId?: string
  diagnostic?: string
}

export interface RunEntry {
  text: string
  evidence?: string
  action?: RunAction
}

interface Params {
  profile: string
  count: string
  succeeded: string
  bytes: string
}

interface Rule {
  text: (p: Params) => string
  action?: RunAction
  evidence?: 'diagnostic' | 'message'
}

const WARNINGS: Record<string, Rule> = {
  'safari-full-disk-access-skip': {
    text: (p) => `已略過 ${p.profile}：沒有完整磁碟取用權限，PathKeep 讀不到 Safari 的歷史記錄。授權後，下次備份就會包含 Safari。`,
    action: 'full-disk-access',
  },
  'profile-history-unreadable-skip': { text: (p) => `已略過 ${p.profile}：它的歷史檔案不存在或無法讀取。`, evidence: 'diagnostic' },
  'profile-not-detected-skip': { text: (p) => `已略過 ${p.profile}：這台電腦上已經找不到它。之前備份的內容仍在存檔中。` },
  'source-profile-not-discovered': { text: (p) => `已略過 ${p.profile}：這台電腦上已經找不到它。之前備份的內容仍在存檔中。` },
  'source-unreadable': { text: (p) => `已略過 ${p.profile}：它的歷史檔案不存在或無法讀取。`, evidence: 'diagnostic' },
  'source-permission-denied': {
    text: (p) => `已略過 ${p.profile}：PathKeep 沒有讀取它歷史紀錄的權限。給予「完整磁碟取用權限」後，下次備份就會包含它。`,
    action: 'full-disk-access',
    evidence: 'diagnostic',
  },
  'source-read-failed': { text: (p) => `無法讀取 ${p.profile} 的歷史檔案，這次沒有從它加入任何內容。`, evidence: 'diagnostic' },
  'source-stream-failed': { text: (p) => `讀取 ${p.profile} 的歷史紀錄中途停止了，之前讀到的部分已儲存。`, evidence: 'diagnostic' },
  'source-favicons-unavailable': { text: (p) => `無法讀取 ${p.profile} 的網站圖示。歷史已儲存，沒有加入新圖示。`, evidence: 'diagnostic' },
  'staging-fallback-recovered-copy': {
    text: (p) => `${p.profile} 正在使用中，因此 PathKeep 讀取的是歷史檔案的副本，而不是正在使用的檔案。`,
    evidence: 'diagnostic',
  },
  'staging-fallback-repaired-copy': { text: (p) => `${p.profile} 的歷史檔案需要修復，因此 PathKeep 讀取的是修復後的副本。`, evidence: 'diagnostic' },
  'git-history-skipped': { text: () => '這次執行的記錄已儲存，但未能寫入 Git 歷史。', evidence: 'diagnostic' },
  'intelligence-refresh-failed': { text: () => '洞察沒能開始用這次備份的造訪記錄更新。', evidence: 'diagnostic' },
  'ai-autoindex-enqueue-failed': { text: () => '新頁面沒能加入語意搜尋的索引佇列。', evidence: 'diagnostic' },
  'ai-autoindex-provider-not-ready': {
    text: () => '語意搜尋的模型還沒準備好，因此新頁面沒有加入索引佇列。',
    action: 'ai-settings',
    evidence: 'diagnostic',
  },
  'enrichment-reoffer-failed': { text: () => '等待抓取內容的頁面沒能加入佇列。', evidence: 'message' },
  'og-refetch-failed': { text: () => '重試之前失敗的連結預覽時出錯。', evidence: 'diagnostic' },
  'og-prefetch-failed': { text: () => '沒能取得新頁面的連結預覽。', evidence: 'diagnostic' },
  'og-cleanup-failed': { text: () => '清理連結預覽快取失敗。', evidence: 'diagnostic' },
  'review-record-failed': { text: () => '內容都已儲存，但這次執行的記錄沒能寫入。', evidence: 'diagnostic' },
  'skipped-missing-timestamp': { text: (p) => `略過了匯入檔案中的 ${p.count} 筆記錄，因為它們沒有時間。` },
  'browser-skipped-missing-url-row': { text: (p) => `略過了匯入檔案中的 ${p.count} 次造訪，因為缺少網址。` },
  'parse-failed': { text: () => '匯入的部分內容無法讀取。', evidence: 'message' },
  'no-importable-files': { text: () => '匯入中沒有 PathKeep 能讀取的歷史檔案。' },
}

const NOTES: Record<string, Rule> = {
  'source-ids-reset': { text: (p) => `已重新核對 ${p.profile} 中仍可讀取的全部歷史紀錄。` },
  'source-visits-without-page': {
    text: (p) => `${p.profile} 的歷史紀錄中有 ${p.count} 筆在瀏覽器檔案裡找不到對應的頁面，沒有網址可以儲存。`,
  },
  'profile-search-terms-captured': { text: (p) => `儲存了在 ${p.profile} 中輸入的 ${p.count} 次搜尋。` },
  'og-prefetch-summary': { text: (p) => `連結預覽：${p.count} 個新頁面中取得了 ${p.succeeded} 個。` },
  'og-refetch-summary': { text: (p) => `連結預覽：重試了 ${p.count} 個之前失敗的頁面，這次成功 ${p.succeeded} 個。` },
  'og-cleanup-summary': { text: (p) => `連結預覽快取：移除了 ${p.count} 筆舊記錄，釋放了 ${p.bytes}。` },
  'ai-autoindex-queued-while-paused': {
    text: () => '新頁面已加入語意搜尋的索引佇列，背景工作恢復後就會建立索引。',
    action: 'background-settings',
  },
  'batch-reverted': { text: (p) => `已復原：這次匯入的 ${p.count} 筆瀏覽記錄已隱藏。` },
  'batch-restored': { text: (p) => `已還原：這次匯入的 ${p.count} 筆瀏覽記錄已重新顯示。` },
}

const PARSER_PREFIX = 'parser-'

/** Sorts recorded entries into warnings and notes, each as one sentence. */
export function describe(entries: Recorded[], skip: readonly string[] = []) {
  const warnings: RunEntry[] = []
  const notes: RunEntry[] = []
  for (const entry of entries) {
    if (skip.includes(entry.code)) continue
    const params: Params = {
      profile: profileName(entry.profileId),
      // `t()` writes a number in the language's digits: 1,840.
      count: number(entry.count ?? 0),
      succeeded: number(entry.succeeded ?? 0),
      bytes: bytes(entry.bytes ?? 0),
    }
    const evidence = (rule: Rule) =>
      rule.evidence === 'message' ? entry.message : rule.evidence === 'diagnostic' ? entry.diagnostic : undefined
    const note = NOTES[entry.code]
    const warning = WARNINGS[entry.code]
    if (note) notes.push({ text: note.text(params), evidence: evidence(note), action: note.action })
    else if (warning) warnings.push({ text: warning.text(params), evidence: evidence(warning), action: warning.action })
    else if (entry.code.startsWith(PARSER_PREFIX))
      warnings.push({ text: '匯入檔案回報了一個問題。', evidence: entry.diagnostic ?? entry.message })
    else warnings.push({ text: '這次執行記錄了：', evidence: entry.message })
  }
  return { warnings, notes }
}
