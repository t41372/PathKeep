/**
 * @file running-now-zone.test.tsx
 * @description Focused coverage for the Running-now zone's interruption-badge tone mapping.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Assert a running import/backup renders the honest 'restart-whole' badge with the warning tone
 *   (not the green "safe to close · resumes" label) — the cross-block integration-review fix.
 *
 * ## Not responsible for
 * - The full page wiring (covered in index.test.tsx) or the adapter mapping (activity-adapter.test).
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { ModelDownloadProgress } from '../../lib/ipc/model-download'
import { RunningNowZone } from './running-now-zone'
import type { Activity } from './activity-types'

const jobsT = createNamespaceTranslator('en', 'jobs')

const idleDownload: ModelDownloadProgress = {
  phase: 'idle',
  downloadedBytes: 0,
  totalBytes: 0,
  currentFile: null,
  error: null,
}

function renderZone(activities: Activity[]) {
  return render(
    <RunningNowZone
      activities={activities}
      modelDownload={idleDownload}
      showModelDownload={false}
      onPauseChange={() => {}}
      onCancel={() => {}}
      onCancelRuntime={() => {}}
      action={null}
      jobsT={jobsT}
      language="en"
    />,
  )
}

describe('RunningNowZone interruption badge', () => {
  test('a running import/backup shows the honest restart-whole badge with the warning tone', () => {
    const activity: Activity = {
      id: 'backup-1',
      kind: 'backup',
      state: 'running',
      taskNameKey: 'taskBackupRunning',
      timestamp: '2026-04-07T10:01:00Z',
      progress: { value: null, label: null, labelKind: null },
      resumability: 'restart-whole',
    }

    renderZone([activity])

    const badge = screen.getByText(jobsT('badgeRestartWhole'))
    expect(badge).toBeVisible()
    // It must NOT claim "safe to close · resumes" — the data is safe but the task restarts.
    expect(screen.queryByText(jobsT('badgeSafeToClose'))).toBeNull()
    expect(badge).toHaveClass('activity-row__badge--warning')
  })

  test('a fully-resumable task shows the safe badge with the calm tone', () => {
    const activity: Activity = {
      id: 'index-1',
      kind: 'index-build',
      state: 'running',
      taskNameKey: 'taskIndexBuild',
      timestamp: '2026-04-07T10:01:00Z',
      progress: { value: 0.5, label: '100', labelKind: 'embedded' },
      resumability: 'safe',
    }

    renderZone([activity])

    const badge = screen.getByText(jobsT('badgeSafeToClose'))
    expect(badge).toHaveClass('activity-row__badge--safe')
  })
})
