/**
 * @file localize-task.test.ts
 * @description Behavioral contract for backend progress-code localization on task cards and consoles.
 * @module components/progress
 *
 * These tests use the REAL translator over the shipped catalog (not a stub), so
 * a missing/renamed `jobs.*` key fails here instead of shipping the raw English
 * transport prose to a zh-CN / zh-TW user.
 */

import { describe, expect, test } from 'vitest'
import { createTranslator } from '../../lib/i18n/catalog'
import type {
  ShellTask,
  ShellTaskKind,
  ShellTaskLogEntry,
  ShellTaskState,
} from '../../app/shell-tasks'
import {
  localizeShellTaskForDisplay,
  localizedTaskDetail,
  localizedTaskLogMessage,
  taskKindLabel,
  taskStateLabel,
} from './localize-task'

const en = createTranslator('en')
const zhTW = createTranslator('zh-TW')

function makeTask(overrides: Partial<ShellTask> = {}): ShellTask {
  return {
    id: 'task-1',
    kind: 'backup',
    state: 'running',
    title: 'Backup',
    detail: 'Copying browser history into the staging area.',
    detailOrigin: 'backend',
    phase: 'stage-profile',
    startedAt: '2026-04-27T10:00:00.000Z',
    updatedAt: '2026-04-27T10:00:01.000Z',
    logEntries: [],
    ...overrides,
  }
}

function makeEntry(
  overrides: Partial<ShellTaskLogEntry> = {},
): ShellTaskLogEntry {
  return {
    id: 'entry-1',
    timestamp: '2026-04-27T10:00:00.000Z',
    level: 'info',
    code: 'backup.prepare',
    message: 'Inspecting the selected browser profiles.',
    origin: 'backend',
    ...overrides,
  }
}

describe('localizedTaskDetail', () => {
  test('replaces backend transport prose with phase-keyed catalog copy', () => {
    const task = makeTask({
      kind: 'backup',
      phase: 'ingest-profile',
      detail: 'BACKEND PROSE THAT MUST NOT REACH THE UI',
    })

    expect(localizedTaskDetail(task, en)).toBe(
      'Writing staged history into the archive.',
    )
    expect(localizedTaskDetail(task, zhTW)).toBe('正在把暫存歷史寫入封存。')
  })

  test('localizes every shipped import phase', () => {
    for (const phase of [
      'prepare',
      'import-file',
      'finalize',
      'complete',
      'unexpected-phase',
    ]) {
      const task = makeTask({ kind: 'import', phase, detail: 'raw' })
      expect(localizedTaskDetail(task, en)).toBe(
        en(`jobs.phase.import.${phase}`),
      )
      expect(localizedTaskDetail(task, en)).not.toBe('raw')
    }
  })

  test('keeps shell-authored detail verbatim (already localized)', () => {
    const task = makeTask({
      detailOrigin: 'shell',
      phase: 'ingest-profile',
      detail: '已在寫入封存。',
    })

    expect(localizedTaskDetail(task, en)).toBe('已在寫入封存。')
  })

  test('falls back to the raw backend detail for phases the catalog does not know', () => {
    // Unknown phase for a known kind: degrade visibly, never render a raw key.
    expect(
      localizedTaskDetail(
        makeTask({ kind: 'backup', phase: 'teleport', detail: 'Teleporting.' }),
        en,
      ),
    ).toBe('Teleporting.')

    // Known kind, no phase reported yet.
    expect(
      localizedTaskDetail(
        makeTask({ kind: 'backup', phase: null, detail: 'Starting up.' }),
        en,
      ),
    ).toBe('Starting up.')

    // Kind with no phase catalog at all (`runtime` jobs narrate themselves).
    expect(
      localizedTaskDetail(
        makeTask({ kind: 'runtime', phase: 'prepare', detail: 'Draining.' }),
        en,
      ),
    ).toBe('Draining.')
  })
})

