/**
 * @file ai-gpu-section.test.tsx
 * @description Tests for the GPU heavy-tier + re-embed Settings section (W-AI-9 Sub-block D).
 *
 * Covers: the honest gpuEnabled toggle (reflects the draft; mutates via the route
 * handler), the honest CPU-only-build state (no green "GPU on" lie), the cost
 * estimate display, the working-set + full-archive re-embed actions with the
 * full-archive GPU gate, bounded progress polling, and the degraded estimate /
 * queue-status error paths. All `backend` calls are spied; no real transport.
 */

import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, render, screen, waitFor } from '@testing-library/react'
import { I18nProvider } from '@/lib/i18n'
import { backend } from '@/lib/backend-client'
import type { AiSettings, ReembedEstimate } from '@/lib/types'
import { AiGpuSection } from './ai-gpu-section'

function aiSettings(overrides: Partial<AiSettings> = {}): AiSettings {
  return {
    enabled: true,
    assistantEnabled: false,
    // Re-embedding requires Smart search (the semantic index) on — the section's whole purpose — so
    // the default fixture has it ON. The blocked-state test overrides it to false to assert the gate.
    semanticIndexEnabled: true,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    enrichmentEnabled: true,
    enrichmentPlugins: [],
    retrievalTopK: 8,
    assistantSystemPrompt: '',
    llmProviders: [],
    embeddingProviders: [],
    ...overrides,
  }
}

function estimate(overrides: Partial<ReembedEstimate> = {}): ReembedEstimate {
  return {
    scope: 'working-set',
    pageCount: 1500,
    estMinutesCpu: 20,
    estMinutesGpu: 2,
    gpuAvailable: false,
    ...overrides,
  }
}

function renderSection(
  props: Partial<Parameters<typeof AiGpuSection>[0]> = {},
) {
  const onToggleGpu = props.onToggleGpu ?? vi.fn()
  const result = render(
    <I18nProvider>
      <AiGpuSection
        settings={props.settings ?? aiSettings()}
        disabled={props.disabled ?? false}
        onToggleGpu={onToggleGpu}
      />
    </I18nProvider>,
  )
  return { ...result, onToggleGpu }
}

function mockEstimates(gpuAvailable: boolean) {
  vi.spyOn(backend, 'estimateReembed').mockImplementation((scope) =>
    Promise.resolve(
      estimate({
        scope,
        pageCount: scope === 'full' ? 12000 : 1500,
        gpuAvailable,
      }),
    ),
  )
}

/** Toggles the disclosure open/closed so the lazy estimate effect runs/cleans up. */
async function setDisclosure(open: boolean) {
  const details = screen.getByTestId<HTMLDetailsElement>('ai-gpu-section')
  await act(async () => {
    details.open = open
    details.dispatchEvent(new Event('toggle', { bubbles: false }))
    await Promise.resolve()
  })
}

/** Opens the disclosure so the lazy estimate fetch + controls render. */
async function openDisclosure() {
  await setDisclosure(true)
}

afterEach(() => {
  vi.restoreAllMocks()
  vi.useRealTimers()
})

