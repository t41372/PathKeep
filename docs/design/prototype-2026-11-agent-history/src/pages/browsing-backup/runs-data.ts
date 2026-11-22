/**
 * Invented runs for 最近記錄 and their audit detail (`load_audit_run_detail`),
 * shaped like `BackupRunOverview` / `AuditRunDetail`. One run of every kind
 * the runs card names, and every status it draws.
 *
 * Design change (data-spaces.md §4): this page lists only what each run did
 * for browsing history. A backup that also backed up Agent conversations
 * shows its browsing sources, visits and warnings only; a run that concerned
 * only another data type (`space: 'agent'`) is left out.
 */
import { NOW } from '~/data/fixture'
import type { Recorded } from './run-entries'

export interface Run {
  id: number
  startedAt: number
  finishedAt?: number | null
  status: 'success' | 'partial' | 'interrupted' | 'failed' | 'running'
  runType: string
  profilesProcessed: number
  newVisits: number
  newUrls: number
  errorMessage?: string | null
  /** Prototype: which data type the run concerned; the page keeps browsing. */
  space: 'browsing' | 'agent'
}

export interface Artifact {
  path: string
  createdAt: number
  sizeBytes?: number
  /** A known reason gets a sentence; another is shown as written; none is 「儲存的副本」. */
  reason?: string
}

export interface RunDetail {
  trigger: string
  timezone?: string
  dueOnly: boolean
  warnings: Recorded[]
  errorMessage?: string
  manifestPath?: string
  manifestHash?: string
  artifacts: Artifact[]
}

const MIN = 60_000
const HOUR = 60 * MIN
const DAY = 24 * HOUR
const ARCHIVE = '/Users/lin/Library/Application Support/PathKeep/archive'
const at = (daysAgo: number, hour: number, minute: number) => {
  const day = new Date(NOW - daysAgo * DAY)
  return Date.UTC(day.getUTCFullYear(), day.getUTCMonth(), day.getUTCDate(), hour - 8, minute)
}
const manifest = (id: number) => `${ARCHIVE}/manifests/run-${id}.json`

export const allRuns: Run[] = [
  { id: 418, startedAt: at(0, 15, 20), finishedAt: at(0, 15, 20) + 42_300, status: 'success', runType: 'backup', profilesProcessed: 8, newVisits: 1284, newUrls: 312, space: 'browsing' },
  { id: 417, startedAt: at(0, 15, 0), finishedAt: at(0, 15, 0) + 8_400, status: 'success', runType: 'backup', profilesProcessed: 0, newVisits: 0, newUrls: 0, space: 'agent' },
  { id: 416, startedAt: at(0, 14, 20), finishedAt: at(0, 14, 20) + 131_000, status: 'partial', runType: 'backup', profilesProcessed: 7, newVisits: 906, newUrls: 240, space: 'browsing' },
  { id: 415, startedAt: at(0, 11, 2), finishedAt: at(0, 11, 2) + 382_000, status: 'success', runType: 'import', profilesProcessed: 1, newVisits: 48_210, newUrls: 19_544, space: 'browsing' },
  { id: 414, startedAt: at(0, 9, 41), finishedAt: at(0, 9, 41) + 3_800, status: 'interrupted', runType: 'backup', profilesProcessed: 3, newVisits: 112, newUrls: 31, space: 'browsing' },
  { id: 413, startedAt: at(1, 22, 30), finishedAt: at(1, 22, 30) + 1_900, status: 'success', runType: 'restore', profilesProcessed: 0, newVisits: 0, newUrls: 0, space: 'browsing' },
  { id: 412, startedAt: at(1, 22, 15), finishedAt: at(1, 22, 15) + 2_100, status: 'success', runType: 'rollback', profilesProcessed: 0, newVisits: 3_120, newUrls: 0, space: 'browsing' },
  { id: 411, startedAt: at(1, 18, 5), finishedAt: at(1, 18, 5) + 96_000, status: 'success', runType: 'rekey', profilesProcessed: 0, newVisits: 0, newUrls: 0, space: 'browsing' },
  { id: 410, startedAt: at(1, 18, 0), finishedAt: at(1, 18, 0) + 14_600, status: 'success', runType: 'doctor', profilesProcessed: 0, newVisits: 0, newUrls: 0, space: 'browsing' },
  { id: 409, startedAt: at(2, 3, 0), finishedAt: at(2, 3, 0) + 22_000, status: 'success', runType: 'retention_prune', profilesProcessed: 0, newVisits: 0, newUrls: 0, space: 'browsing' },
  { id: 408, startedAt: at(2, 1, 10), finishedAt: at(2, 1, 10) + 61_000, status: 'interrupted', runType: 'import', profilesProcessed: 1, newVisits: 10_200, newUrls: 4_018, space: 'browsing' },
  { id: 407, startedAt: at(3, 8, 0), finishedAt: at(3, 8, 0) + 5_200, status: 'failed', runType: 'backup', profilesProcessed: 0, newVisits: 0, newUrls: 0, errorMessage: 'database is locked', space: 'browsing' },
  { id: 406, startedAt: at(3, 7, 0), finishedAt: at(3, 7, 0) + 900, status: 'failed', runType: 'backup', profilesProcessed: 0, newVisits: 0, newUrls: 0, space: 'browsing' },
  { id: 405, startedAt: at(5, 20, 12), finishedAt: at(5, 20, 12) + 48_000, status: 'success', runType: 'snapshot_restore', profilesProcessed: 0, newVisits: 2_340, newUrls: 610, space: 'browsing' },
  { id: 404, startedAt: at(6, 10, 30), finishedAt: at(6, 10, 30) + 210_000, status: 'success', runType: 'archive_restore', profilesProcessed: 0, newVisits: 0, newUrls: 0, space: 'browsing' },
  { id: 403, startedAt: at(8, 4, 0), finishedAt: at(8, 4, 0) + 7_700, status: 'success', runType: 'same-visit', profilesProcessed: 0, newVisits: 0, newUrls: 0, space: 'browsing' },
]