describe('localizedTaskLogMessage', () => {
  test('replaces backend narration with code-keyed catalog copy', () => {
    const entry = makeEntry({
      code: 'backup.stage-profile.fallback',
      message: 'BACKEND PROSE THAT MUST NOT REACH THE UI',
    })

    expect(localizedTaskLogMessage(entry, en)).toBe(
      'Staging fell back to an alternate copy strategy for this profile.',
    )
    expect(localizedTaskLogMessage(entry, zhTW)).toBe(
      '該設定檔改用備選複製策略完成暫存。',
    )
  })

  test('localizes every shipped backend log code', () => {
    const codes = [
      'backup.prepare',
      'backup.stage-profile',
      'backup.stage-profile.fallback',
      'backup.stage-profile.skip',
      'backup.ingest-profile',
      'backup.finalize',
      'import.prepare',
      'import.import-file',
      'import.finalize',
      'import.complete',
      'import.unexpected-phase',
    ]

    for (const code of codes) {
      const message = localizedTaskLogMessage(
        makeEntry({ code, message: 'raw' }),
        en,
      )
      expect(message).toBe(en(`jobs.log.${code}`))
      expect(message).not.toBe('raw')
      expect(message).not.toContain('jobs.log')
    }
  })

  test('interpolates the record counter with locale thousands separators', () => {
    const entry = makeEntry({
      code: 'backup.ingest-profile.records',
      message: 'Processed 1234567 records so far.',
      current: 1_234_567,
    })

    expect(localizedTaskLogMessage(entry, en)).toBe(
      'Processed 1,234,567 records so far.',
    )
    expect(localizedTaskLogMessage(entry, zhTW)).toBe(
      '目前已處理 1,234,567 筆紀錄。',
    )
  })

  test('uses the singular record line when exactly one record is done', () => {
    const entry = makeEntry({
      code: 'backup.ingest-profile.records',
      message: 'Processed 1 records so far.',
      current: 1,
    })

    // Plural pair, not "record(s)" — the singular line drops the count.
    expect(localizedTaskLogMessage(entry, en)).toBe(
      'Processed 1 record so far.',
    )
  })

  test('renders a zero counter when the backend omitted `current`', () => {
    const entry = makeEntry({
      code: 'backup.ingest-profile.records',
      message: 'Processed record(s).',
      current: null,
    })

    expect(localizedTaskLogMessage(entry, en)).toBe(
      'Processed 0 records so far.',
    )
  })

  test('keeps shell-authored entries verbatim (already localized)', () => {
    const entry = makeEntry({
      origin: 'shell',
      code: 'backup.prepare',
      message: '正在檢查設定檔。',
    })

    expect(localizedTaskLogMessage(entry, en)).toBe('正在檢查設定檔。')
  })

  test('falls back to the raw backend message for unknown codes', () => {
    const entry = makeEntry({
      code: 'backup.quantum-tunnel',
      message: 'Quantum tunneling the profile.',
    })

    expect(localizedTaskLogMessage(entry, en)).toBe(
      'Quantum tunneling the profile.',
    )
  })
})

describe('localizeShellTaskForDisplay', () => {
  test('localizes the detail and every console entry while preserving task identity', () => {
    const task = makeTask({
      kind: 'backup',
      phase: 'finalize',
      detail: 'RAW DETAIL',
      progressLabel: '3 / 4',
      logEntries: [
        makeEntry({ id: 'a', code: 'backup.prepare', message: 'RAW A' }),
        makeEntry({
          id: 'b',
          code: 'backup.ingest-profile.records',
          message: 'RAW B',
          current: 2_500,
          sourceLabel: 'Chrome Default',
        }),
        makeEntry({
          id: 'c',
          origin: 'shell',
          code: 'shell.local',
          message: 'Shell line stays.',
        }),
      ],
    })

    const localized = localizeShellTaskForDisplay(task, en)

    expect(localized.detail).toBe('Finalizing the manifest and run ledger.')
    expect(localized.logEntries.map((entry) => entry.message)).toEqual([
      'Inspecting the selected browser profiles.',
      'Processed 2,500 records so far.',
      'Shell line stays.',
    ])
    // Identity and non-message fields survive the copy.
    expect(localized.id).toBe('task-1')
    expect(localized.progressLabel).toBe('3 / 4')
    expect(localized.logEntries[1]?.sourceLabel).toBe('Chrome Default')
    // The source task is not mutated — renderers may still read the raw ledger.
    expect(task.detail).toBe('RAW DETAIL')
    expect(task.logEntries[0]?.message).toBe('RAW A')
  })
})

describe('task vocabulary labels', () => {
  test('localizes every task kind', () => {
    const expected: Record<ShellTaskKind, string> = {
      import: 'Import',
      backup: 'Backup',
      runtime: 'Background job',
    }
    for (const [kind, label] of Object.entries(expected)) {
      expect(taskKindLabel(kind as ShellTaskKind, en)).toBe(label)
    }
    expect(taskKindLabel('backup', zhTW)).toBe('備份')
  })

  test('localizes every task state', () => {
    const expected: Record<ShellTaskState, string> = {
      queued: 'Queued',
      running: 'Running',
      succeeded: 'Succeeded',
      failed: 'Failed',
      stale: 'Interrupted',
    }
    for (const [state, label] of Object.entries(expected)) {
      expect(taskStateLabel(state as ShellTaskState, en)).toBe(label)
    }
    expect(taskStateLabel('stale', zhTW)).toBe('已中斷')
  })
})
