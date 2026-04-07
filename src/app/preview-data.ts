export interface DashboardStat {
  label: string
  value: string
  detail: string
  tone: 'accent' | 'neutral' | 'success'
}

export interface RecentRun {
  id: string
  type: string
  source: string
  records: string
  status: string
  time: string
}

export interface QueueItem {
  id: string
  title: string
  state: 'ready' | 'blocked' | 'running'
  detail: string
}

export interface InsightHighlight {
  title: string
  source: string
}

export const shellStatus = {
  archiveHealth: 'Archive healthy',
  runtime: 'SQLite 3.46',
  archiveSize: '12.4 GB',
  version: 'v0.1.0-alpha',
}

export const dashboardStats: DashboardStat[] = [
  {
    label: 'TOTAL RECORDS',
    value: '2,847,391',
    detail: '+12,847 since last run',
    tone: 'accent',
  },
  {
    label: 'ARCHIVE SPAN',
    value: '4y 7m',
    detail: '2021-09-14 -> today',
    tone: 'neutral',
  },
  {
    label: 'PROFILES TRACKED',
    value: '3',
    detail: 'Chrome · Arc · Firefox',
    tone: 'neutral',
  },
  {
    label: 'LAST BACKUP',
    value: '2h ago',
    detail: 'Run #1847 · completed',
    tone: 'success',
  },
]

export const recentRuns: RecentRun[] = [
  {
    id: '#1847',
    type: 'BACKUP',
    source: 'Chrome / Default',
    records: '+12,847',
    status: 'COMPLETED',
    time: '2h ago',
  },
  {
    id: '#1846',
    type: 'BACKUP',
    source: 'Arc / Work',
    records: '+3,291',
    status: 'COMPLETED',
    time: '2h ago',
  },
  {
    id: '#1845',
    type: 'IMPORT',
    source: 'Google Takeout',
    records: '+89,412',
    status: 'COMPLETED',
    time: '1d ago',
  },
]

export const queueItems: QueueItem[] = [
  {
    id: 'job-019',
    title: 'Semantic index rebuild',
    state: 'running',
    detail: 'AI optional pipeline is re-indexing the latest Chrome run.',
  },
  {
    id: 'job-020',
    title: 'Weekly archive verification',
    state: 'ready',
    detail: 'Queued after the index rebuild finishes.',
  },
  {
    id: 'job-021',
    title: 'Remote backup upload',
    state: 'blocked',
    detail: 'Waiting for S3 credentials and the next manual approval.',
  },
]

export const insightHighlights: InsightHighlight[] = [
  {
    title: 'Understanding Gaussian Splatting — A Visual Guide',
    source: 'medium.com/@neural3d/gaussian-splatting-guide',
  },
  {
    title: '3D Gaussian Splatting for Real-Time Rendering',
    source: 'arxiv.org/abs/2308.14737',
  },
  {
    title: 'tauri-apps/tauri: Build desktop apps with web tech',
    source: 'github.com/tauri-apps/tauri',
  },
]

export const onboardingSteps = [
  {
    title: 'Preview',
    detail:
      'Inspect detected browser profiles, archive paths, and schedule artifacts before touching data.',
  },
  {
    title: 'Manual',
    detail:
      'If PathKeep cannot act directly, it should hand you exact commands, reasons, and verification hints.',
  },
  {
    title: 'Execute',
    detail:
      'Only run the mutating step after the intent, inputs, and rollback path are visible.',
  },
]
