/**
 * @file backend-preview-showcase-fixtures.ts
 * @description Synthetic browser-preview showcase dataset used by public static deployments.
 * @module lib/backend-preview-showcase-fixtures
 *
 * ## Responsibilities
 * - Generate bounded, deterministic-looking sample archive rows for browser-preview showcase mode.
 * - Shape Dashboard, Explorer, and Core Intelligence fixtures from public synthetic domains only.
 * - Keep Vercel/static preview data separate from the local desktop archive and Tauri command path.
 *
 * ## Not responsible for
 * - Reading real browser history, archive SQLite files, local paths, or secret material.
 * - Replacing the default setup/onboarding fixture used by local browser-preview tests.
 * - Proving desktop behavior; browser-preview remains a fixture-backed smoke surface.
 *
 * ## Dependencies
 * - Depends on frontend archive and Core Intelligence contracts from `./types` and `./core-intelligence`.
 * - Uses a build-time dataset define from Vite / Vercel config, with local fallback to setup mode.
 *
 * ## Performance notes
 * - The showcase sample is intentionally small and generated in memory; modeled totals communicate archive scale without rendering unbounded rows.
 */

import type {
  AppSnapshot,
  BrowserProfile,
  HistoryQueryResponse,
  IntelligenceRuntimeSnapshot,
} from './types'

declare const __PATHKEEP_BROWSER_PREVIEW_DATASET__: string | undefined

type PreviewDatasetMode = 'setup' | 'showcase'
type HistoryItem = HistoryQueryResponse['items'][number]

export interface PreviewShowcaseTotals {
  modeledTotalVisits: number
  modeledTotalUrls: number
  modeledProfiles: number
  modeledSearches: number
  sampledRows: number
}

export interface ShowcaseSite {
  domain: string
  category: string
  displayName: string
  paths: string[]
  titles: string[]
}

export const SHOWCASE_PROFILE_IDS = [
  'chrome:Default',
  'comet:Default',
  'safari:default',
  'takeout:chrome-2025',
] as const

const SHOWCASE_PROFILE_PATTERN = [
  'chrome:Default',
  'chrome:Default',
  'comet:Default',
  'safari:default',
  'chrome:Default',
  'takeout:chrome-2025',
] as const

export const SHOWCASE_ACTIVE_HOURS = [
  16, 17, 23, 22, 0, 18, 15, 21, 14, 10, 11, 20,
]

const SHOWCASE_MODEL: PreviewShowcaseTotals = {
  modeledTotalVisits: 348_000,
  modeledTotalUrls: 172_000,
  modeledProfiles: 4,
  modeledSearches: 1_700,
  sampledRows: 168,
}

export const SHOWCASE_SITES: ShowcaseSite[] = [
  {
    domain: 'github.com',
    category: 'developer',
    displayName: 'GitHub',
    paths: [
      '/tauri-apps/tauri',
      '/oven-sh/bun',
      '/sqlite/sqlite',
      '/vercel/next.js/discussions',
    ],
    titles: [
      'Tauri desktop runtime notes',
      'Bun test runner issue review',
      'SQLite write-ahead log reference',
      'Static deployment discussion',
    ],
  },
  {
    domain: 'tauri.app',
    category: 'docs',
    displayName: 'Tauri',
    paths: ['/start/', '/plugin/window-state/', '/security/csp/', '/develop/'],
    titles: [
      'Tauri v2 getting started',
      'Window state plugin documentation',
      'Content security policy guide',
      'Tauri desktop development workflow',
    ],
  },
  {
    domain: 'sqlite.org',
    category: 'docs',
    displayName: 'SQLite',
    paths: ['/wal.html', '/lang_datefunc.html', '/queryplanner.html'],
    titles: [
      'Write-ahead logging',
      'Date and time functions',
      'Query planner overview',
    ],
  },
  {
    domain: 'react.dev',
    category: 'docs',
    displayName: 'React',
    paths: ['/reference/react/useMemo', '/learn', '/reference/react/Suspense'],
    titles: ['useMemo reference', 'React learning path', 'Suspense reference'],
  },
  {
    domain: 'developer.mozilla.org',
    category: 'docs',
    displayName: 'MDN',
    paths: [
      '/en-US/docs/Web/API/URL',
      '/en-US/docs/Web/CSS/grid-template-columns',
    ],
    titles: ['URL API reference', 'CSS grid template columns'],
  },
  {
    domain: 'vercel.com',
    category: 'developer',
    displayName: 'Vercel',
    paths: [
      '/docs/deployments/overview',
      '/docs/projects/project-configuration',
    ],
    titles: ['Deployment overview', 'Project configuration reference'],
  },
  {
    domain: 'docs.rs',
    category: 'docs',
    displayName: 'Docs.rs',
    paths: ['/rusqlite/latest/rusqlite/', '/time/latest/time/'],
    titles: ['rusqlite crate documentation', 'time crate documentation'],
  },
  {
    domain: 'perplexity.ai',
    category: 'ai',
    displayName: 'Perplexity',
    paths: [
      '/search/local-first-browser-history',
      '/search/sqlite-browser-profile',
    ],
    titles: [
      'Local-first browser history research',
      'SQLite browser profile notes',
    ],
  },
  {
    domain: 'news.ycombinator.com',
    category: 'community',
    displayName: 'Hacker News',
    paths: ['/item?id=preview001', '/item?id=preview002'],
    titles: [
      'Discussion: local-first apps',
      'Discussion: desktop app packaging',
    ],
  },
  {
    domain: 'lobste.rs',
    category: 'community',
    displayName: 'Lobsters',
    paths: ['/s/pathkeep/local_first_archives', '/s/rust/sqlite_at_scale'],
    titles: ['Local-first archive discussion', 'SQLite at scale thread'],
  },
  {
    domain: 'youtube.com',
    category: 'video',
    displayName: 'YouTube',
    paths: ['/watch?v=sqlite-preview', '/watch?v=tauri-preview'],
    titles: ['SQLite indexing walkthrough', 'Tauri desktop app walkthrough'],
  },
  {
    domain: 'google.com',
    category: 'search',
    displayName: 'Google',
    paths: [
      '/search?q=tauri+sqlite+local+first',
      '/search?q=browser+history+sqlite',
    ],
    titles: [
      'Search results for tauri sqlite local first',
      'Search results for browser history sqlite',
    ],
  },
]

