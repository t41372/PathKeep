/**
 * @file smart-index-status.test.tsx
 * @description Component coverage for the in-surface Smart-index status + Build CTA (REACH-B B1).
 * @module pages/explorer/panels
 *
 * ## Responsibilities
 * - Verify each queue-derived phase renders the honest copy + CTA state:
 *   idle/empty, idle/ready, queued, running (with live counts), paused.
 * - Verify the CTA never claims success on a bare enqueue and is disabled while
 *   a build is pending, and that the build handler fires only from the idle CTA.
 *
 * ## Not responsible for
 * - Deriving the progress (covered by `deriveSmartIndexProgress` unit tests).
 * - The bounded queue poll (covered by the route integration tests).
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import type { SmartIndexProgress } from '../paper-search-helpers'
import { SmartIndexStatusCallout } from './smart-index-status'

// Echo translator: returns the key (with params appended) so assertions can
// target the i18n key without coupling to English copy.
const t = (key: string, params?: Record<string, string | number>) =>
  params ? `${key}:${JSON.stringify(params)}` : key

function makeProgress(
  over: Partial<SmartIndexProgress> = {},
): SmartIndexProgress {
  return {
    phase: 'idle',
    active: false,
    queuedJobs: 0,
    runningJobs: 0,
    indexedItems: 0,
    ...over,
  }
}

function renderCallout(
  progress: SmartIndexProgress,
  over: {
    onBuild?: () => void
    hasEmbeddingProvider?: boolean
    readyTitle?: string
  } = {},
) {
  const onBuild = over.onBuild ?? vi.fn()
  render(
    <MemoryRouter>
      <SmartIndexStatusCallout
        progress={progress}
        readyTitle={over.readyTitle ?? 'Smart recall ready'}
        hasEmbeddingProvider={over.hasEmbeddingProvider ?? true}
        language="en"
        explorerT={t}
        onBuild={onBuild}
      />
    </MemoryRouter>,
  )
  return { onBuild }
}

describe('SmartIndexStatusCallout', () => {
  test('idle + empty index: prompts to build and the CTA is enabled', async () => {
    const user = userEvent.setup()
    const { onBuild } = renderCallout(makeProgress({ indexedItems: 0 }))
    expect(screen.getByText('smartIndexBuildTitle')).toBeVisible()
    const cta = screen.getByTestId('explorer-smart-build-index')
    expect(cta).toHaveTextContent('smartIndexBuildCta')
    expect(cta).toBeEnabled()
    await user.click(cta)
    expect(onBuild).toHaveBeenCalledTimes(1)
  })

  test('idle + ready index: shows the ready title + "N indexed" and can rebuild', () => {
    renderCallout(makeProgress({ indexedItems: 1200 }), {
      readyTitle: 'Smart recall ready',
    })
    expect(screen.getByText('Smart recall ready')).toBeVisible()
    // Completion signal: the indexed count is surfaced.
    expect(screen.getByText(/smartIndexReadyBody/)).toHaveTextContent(
      '"count":"1,200"',
    )
    expect(screen.getByTestId('explorer-smart-build-index')).toBeEnabled()
  })

  test('idle CTA is disabled without an embedding provider (component invariant)', () => {
    renderCallout(makeProgress({ indexedItems: 0 }), {
      hasEmbeddingProvider: false,
    })
    expect(screen.getByTestId('explorer-smart-build-index')).toBeDisabled()
  })

  test('queued: honest "not ready yet" copy + disabled CTA, never implies built', () => {
    const { onBuild } = renderCallout(
      makeProgress({ phase: 'queued', active: true, queuedJobs: 2 }),
    )
    expect(screen.getByText('smartIndexQueuedTitle')).toBeVisible()
    // The CTA shows the building label and is disabled — no second enqueue.
    const cta = screen.getByTestId('explorer-smart-build-index')
    expect(cta).toHaveTextContent('smartIndexBuildingCta')
    expect(cta).toBeDisabled()
    // The ready copy must NOT show on a bare enqueue.
    expect(screen.queryByText(/smartIndexReadyBody/)).toBeNull()
    expect(onBuild).not.toHaveBeenCalled()
  })

  test('running: live queued/running counts via the loading progress, no fake percent', () => {
    renderCallout(
      makeProgress({
        phase: 'running',
        active: true,
        queuedJobs: 5,
        runningJobs: 1,
        indexedItems: 40,
      }),
    )
    expect(screen.getByText('smartIndexBuildingTitle')).toBeVisible()
    // The shared LoadingState progress shows the real queue counts (no percent).
    const progress = screen.getByTestId('explorer-smart-build-progress')
    expect(progress).toHaveTextContent('queueProgressLabel')
    expect(progress).toHaveTextContent('"queued":"5"')
    expect(progress).toHaveTextContent('"running":"1"')
    // No fabricated percent rendered.
    expect(progress).not.toHaveTextContent('%')
    expect(screen.getByTestId('explorer-smart-build-index')).toBeDisabled()
  })

  test('paused: says the queue is paused and points to resume, not "building"', () => {
    renderCallout(
      makeProgress({ phase: 'paused', active: true, queuedJobs: 1 }),
    )
    expect(screen.getByText('smartIndexPausedTitle')).toBeVisible()
    const resume = screen.getByTestId('explorer-smart-resume-index')
    expect(resume).toHaveTextContent('smartIndexResumeCta')
    expect(resume).toHaveAttribute('href', '/settings#settings-ai')
    // No build progress / building title in the paused state.
    expect(screen.queryByTestId('explorer-smart-build-progress')).toBeNull()
    expect(screen.queryByText('smartIndexBuildingTitle')).toBeNull()
  })
})
