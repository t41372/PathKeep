/**
 * Invented inspections and import batches for the two import cards and
 * 最近的匯入, shaped like `TakeoutInspection`, `ImportBatchOverview` and
 * `ImportBatchDetail`. One of each case the cards draw differently.
 */
import { NOW } from '~/data/fixture'
import type { Recorded } from './run-entries'

const HOUR = 3600_000
const DAY = 24 * HOUR

export interface PreviewEntry {
  url: string
  title?: string | null
  visitedAt: number
  sourceVisitId: number
  /** `new`, `duplicate` (already in the archive) or `same-visit` (a Firefox Sync copy). */
  status: string
}

export interface Inspection {
  recognizedFiles: { path: string; kind: string }[]
  quarantinedFiles: { path: string; reasonCode?: string }[]
  previewEntries: PreviewEntry[]
  candidateItems: number
  importedItems: number
  duplicateItems: number
  sameVisitItems?: number
  continuedFrom?: number
  stopped?: boolean
  notes: Recorded[]
  previewRangeStart?: number
  previewRangeEnd?: number
}

export interface ImportProgress {
  processedRecords: number | null
  totalRecords: number | null
  progressPercent: number | null
}

const empty: Inspection = {
  recognizedFiles: [],
  quarantinedFiles: [],
  previewEntries: [],
  candidateItems: 0,
  importedItems: 0,
  duplicateItems: 0,
  notes: [],
}

export const TAKEOUT_PATH = '/Users/lin/Downloads/takeout-20261102T081544Z-001.zip'

/** Takeout previews. */
export const takeoutFound: Inspection = {
  ...empty,
  recognizedFiles: [{ path: 'Takeout/Chrome/History.json', kind: 'chrome-history-json' }],
  quarantinedFiles: [
    { path: 'Takeout/Chrome/Dictionary.csv', reasonCode: 'unsupported-file' },
    { path: 'Takeout/Chrome/BrowserHistory-2.json', reasonCode: 'parse-failed' },
  ],
  candidateItems: 61_842,
  previewRangeStart: Date.UTC(2019, 2, 2),
  previewRangeEnd: Date.UTC(2026, 9, 30),
  notes: [
    { code: 'skipped-missing-timestamp', message: 'Skipped 37 records without a time', count: 37 },
    { code: 'parse-failed', message: 'Takeout/Chrome/BrowserHistory-2.json could not be parsed' },
  ],
}

export const takeoutNothing: Inspection = {
  ...empty,
  // Said by the headline and its hint already, so the card leaves it out.
  notes: [{ code: 'no-importable-files', message: 'No importable history files' }],
}

export const takeoutMyActivity: Inspection = {
  ...empty,
  quarantinedFiles: [{ path: 'Takeout/My Activity/Chrome/MyActivity.html', reasonCode: 'chrome-my-activity-html' }],
  notes: [{ code: 'no-importable-files', message: 'No importable history files' }],
}

/** Takeout results. */
export const takeoutDone: Inspection = { ...takeoutFound, importedItems: 48_210, duplicateItems: 13_595, sameVisitItems: 1_204 }
export const takeoutContinued: Inspection = { ...takeoutDone, continuedFrom: NOW - 2 * DAY }
export const takeoutStopped: Inspection = { ...takeoutFound, importedItems: 10_200, duplicateItems: 3_418, stopped: true }

export const FIREFOX_PATH = '/Users/lin/Desktop/舊電腦/Firefox/places.sqlite'
export const EDGE_FOLDER = '/Users/lin/Desktop/舊電腦/Edge/Profile 1'

/** Browser-file previews. */
export const firefoxPreview: Inspection = {
  ...empty,
  recognizedFiles: [{ path: FIREFOX_PATH, kind: 'firefox-places-db' }],
  candidateItems: 23_412,
  duplicateItems: 18_020,
  sameVisitItems: 640,
  previewRangeStart: Date.UTC(2021, 4, 14),
  previewRangeEnd: NOW - 30 * DAY,
  previewEntries: [
    { url: 'https://developer.mozilla.org/zh-TW/docs/Web/API/IndexedDB_API', title: 'IndexedDB API - Web API | MDN', visitedAt: NOW - 30 * DAY, sourceVisitId: 90311, status: 'new' },
    { url: 'https://www.ptt.cc/bbs/Soft_Job/index.html', title: '看板 Soft_Job 文章列表 - 批踢踢實業坊', visitedAt: NOW - 30 * DAY - 2 * HOUR, sourceVisitId: 90310, status: 'duplicate' },
    { url: 'https://github.com/tauri-apps/tauri/releases', title: 'Releases · tauri-apps/tauri', visitedAt: NOW - 30 * DAY - 3 * HOUR, sourceVisitId: 90309, status: 'same-visit' },
    { url: 'https://news.ycombinator.com/item?id=41022317', title: null, visitedAt: NOW - 31 * DAY, sourceVisitId: 90302, status: 'new' },
    { url: 'https://www.google.com/maps/place/台北車站', title: '台北車站 - Google 地圖', visitedAt: NOW - 31 * DAY - 5 * HOUR, sourceVisitId: 90298, status: 'new' },
  ],
  notes: [{ code: 'browser-skipped-missing-url-row', message: 'Skipped 12 visits without a URL', count: 12 }],
}

