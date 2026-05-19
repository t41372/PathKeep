import { describe, expect, test, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import type { BackupRunOverview } from '../../lib/types'
import { PaperAuditPanel } from './paper-audit-panel'
import { paperRunTypeLabel, paperWhenLabel } from './paper-audit-helpers'

function tFor(key: string, vars?: Record<string, string | number>) {
  return vars ? `${key}:${JSON.stringify(vars)}` : key
}

function makeRun(over: Partial<BackupRunOverview> = {}): BackupRunOverview {
  return {
    id: 1,
    startedAt: '2026-05-18T10:00:00Z',
    finishedAt: '2026-05-18T10:01:00Z',
    status: 'success',
    runType: 'backup',
    trigger: 'manual',
    profileScope: [],
    manifestHash: '0a4cef82deadbeef',
    profilesProcessed: 1,
    newVisits: 100,
    newUrls: 50,
    newDownloads: 0,
    ...over,
  }
}

describe('paperRunTypeLabel', () => {
  test('maps known kinds to localised keys', () => {
    expect(paperRunTypeLabel('backup', tFor)).toBe('paperRunTypeBackup')
    expect(paperRunTypeLabel('import', tFor)).toBe('paperRunTypeImport')
    expect(paperRunTypeLabel('maintenance', tFor)).toBe(
      'paperRunTypeMaintenance',
    )
  })

  test('upper-cases unknown run types without going through the translator', () => {
    expect(paperRunTypeLabel('schedule', tFor)).toBe('SCHEDULE')
  })

  test('returns empty string when run type is undefined', () => {
    expect(paperRunTypeLabel(undefined, tFor)).toBe('')
  })
})

describe('paperWhenLabel', () => {
  const now = () => new Date('2026-05-18T12:00:00Z').getTime()

  test('renders "just now" within the first minute', () => {
    const out = paperWhenLabel('2026-05-18T11:59:30Z', tFor, now)
    expect(out).toBe('paperWhenJustNow')
  })

  test('renders minutes-ago for under-hour deltas', () => {
    const out = paperWhenLabel('2026-05-18T11:42:00Z', tFor, now)
    expect(out).toBe('paperWhenMinutesAgo:{"count":18}')
  })

  test('renders hours-ago for under-day deltas', () => {
    const out = paperWhenLabel('2026-05-18T05:00:00Z', tFor, now)
    expect(out).toBe('paperWhenHoursAgo:{"count":7}')
  })

  test('renders days-ago for older runs', () => {
    const out = paperWhenLabel('2026-05-15T10:00:00Z', tFor, now)
    expect(out).toBe('paperWhenDaysAgo:{"count":3}')
  })

  test('returns empty string for unparseable timestamps', () => {
    expect(paperWhenLabel('not-a-date', tFor, now)).toBe('')
  })

  test('defaults to Date.now when no clock override is provided', () => {
    const spy = vi.spyOn(Date, 'now').mockReturnValue(now())
    try {
      const out = paperWhenLabel('2026-05-18T11:59:30Z', tFor)
      expect(out).toBe('paperWhenJustNow')
    } finally {
      spy.mockRestore()
    }
  })
})

describe('PaperAuditPanel', () => {
  const now = () => new Date('2026-05-18T12:00:00Z').getTime()

  test('renders one chain block per recent run, current one marked, reverse-ordered', () => {
    const runs = [
      makeRun({ id: 10, manifestHash: 'aaaaaaa1' }),
      makeRun({ id: 11, manifestHash: 'bbbbbbb2', runType: 'import' }),
      makeRun({ id: 12, manifestHash: '' }),
    ]
    render(
      <PaperAuditPanel
        recentRuns={runs}
        currentRunId={11}
        onSelectRun={() => {}}
        auditT={tFor}
        now={now}
      />,
    )
    expect(screen.getByTestId('paper-audit-panel')).toBeInTheDocument()
    expect(screen.getByTestId('paper-audit-view')).toBeInTheDocument()
    // chain entries are reverse-ordered so the newest run sits on the right
    const blocks = screen.getAllByTestId(/^paper-chain-block-/)
    expect(blocks).toHaveLength(3)
    expect(blocks[0].getAttribute('data-testid')).toBe('paper-chain-block-12')
    expect(blocks[2].getAttribute('data-testid')).toBe('paper-chain-block-10')
    expect(
      blocks
        .find(
          (node) => node.getAttribute('data-testid') === 'paper-chain-block-11',
        )
        ?.getAttribute('data-current'),
    ).toBe('true')
  })

  test('clicking a chain block resolves the numeric run id', () => {
    const onSelectRun = vi.fn()
    render(
      <PaperAuditPanel
        recentRuns={[makeRun({ id: 42 })]}
        currentRunId={null}
        onSelectRun={onSelectRun}
        auditT={tFor}
        now={now}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-chain-block-42'))
    expect(onSelectRun).toHaveBeenCalledWith(42)
  })

  test('caps at six runs and renders an em-dash hash when manifestHash is missing', () => {
    const runs = Array.from({ length: 8 }, (_, index) =>
      makeRun({
        id: index + 1,
        manifestHash: index === 0 ? null : 'deadbeef',
      }),
    )
    render(
      <PaperAuditPanel
        recentRuns={runs}
        currentRunId={null}
        onSelectRun={() => {}}
        auditT={tFor}
        now={now}
      />,
    )
    expect(screen.getAllByTestId(/^paper-chain-block-/)).toHaveLength(6)
  })
})