export function configuredPreviewDataset(): PreviewDatasetMode {
  const configured =
    typeof __PATHKEEP_BROWSER_PREVIEW_DATASET__ === 'string'
      ? __PATHKEEP_BROWSER_PREVIEW_DATASET__
      : 'setup'
  return configured === 'showcase' ? 'showcase' : 'setup'
}

export function isShowcasePreviewDataset() {
  return configuredPreviewDataset() === 'showcase'
}

export function showcaseTotals(): PreviewShowcaseTotals {
  return { ...SHOWCASE_MODEL }
}

export function nowMs() {
  return Date.now()
}

export function isoAt(ms: number) {
  return new Date(ms).toISOString()
}

export function localDateKey(ms: number) {
  return isoAt(ms).slice(0, 10)
}

function shiftedVisitTime(index: number) {
  const dayOffset = Math.floor(index * 2.2)
  const date = new Date(nowMs() - dayOffset * 86_400_000)
  const hour = SHOWCASE_ACTIVE_HOURS[index % SHOWCASE_ACTIVE_HOURS.length] ?? 16
  date.setHours(hour, (index * 13) % 60, (index * 7) % 60, 0)
  return date.getTime()
}

function faviconDataUrl(site: ShowcaseSite, index: number) {
  const bg = ['0f172a', '111827', '1f2937', '27272a'][index % 4]
  const fg = ['38bdf8', 'fb923c', 'a78bfa', '4ade80'][index % 4]
  const initial = encodeURIComponent(site.displayName.slice(0, 1).toUpperCase())
  return `data:image/svg+xml;utf8,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 32 32%22%3E%3Crect width=%2232%22 height=%2232%22 rx=%226%22 fill=%22%23${bg}%22/%3E%3Ctext x=%2216%22 y=%2221%22 text-anchor=%22middle%22 font-size=%2215%22 font-family=%22Arial%22 font-weight=%22700%22 fill=%22%23${fg}%22%3E${initial}%3C/text%3E%3C/svg%3E`
}

export function buildShowcaseHistory(): HistoryQueryResponse {
  const items = Array.from(
    { length: SHOWCASE_MODEL.sampledRows },
    (_, index) => {
      const site = SHOWCASE_SITES[index % SHOWCASE_SITES.length]
      const path =
        site.paths[
          Math.floor(index / SHOWCASE_SITES.length) % site.paths.length
        ]
      const title = site.titles[index % site.titles.length]
      const visitTime = shiftedVisitTime(index)
      const url = `https://${site.domain}${path}`

      return {
        id: index + 1,
        profileId:
          SHOWCASE_PROFILE_PATTERN[index % SHOWCASE_PROFILE_PATTERN.length],
        url,
        title,
        domain: site.domain,
        favicon: { dataUrl: faviconDataUrl(site, index) },
        visitedAt: isoAt(visitTime),
        visitTime,
        durationMs: 12_000 + (index % 9) * 8_000,
        transition: index % 6 === 0 ? 805_306_368 : 0,
        sourceVisitId: 10_000 + index,
        appId: null,
      } satisfies HistoryItem
    },
  ).sort((left, right) => right.visitTime - left.visitTime)

  return {
    total: items.length,
    page: 1,
    pageSize: 50,
    pageCount: Math.ceil(items.length / 50),
    hasPrevious: false,
    hasNext: items.length > 50,
    items,
    nextCursor: null,
  }
}