describe('AiGpuSection', () => {
  it('returns null when there is no draft', () => {
    const { container } = render(
      <I18nProvider>
        <AiGpuSection settings={null} disabled={false} onToggleGpu={vi.fn()} />
      </I18nProvider>,
    )
    expect(container.querySelector('[data-testid="ai-gpu-section"]')).toBeNull()
  })

  it('reflects the draft gpuEnabled and flips it through the route handler on a Metal build', async () => {
    mockEstimates(true)
    const { onToggleGpu } = renderSection({
      settings: aiSettings({ gpuEnabled: true }),
    })
    // Open the disclosure so the estimate (gpuAvailable: true) resolves and the
    // toggle becomes actuating — a CPU-only build keeps it disabled (see below).
    await openDisclosure()
    const toggle = screen.getByTestId<HTMLInputElement>('ai-gpu-toggle')
    expect(toggle.checked).toBe(true)
    await waitFor(() => {
      expect(toggle.disabled).toBe(false)
    })
    await act(async () => {
      toggle.click()
      await Promise.resolve()
    })
    expect(onToggleGpu).toHaveBeenCalledTimes(1)
  })

  it('shows the honest CPU-only-build state when GPU is unavailable', async () => {
    mockEstimates(false)
    renderSection({ settings: aiSettings({ gpuEnabled: true }) })
    await openDisclosure()
    await waitFor(() => {
      expect(screen.getByTestId('ai-gpu-unavailable')).toBeTruthy()
    })
    expect(screen.getByTestId('ai-gpu-build-badge').textContent).toContain(
      'CPU-only build',
    )
    // Honesty: in a CPU-only build the toggle is DISABLED (non-actuating) so a
    // filled checkbox can never assert "ON" for a build that can't run Metal.
    const toggle = screen.getByTestId<HTMLInputElement>('ai-gpu-toggle')
    expect(toggle.disabled).toBe(true)
    // Full-archive re-embed is blocked (no GPU), shown as an honest reason not a button.
    expect(screen.getByTestId('ai-reembed-full-blocked')).toBeTruthy()
    expect(screen.queryByTestId('ai-reembed-full-start')).toBeNull()
  })

  it('keeps the GPU toggle non-actuating in a CPU-only build (no false "ON")', async () => {
    mockEstimates(false)
    const { onToggleGpu } = renderSection({
      settings: aiSettings({ gpuEnabled: false }),
    })
    await openDisclosure()
    await waitFor(() => {
      expect(screen.getByTestId('ai-gpu-unavailable')).toBeTruthy()
    })
    const toggle = screen.getByTestId<HTMLInputElement>('ai-gpu-toggle')
    // A disabled checkbox cannot be actuated — clicking it fires no change.
    expect(toggle.disabled).toBe(true)
    await act(async () => {
      toggle.click()
      await Promise.resolve()
    })
    expect(onToggleGpu).not.toHaveBeenCalled()
  })

  it('shows the cost estimate (pages + CPU + GPU minutes) when GPU is available', async () => {
    mockEstimates(true)
    renderSection({ settings: aiSettings({ gpuEnabled: true }) })
    await openDisclosure()
    await waitFor(() => {
      expect(screen.getByTestId('ai-reembed-working-set-estimate')).toBeTruthy()
    })
    const chip = screen.getByTestId('ai-reembed-working-set-estimate')
    expect(chip.textContent).toContain('pages')
    expect(chip.textContent).toContain('on CPU')
    expect(chip.textContent).toContain('on GPU')
    expect(screen.getByTestId('ai-gpu-build-badge').textContent).toContain(
      'Metal build',
    )
  })

  it('shows the GPU-unavailable estimate note when this build is CPU-only', async () => {
    mockEstimates(false)
    renderSection()
    await openDisclosure()
    await waitFor(() => {
      const chip = screen.getByTestId('ai-reembed-working-set-estimate')
      expect(chip.textContent).toContain('needs a Metal build')
    })
  })

  it('starts a working-set re-embed and polls bounded progress to done', async () => {
    vi.useFakeTimers()
    mockEstimates(true)
    const build = vi
      .spyOn(backend, 'buildAiIndex')
      .mockResolvedValue({} as never)
    const queue = vi
      .spyOn(backend, 'loadAiQueueStatus')
      .mockResolvedValueOnce({ queued: 1, running: 0 } as never)
      .mockResolvedValueOnce({ queued: 0, running: 0 } as never)
    renderSection({ settings: aiSettings({ gpuEnabled: true }) })
    await openDisclosure()
    await act(async () => {
      await vi.runOnlyPendingTimersAsync()
    })

    await act(async () => {
      screen.getByTestId('ai-reembed-working-set-start').click()
      await Promise.resolve()
    })
    expect(build).toHaveBeenCalledWith({
      fullRebuild: false,
      clearOnly: false,
      scope: 'working-set',
    })
    // First poll: queue still has work → running.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500)
    })
    expect(screen.getByTestId('ai-reembed-status').textContent).toContain(
      'Re-embedding',
    )
    // Second poll: queue drained → done.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500)
    })
    expect(screen.getByTestId('ai-reembed-status').textContent).toContain(
      'complete',
    )
    expect(queue).toHaveBeenCalled()
  })

  it('starts a full-archive re-embed with fullRebuild=true when GPU is enabled+available', async () => {
    vi.useFakeTimers()
    mockEstimates(true)
    const build = vi
      .spyOn(backend, 'buildAiIndex')
      .mockResolvedValue({} as never)
    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue({
      queued: 0,
      running: 0,
    } as never)
    renderSection({ settings: aiSettings({ gpuEnabled: true }) })
    await openDisclosure()
    await act(async () => {
      await vi.runOnlyPendingTimersAsync()
    })
    await act(async () => {
      screen.getByTestId('ai-reembed-full-start').click()
      await Promise.resolve()
    })
    expect(build).toHaveBeenCalledWith({
      fullRebuild: true,
      clearOnly: false,
      scope: 'full',
    })
  })

  it('surfaces an error when the re-embed fails to start', async () => {
    mockEstimates(true)
    vi.spyOn(backend, 'buildAiIndex').mockRejectedValue(new Error('boom'))
    renderSection({ settings: aiSettings({ gpuEnabled: true }) })
    await openDisclosure()
    await waitFor(() => {
      expect(screen.getByTestId('ai-reembed-working-set-start')).toBeTruthy()
    })
    await act(async () => {
      screen.getByTestId('ai-reembed-working-set-start').click()
      await Promise.resolve()
    })
    await waitFor(() => {
      expect(screen.getByTestId('ai-reembed-error')).toBeTruthy()
    })
  })

  it('degrades to an honest note when the estimate cannot load', async () => {
    vi.spyOn(backend, 'estimateReembed').mockRejectedValue(new Error('nope'))
    renderSection()
    await openDisclosure()
    await waitFor(() => {
      expect(screen.getByTestId('ai-reembed-estimate-error')).toBeTruthy()
    })
  })

  it('settles progress to the honest background state when the queue status read fails mid-poll', async () => {
    vi.useFakeTimers()
    mockEstimates(true)
    vi.spyOn(backend, 'buildAiIndex').mockResolvedValue({} as never)
    vi.spyOn(backend, 'loadAiQueueStatus').mockRejectedValue(
      new Error('status down'),
    )
    renderSection({ settings: aiSettings({ gpuEnabled: true }) })
    await openDisclosure()
    await act(async () => {
      await vi.runOnlyPendingTimersAsync()
    })
    await act(async () => {
      screen.getByTestId('ai-reembed-working-set-start').click()
      await Promise.resolve()
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500)
    })
    // A status hiccup must NOT claim "complete" — it settles to "running in the
    // background, check Jobs" (we genuinely don't know if it finished).
    const status = screen.getByTestId('ai-reembed-status').textContent
    expect(status).toContain('background')
    expect(status).not.toContain('complete')
  })

  it('never claims "complete" at the poll ceiling while work is still pending', async () => {
    vi.useFakeTimers()
    mockEstimates(true)
    vi.spyOn(backend, 'buildAiIndex').mockResolvedValue({} as never)
    // The queue never drains within the poll window — every read shows pending work,
    // so we must hit the ceiling and settle to "background", never a false "complete".
    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue({
      queued: 5,
      running: 1,
    } as never)
    renderSection({ settings: aiSettings({ gpuEnabled: true }) })
    await openDisclosure()
    await act(async () => {
      await vi.runOnlyPendingTimersAsync()
    })
    await act(async () => {
      screen.getByTestId('ai-reembed-working-set-start').click()
      await Promise.resolve()
    })
    // Drive past the bounded ceiling (MAX_POLLS = 80 ticks of 1500ms, plus slack).
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500 * 82)
    })
    const status = screen.getByTestId('ai-reembed-status').textContent
    expect(status).toContain('background')
    expect(status).not.toContain('complete')
  })

  it('drops a stale estimate resolution when the disclosure closes mid-fetch', async () => {
    // Race guard: if the disclosure closes before the estimate lands, the cleanup
    // sets `cancelled` so the late resolution is ignored (no setState on a stale
    // effect). Exercises the `if (!cancelled)` false branch in the `.then`.
    const deferreds: Array<(value: ReembedEstimate) => void> = []
    vi.spyOn(backend, 'estimateReembed').mockImplementation(
      () =>
        new Promise<ReembedEstimate>((resolve) => {
          deferreds.push(resolve)
        }),
    )
    renderSection()
    await openDisclosure()
    expect(deferreds.length).toBe(2)
    // Close the disclosure (cleanup runs → cancelled = true) BEFORE the fetch lands.
    await setDisclosure(false)
    // Now resolve the in-flight estimates; the stale resolution must be dropped.
    await act(async () => {
      deferreds.forEach((resolve) => resolve(estimate({ gpuAvailable: true })))
      await Promise.resolve()
    })
    // No estimate landed — the stale resolution was ignored (no setState).
    expect(screen.queryByTestId('ai-reembed-working-set-estimate')).toBeNull()
  })

  it('drops a stale estimate rejection when the disclosure closes mid-fetch', async () => {
    // The catch-side mirror of the above: a late REJECTION after close is ignored
    // (the `if (!cancelled)` false branch in `.catch`), so no error note surfaces.
    const rejecters: Array<(reason: unknown) => void> = []
    vi.spyOn(backend, 'estimateReembed').mockImplementation(
      () =>
        new Promise<ReembedEstimate>((_, reject) => {
          rejecters.push(reject)
        }),
    )
    renderSection()
    await openDisclosure()
    expect(rejecters.length).toBe(2)
    await setDisclosure(false)
    await act(async () => {
      rejecters.forEach((reject) => reject(new Error('late')))
      await Promise.resolve()
    })
    expect(screen.queryByTestId('ai-reembed-estimate-error')).toBeNull()
  })

  it('disables the actions while AI is off (disabled)', async () => {
    mockEstimates(true)
    renderSection({
      settings: aiSettings({ gpuEnabled: true }),
      disabled: true,
    })
    await openDisclosure()
    await waitFor(() => {
      expect(screen.getByTestId('ai-reembed-working-set-start')).toBeTruthy()
    })
    expect(
      screen.getByTestId<HTMLButtonElement>('ai-reembed-working-set-start')
        .disabled,
    ).toBe(true)
    expect(screen.getByTestId<HTMLInputElement>('ai-gpu-toggle').disabled).toBe(
      true,
    )
  })

  it('blocks BOTH re-embed actions with an honest reason when Smart search is off (M-3)', async () => {
    // A re-embed enqueues an embedding job (provider egress + a large derived-vector tail), so it
    // requires the semantic index (Smart search) sub-flag — mirroring the backend gate. With Smart
    // search OFF, neither action exposes a start button; both show the honest "turn on Smart search"
    // reason instead, so the UI never offers an action the backend would refuse. A Metal build is
    // mocked to prove the block is the SEMANTIC gate, not the GPU gate.
    mockEstimates(true)
    const build = vi
      .spyOn(backend, 'buildAiIndex')
      .mockResolvedValue({} as never)
    renderSection({
      settings: aiSettings({ gpuEnabled: true, semanticIndexEnabled: false }),
    })
    await openDisclosure()
    await waitFor(() => {
      expect(screen.getByTestId('ai-reembed-working-set-blocked')).toBeTruthy()
    })
    // Both scopes are blocked (no start buttons), and the reason names Smart search.
    expect(screen.queryByTestId('ai-reembed-working-set-start')).toBeNull()
    expect(screen.queryByTestId('ai-reembed-full-start')).toBeNull()
    const workingReason = screen.getByTestId(
      'ai-reembed-working-set-blocked',
    ).textContent
    expect(workingReason).toContain('Smart search')
    expect(screen.getByTestId('ai-reembed-full-blocked').textContent).toContain(
      'Smart search',
    )
    // No re-embed can be fired while blocked (there is no control to click).
    expect(build).not.toHaveBeenCalled()
  })
})
