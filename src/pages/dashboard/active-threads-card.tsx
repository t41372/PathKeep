/**
 * Dashboard "What you've been thinking about" active threads card.
 *
 * Static synthesis for now: the real backend mapping to query_families /
 * path_flows happens during the Intelligence route sweep. Each thread is
 * clickable and routes into /intelligence for the full threading surface.
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { useI18n } from '@/lib/i18n'
import { cn } from '@/lib/cn'

interface Thread {
  id: string
  title: string
  pages: number
  days: number
  lastTouched: string
  tone: 'hot' | 'warm' | 'cool'
}

const SAMPLE_THREADS: Thread[] = [
  {
    id: 't1',
    title: 'Building PathKeep — browser history parser',
    pages: 89,
    days: 12,
    lastTouched: 'today',
    tone: 'hot',
  },
  {
    id: 't2',
    title: 'Tokio scheduler deep dive',
    pages: 34,
    days: 3,
    lastTouched: 'today',
    tone: 'hot',
  },
  {
    id: 't3',
    title: 'SQLite FTS5 vs Tantivy comparison',
    pages: 21,
    days: 4,
    lastTouched: 'yesterday',
    tone: 'warm',
  },
  {
    id: 't4',
    title: 'Tauri 2 plugin architecture',
    pages: 18,
    days: 2,
    lastTouched: 'yesterday',
    tone: 'warm',
  },
  {
    id: 't5',
    title: 'Wavetable synthesis & Vital',
    pages: 12,
    days: 1,
    lastTouched: '3d ago',
    tone: 'cool',
  },
]

export interface DashboardActiveThreadsProps {
  onOpenAll: () => void
  onOpenThread: (threadId: string) => void
}

export function DashboardActiveThreads({
  onOpenAll,
  onOpenThread,
}: DashboardActiveThreadsProps) {
  const { t } = useI18n()
  return (
    <PaperCard testId="dashboard-active-threads">
      <PaperCardHeader
        title={t('dashboard.activeThreadsTitle')}
        right={
          <PaperCardBadge onClick={onOpenAll}>
            {t('dashboard.activeThreadsAll')} →
          </PaperCardBadge>
        }
      />
      <PaperCardBody className="px-[18px] pt-1 pb-[10px]">
        <ul className="m-0 flex list-none flex-col p-0">
          {SAMPLE_THREADS.map((thread) => (
            <li key={thread.id}>
              <button
                type="button"
                onClick={() => onOpenThread(thread.id)}
                className="group flex w-full items-center gap-3 border-b border-border-light py-2 text-left last:border-b-0 transition-colors hover:bg-hover"
              >
                <span
                  className={cn(
                    'pk-thread-pulse',
                    thread.tone === 'warm' && 'pk-thread-pulse--warm',
                    thread.tone === 'cool' && 'pk-thread-pulse--cool',
                  )}
                />
                <span className="flex flex-1 min-w-0 flex-col">
                  <span className="truncate font-serif text-[13.5px] text-ink">
                    {thread.title}
                  </span>
                  <span className="font-sans text-[11px] text-ink-faint">
                    {thread.days}d · {thread.lastTouched}
                  </span>
                </span>
                <span className="flex shrink-0 flex-col items-end gap-0 font-mono text-[11px] text-ink-faint">
                  <span className="text-ink-secondary text-[14px] font-medium">
                    {thread.pages}
                  </span>
                  <span>{t('dashboard.threadsPagesUnit')}</span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      </PaperCardBody>
    </PaperCard>
  )
}