function showcaseProfiles(): BrowserProfile[] {
  return [
    {
      profileId: 'chrome:Default',
      profileName: 'Primary',
      browserFamily: 'chromium',
      browserName: 'Google Chrome',
      userName: null,
      profilePath: 'showcase://profiles/chromium/primary',
      historyPath: 'showcase://sources/chromium/primary/history',
      faviconsPath: 'showcase://sources/chromium/primary/favicons',
      historyExists: true,
      browserVersion: '146.0.7680.178',
      historyFileName: 'History',
      historyBytes: 192 * 1024 * 1024,
      faviconsBytes: 26 * 1024 * 1024,
      supportingBytes: 14 * 1024 * 1024,
      retentionBoundary: { kind: 'browser-managed', localDays: null },
    },
    {
      profileId: 'comet:Default',
      profileName: 'Research',
      browserFamily: 'chromium',
      browserName: 'Perplexity Comet',
      userName: null,
      profilePath: 'showcase://profiles/comet/research',
      historyPath: 'showcase://sources/comet/research/history',
      faviconsPath: 'showcase://sources/comet/research/favicons',
      historyExists: true,
      browserVersion: '146.0.7680.178',
      historyFileName: 'History',
      historyBytes: 88 * 1024 * 1024,
      faviconsBytes: 11 * 1024 * 1024,
      supportingBytes: 8 * 1024 * 1024,
      retentionBoundary: { kind: 'browser-managed', localDays: null },
    },
    {
      profileId: 'safari:default',
      profileName: 'Safari',
      browserFamily: 'safari',
      browserName: 'Safari',
      userName: null,
      profilePath: 'showcase://profiles/safari/default',
      historyPath: 'showcase://sources/safari/default/history',
      faviconsPath: null,
      historyExists: true,
      historyReadable: true,
      browserVersion: '18.4',
      historyFileName: 'History.db',
      historyBytes: 34 * 1024 * 1024,
      faviconsBytes: 0,
      supportingBytes: 4 * 1024 * 1024,
      retentionBoundary: { kind: 'macos-safari', localDays: 365 },
    },
    {
      profileId: 'takeout:chrome-2025',
      profileName: 'Google Takeout recovery',
      browserFamily: 'takeout',
      browserName: 'Google Takeout',
      userName: null,
      profilePath: 'showcase://profiles/takeout/chrome-2025',
      historyPath: 'showcase://sources/takeout/chrome-2025/history-json',
      faviconsPath: null,
      historyExists: true,
      browserVersion: null,
      historyFileName: 'BrowserHistory.json',
      historyBytes: 124 * 1024 * 1024,
      faviconsBytes: 0,
      supportingBytes: 2 * 1024 * 1024,
      retentionBoundary: { kind: 'browser-managed', localDays: null },
    },
  ]
}