export const edgeNothingNew: Inspection = {
  ...empty,
  recognizedFiles: [{ path: `${EDGE_FOLDER}/History`, kind: 'chromium-history-db' }],
  candidateItems: 9_104,
  duplicateItems: 9_104,
  previewRangeStart: Date.UTC(2024, 0, 9),
  previewRangeEnd: NOW - 3 * DAY,
  previewEntries: [
    { url: 'https://learn.microsoft.com/zh-tw/windows/', title: 'Windows 文件', visitedAt: NOW - 3 * DAY, sourceVisitId: 5120, status: 'duplicate' },
    { url: 'https://outlook.live.com/mail/0/', title: '郵件 - Outlook', visitedAt: NOW - 3 * DAY - HOUR, sourceVisitId: 5119, status: 'duplicate' },
  ],
}

export const firefoxDone: Inspection = { ...firefoxPreview, importedItems: 4_752 }
export const firefoxStopped: Inspection = { ...firefoxPreview, importedItems: 1_900, duplicateItems: 7_310, sameVisitItems: 210, stopped: true }

/** 最近的匯入. */
export interface Batch {
  id: number
  sourceKind: 'takeout' | 'browser-file'
  sourcePath: string
  createdAt: number
  importedAt?: number
  /** running, imported, reverted, reverting, restoring, interrupted, stopped, failed. */
  status: string
  importedItems: number
  duplicateItems: number
  visibleItems: number
  auditPath?: string
}

export interface BatchDetail {
  previewEntries: PreviewEntry[]
  notes: Recorded[]
  previewRangeStart?: number
  previewRangeEnd?: number
}

const AUDIT = '/Users/lin/Library/Application Support/PathKeep/archive/audit/imports'

export const batches: Batch[] = [
  // Broken off when the app closed: the backend lists unfinished ones first.
  { id: 31, sourceKind: 'takeout', sourcePath: '/Users/lin/Downloads/takeout-20261025T020011Z-001.zip', createdAt: NOW - 2 * DAY, status: 'interrupted', importedItems: 10_200, duplicateItems: 0, visibleItems: 10_200, auditPath: `${AUDIT}/31.json` },
  { id: 30, sourceKind: 'browser-file', sourcePath: '/Users/lin/Desktop/舊電腦/Chrome/History', createdAt: NOW - 3 * DAY, importedAt: NOW - 3 * DAY, status: 'reverting', importedItems: 6_480, duplicateItems: 1_022, visibleItems: 2_900, auditPath: `${AUDIT}/30.json` },
  { id: 29, sourceKind: 'takeout', sourcePath: '/Users/lin/Downloads/takeout-20260901T110203Z-001.zip', createdAt: NOW - 4 * DAY, importedAt: NOW - 4 * DAY, status: 'restoring', importedItems: 3_120, duplicateItems: 88, visibleItems: 1_400, auditPath: `${AUDIT}/29.json` },
  { id: 28, sourceKind: 'takeout', sourcePath: TAKEOUT_PATH, createdAt: NOW - 5 * HOUR, importedAt: NOW - 5 * HOUR, status: 'imported', importedItems: 48_210, duplicateItems: 13_595, visibleItems: 48_210, auditPath: `${AUDIT}/28.json` },
  { id: 27, sourceKind: 'browser-file', sourcePath: '/Users/lin/Desktop/舊電腦/Safari/History.db', createdAt: NOW - DAY, importedAt: NOW - DAY, status: 'reverted', importedItems: 3_120, duplicateItems: 214, visibleItems: 0, auditPath: `${AUDIT}/27.json` },
  { id: 26, sourceKind: 'browser-file', sourcePath: FIREFOX_PATH, createdAt: NOW - 6 * DAY, importedAt: NOW - 6 * DAY, status: 'stopped', importedItems: 1_900, duplicateItems: 7_310, visibleItems: 1_900, auditPath: `${AUDIT}/26.json` },
  { id: 25, sourceKind: 'takeout', sourcePath: '/Users/lin/Downloads/takeout-20260610T090000Z-001.zip', createdAt: NOW - 9 * DAY, importedAt: NOW - 9 * DAY, status: 'failed', importedItems: 512, duplicateItems: 40, visibleItems: 512, auditPath: `${AUDIT}/25.json` },
]

/** While an import runs, its batch is listed first with no action. */
export const runningBatch = (sourceKind: Batch['sourceKind'], sourcePath: string): Batch => ({
  id: 32,
  sourceKind,
  sourcePath,
  createdAt: NOW - 40_000,
  status: 'running',
  importedItems: 0,
  duplicateItems: 0,
  visibleItems: 0,
})

const sample = (prefix: string, at: number): PreviewEntry[] =>
  ['搜尋結果', '文件', '討論區', '影片', '新聞'].map((title, i) => ({
    url: `https://example.org/${prefix}/${i}`,
    title: `${prefix} ${title}`,
    visitedAt: at - i * 37 * 60_000,
    sourceVisitId: 1000 + i,
    status: 'new',
  }))

export function batchDetail(batch: Batch): BatchDetail {
  const at = batch.importedAt ?? batch.createdAt
  return {
    previewRangeStart: Date.UTC(2020, 6, 1),
    previewRangeEnd: at - 10 * DAY,
    // A batch whose visits are all hidden has nothing to sample: 「它的記錄目前是隱藏的。」
    previewEntries: batch.visibleItems === 0 ? [] : sample(batch.sourceKind === 'takeout' ? 'Takeout' : 'History', at - 10 * DAY),
    notes: [
      ...(batch.sourceKind === 'takeout' ? [{ code: 'skipped-missing-timestamp', message: 'Skipped records without a time', count: 21 }] : []),
      ...(batch.status === 'reverted' || batch.status === 'restoring'
        ? [{ code: 'batch-reverted', message: 'Hid visits of this import', count: batch.importedItems }]
        : []),
      ...(batch.status === 'restoring' ? [{ code: 'batch-restored', message: 'Showed visits of this import again', count: 1_400 }] : []),
    ],
  }
}