/** While a backup runs, the snapshot lists it first; its audit detail has nothing yet. */
export const runningRun: Run = {
  id: 419,
  startedAt: NOW - 40_000,
  finishedAt: null,
  status: 'running',
  runType: 'backup',
  profilesProcessed: 2,
  newVisits: 0,
  newUrls: 0,
  space: 'browsing',
}

/** The same run once it ends: Safari still has no Full Disk Access, so it is partial. */
export const finishedRun: Run = {
  ...runningRun,
  finishedAt: NOW + 12_000,
  status: 'partial',
  profilesProcessed: 7,
  newVisits: 1_284,
  newUrls: 312,
}

export const runningDetail: RunDetail = { trigger: 'manual', dueOnly: false, warnings: [], artifacts: [] }

const runStart = (id: number) => allRuns.find((r) => r.id === id)!.startedAt

const checkpoint = (id: number, offset: number): Artifact => ({
  path: `${ARCHIVE}/checkpoints/chrome-Default-${id}.sqlite`,
  createdAt: runStart(id) + offset,
  sizeBytes: 187_400_000,
  reason: 'periodic-checkpoint',
})

export const details: Record<number, RunDetail> = {
  418: {
    trigger: 'schedule',
    timezone: 'Asia/Taipei',
    dueOnly: true,
    manifestPath: manifest(418),
    manifestHash: 'sha256:4f1c9a0e7b2d58c3a61e09fd2b7c4e1a8d0f3b6c9e2a5d7f1b4c8e0a3d6f9b2c',
    warnings: [
      { code: 'profile-search-terms-captured', message: 'Captured 37 search terms from Google Chrome 個人', count: 37, profileId: 'chrome:Default' },
      { code: 'og-prefetch-summary', message: 'Fetched link previews for 288 of 312 new pages', count: 312, succeeded: 288 },
    ],
    artifacts: [checkpoint(418, 30_000)],
  },
  416: {
    trigger: 'schedule',
    timezone: 'Asia/Taipei',
    dueOnly: true,
    manifestPath: manifest(416),
    manifestHash: 'sha256:9b0e3d7a1c5f28e46b0d9c3a7e1f5b2d8c4a6e0f3b9d1c7a5e2f8b4d0c6a3e9f',
    warnings: [
      {
        code: 'source-permission-denied',
        message: 'Skipped Safari Default: permission denied',
        profileId: 'safari:default',
        diagnostic: 'Operation not permitted (os error 1): ~/Library/Safari/History.db',
      },
      {
        code: 'staging-fallback-recovered-copy',
        message: 'Google Chrome 工作 was in use; read a recovered copy',
        profileId: 'chrome:Profile 1',
        diagnostic: 'SQLITE_BUSY on live file; recovered copy from WAL (3 frames)',
      },
      {
        code: 'ai-autoindex-provider-not-ready',
        message: 'Semantic index provider is not ready',
        diagnostic: 'embedding provider http://127.0.0.1:1234/v1 refused the connection',
      },
      { code: 'git-history-skipped', message: 'Run recorded; git commit skipped', diagnostic: 'git: index.lock exists' },
      // An entry from before codes: a lead-in, then the recorded text.
      { code: '', message: 'Favicons database for Arc Default was empty; nothing to copy.' },
      { code: 'source-visits-without-page', message: '14 visits without a page row', count: 14, profileId: 'chrome:Default' },
      { code: 'ai-autoindex-queued-while-paused', message: 'Queued 240 pages while background work is paused' },
    ],
    artifacts: [
      { path: `${ARCHIVE}/checkpoints/arc-Default-416.sqlite`, createdAt: runStart(416) + 60_000, sizeBytes: 41_900_000, reason: 'source-schema-changed' },
    ],
  },
  415: {
    trigger: 'manual',
    dueOnly: false,
    manifestPath: manifest(415),
    warnings: [
      { code: 'skipped-missing-timestamp', message: 'Skipped 37 records without a time', count: 37 },
      { code: 'parse-failed', message: 'Takeout/Chrome/BrowserHistory-2.json could not be parsed' },
      { code: 'parser-unknown-field', message: 'Unknown field "client_id" in BrowserHistory.json', diagnostic: 'unknown field `client_id` at line 1 column 48211' },
    ],
    artifacts: [],
  },
  414: { trigger: 'manual', dueOnly: false, warnings: [], artifacts: [] },
  412: {
    trigger: 'manual',
    dueOnly: false,
    warnings: [{ code: 'batch-reverted', message: 'Hid 3120 visits of this import', count: 3_120 }],
    artifacts: [],
  },
  413: {
    trigger: 'manual',
    dueOnly: false,
    warnings: [{ code: 'batch-restored', message: 'Showed 3120 visits of this import again', count: 3_120 }],
    artifacts: [],
  },
  411: {
    trigger: 'manual',
    dueOnly: false,
    manifestPath: manifest(411),
    warnings: [],
    artifacts: [
      { path: `${ARCHIVE}/safety/before-rekey-2026-11-12.sqlite`, createdAt: runStart(411) + 2_000, sizeBytes: 2_310_000_000, reason: 'before-rekey' },
    ],
  },
  410: { trigger: 'cli', dueOnly: false, warnings: [], artifacts: [] },
  409: {
    trigger: 'schedule',
    timezone: 'Asia/Taipei',
    dueOnly: true,
    warnings: [{ code: 'og-cleanup-summary', message: 'Removed 1840 old link preview rows', count: 1_840, bytes: 96_500_000 }],
    artifacts: [],
  },
  408: { trigger: 'manual', dueOnly: false, warnings: [], artifacts: [] },
  407: {
    trigger: 'schedule',
    timezone: 'Asia/Taipei',
    dueOnly: true,
    errorMessage: 'database is locked\n  while opening archive.sqlite for writing (another PathKeep was writing)',
    warnings: [],
    artifacts: [],
  },
  406: { trigger: 'manual', dueOnly: false, warnings: [], artifacts: [] },
  405: {
    trigger: 'manual',
    dueOnly: false,
    manifestPath: manifest(405),
    warnings: [],
    artifacts: [{ path: `${ARCHIVE}/snapshots/2026-11-01.sqlite`, createdAt: runStart(405), sizeBytes: 2_090_000_000 }],
  },
  404: { trigger: 'manual', dueOnly: false, warnings: [], artifacts: [] },
  403: { trigger: 'app', dueOnly: false, warnings: [{ code: 'source-ids-reset', message: 'Re-checked all readable history', profileId: 'arc:Default' }], artifacts: [] },
  419: {
    trigger: 'manual',
    dueOnly: false,
    manifestPath: manifest(419),
    warnings: [
      {
        code: 'source-permission-denied',
        message: 'Skipped Safari Default: permission denied',
        profileId: 'safari:default',
        diagnostic: 'Operation not permitted (os error 1): ~/Library/Safari/History.db',
      },
    ],
    artifacts: [],
  },
}