export function buildShowcaseSnapshot(base: AppSnapshot): AppSnapshot {
  const finishedAt = isoAt(nowMs() - 45 * 60_000)
  const earlierAt = isoAt(nowMs() - 26 * 60 * 60_000)
  return {
    ...structuredClone(base),
    config: {
      ...base.config,
      initialized: true,
      selectedProfileIds: [...SHOWCASE_PROFILE_IDS],
      dueAfterHours: 12,
      checkpointDays: 90,
      rememberDatabaseKeyInKeyring: false,
    },
    archiveStatus: {
      ...base.archiveStatus,
      initialized: true,
      unlocked: true,
      lastSuccessfulBackupAt: finishedAt,
      warning: null,
    },
    intelligenceStatus: {
      ready: true,
      lastRunAt: finishedAt,
      runs: 39,
      cards: 12,
      topics: 18,
      threads: 42,
      queryGroups: 28,
      referencePages: 64,
      contentCoverage: 0.72,
      warning: null,
    },
    browserProfiles: showcaseProfiles(),
    recentRuns: [
      {
        id: 2042,
        startedAt: finishedAt,
        finishedAt,
        status: 'success',
        runType: 'backup',
        trigger: 'scheduled',
        profileScope: [...SHOWCASE_PROFILE_IDS],
        manifestHash: 'preview-showcase-manifest-2042',
        profilesProcessed: SHOWCASE_PROFILE_IDS.length,
        newVisits: 6_040,
        newUrls: 2_480,
        newDownloads: 0,
      },
      {
        id: 2041,
        startedAt: earlierAt,
        finishedAt: isoAt(nowMs() - 25 * 60 * 60_000),
        status: 'success',
        runType: 'import',
        trigger: 'manual',
        profileScope: ['takeout:chrome-2025'],
        manifestHash: 'preview-showcase-manifest-2041',
        profilesProcessed: 1,
        newVisits: 18_400,
        newUrls: 9_600,
        newDownloads: 0,
      },
      {
        id: 2040,
        startedAt: isoAt(nowMs() - 3 * 86_400_000),
        finishedAt: isoAt(nowMs() - 3 * 86_400_000 + 8 * 60_000),
        status: 'success',
        runType: 'backup',
        trigger: 'scheduled',
        profileScope: ['chrome:Default', 'comet:Default', 'safari:default'],
        manifestHash: 'preview-showcase-manifest-2040',
        profilesProcessed: 3,
        newVisits: 5_820,
        newUrls: 2_210,
        newDownloads: 1,
      },
    ],
  }
}

export function buildShowcaseRuntime(
  base: IntelligenceRuntimeSnapshot,
): IntelligenceRuntimeSnapshot {
  const generatedAt = isoAt(nowMs() - 50 * 60_000)
  return {
    ...structuredClone(base),
    queue: {
      queued: 0,
      running: 1,
      succeeded: 38,
      failed: 0,
      cancelled: 0,
      lastActivityAt: generatedAt,
    },
    plugins: base.plugins.map((plugin) =>
      plugin.pluginId === 'title-normalization'
        ? {
            ...plugin,
            enabled: true,
            storedRecords: 28_400,
            lastCompletedAt: generatedAt,
            lastError: null,
          }
        : {
            ...plugin,
            enabled: false,
            storedRecords: 0,
            queuedJobs: 0,
            runningJobs: 0,
            failedJobs: 0,
            lastCompletedAt: null,
            lastError: null,
          },
    ),
    modules: base.modules.map((module, index) => ({
      ...module,
      status: 'ready',
      lastRunId: 2042,
      lastBuiltAt: generatedAt,
      lastInvalidatedAt: null,
      staleReason: null,
      notes: [
        `Showcase deterministic module ${index + 1} is current for the synthetic preview archive.`,
      ],
    })),
    recentJobs: [
      {
        id: 2042,
        jobType: 'deterministic-rebuild',
        pluginId: null,
        state: 'running',
        historyId: null,
        profileId: null,
        url: null,
        title: 'All profiles · deterministic analysis',
        attempt: 1,
        createdAt: isoAt(nowMs() - 52 * 60_000),
        startedAt: isoAt(nowMs() - 51 * 60_000),
        finishedAt: null,
        updatedAt: generatedAt,
        heartbeatAt: generatedAt,
        progressLabel: 'daily-rollups',
        progressDetail:
          'Refreshing daily rollups for the synthetic preview archive.',
        progressCurrent: 274_000,
        progressTotal: SHOWCASE_MODEL.modeledTotalVisits,
        progressPercent: 79,
        lastError: null,
        retryable: false,
        cancellable: true,
      },
      {
        id: 2041,
        jobType: 'enrichment-plugin',
        pluginId: 'title-normalization',
        state: 'succeeded',
        historyId: 12,
        profileId: 'chrome:Default',
        url: 'https://sqlite.org/wal.html',
        title: 'Write-ahead logging',
        attempt: 1,
        createdAt: isoAt(nowMs() - 2 * 60 * 60_000),
        startedAt: isoAt(nowMs() - 2 * 60 * 60_000 + 30_000),
        finishedAt: isoAt(nowMs() - 2 * 60 * 60_000 + 90_000),
        updatedAt: isoAt(nowMs() - 2 * 60 * 60_000 + 90_000),
        heartbeatAt: null,
        progressLabel: null,
        progressDetail: null,
        progressCurrent: null,
        progressTotal: null,
        progressPercent: null,
        lastError: null,
        retryable: false,
        cancellable: false,
      },
    ],
    notes: ['Browser preview showcase uses synthetic local-only fixture data.'],
  }
}
