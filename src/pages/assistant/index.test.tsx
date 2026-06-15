/**
 * @file index.test.tsx
 * @description Guards the Assistant route's paper-shell migration for its v0.2.0-reachable states.
 * @module pages/assistant
 *
 * ## Responsibilities
 * - Pin the deferred (v0.3-roadmap) state to the paper outer wrapper + PaperCard so it stops rendering as a v0.2 `page-shell` style island.
 * - Keep the setup and locked gates on the same paper wrapper while preserving their existing copy and deep links.
 *
 * ## Not responsible for
 * - The active-AI conversation surface, queue mutations, or provider probes; `intelligence-surfaces/assistant-and-shell.test.tsx` owns those (it mocks `optionalAiFeaturesAvailable` to true).
 * - Re-testing PaperCard internals, which `components/cards` owns.
 *
 * ## Dependencies
 * - Reuses the Intelligence-surface harness for archive seeding and the shipped provider stack.
 * - Runs against the real `release-capabilities` module so the deferred (`optionalAiFeaturesAvailable === false`) branch is exercised with production values.
 */

import { screen } from '@testing-library/react'
import { beforeEach, describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import { AssistantPage } from './index'
import {
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from '../intelligence-surfaces/test-helpers'

describe('AssistantPage paper-shell migration', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('renders the deferred state inside the paper wrapper and PaperCard', async () => {
    const { snapshot } = await seedArchiveState()
    const assistantT = createNamespaceTranslator('en', 'assistant')

    renderSurface(<AssistantPage />, {
      route: '/assistant',
      snapshot,
    })

    const page = await screen.findByTestId('assistant-page')
    expect(page).toBeVisible()
    // Paper outer wrapper, not the legacy `page-shell` section.
    expect(page.tagName).toBe('DIV')
    expect(page).toHaveClass('max-w-[1080px]')
    expect(page.querySelector('.page-shell')).toBeNull()

    // Deferred copy now lives in a PaperCard rather than a `.panel`.
    const deferredCard = screen.getByTestId('assistant-deferred-panel')
    expect(deferredCard).toBeVisible()
    expect(deferredCard.querySelector('.panel')).toBeNull()
    expect(screen.getByText(assistantT('deferredTitle'))).toBeVisible()
    expect(screen.getByText(assistantT('deferredPanelEyebrow'))).toBeVisible()
    expect(screen.getByText(assistantT('deferredBadge'))).toBeVisible()
    expect(screen.getByText(assistantT('deferredPanelBody'))).toBeVisible()

    // The deferred state must not leak the active-AI suggested-question rows.
    expect(
      screen.queryByText(assistantT('examplePromptTimeline')),
    ).not.toBeInTheDocument()
  })

  test('keeps the setup gate on the paper wrapper with its onboarding link', async () => {
    const { snapshot } = await seedArchiveState()
    const assistantT = createNamespaceTranslator('en', 'assistant')
    const uninitialized = structuredClone(snapshot)
    uninitialized.config.initialized = false

    renderSurface(<AssistantPage />, {
      route: '/assistant',
      snapshot: uninitialized,
    })

    const page = await screen.findByTestId('assistant-page')
    expect(page.tagName).toBe('DIV')
    expect(page).toHaveClass('max-w-[1080px]')
    expect(
      screen.getByText(assistantT('archiveNotInitializedTitle')),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: assistantT('goToSetup') }),
    ).toHaveAttribute('href', '/onboarding')
  })

  test('keeps the locked gate on the paper wrapper with its security link', async () => {
    const { snapshot } = await seedArchiveState()
    const assistantT = createNamespaceTranslator('en', 'assistant')
    const locked = structuredClone(snapshot)
    locked.archiveStatus.unlocked = false
    locked.appLockStatus.locked = true

    renderSurface(<AssistantPage />, {
      route: '/assistant',
      snapshot: locked,
    })

    const page = await screen.findByTestId('assistant-page')
    expect(page.tagName).toBe('DIV')
    expect(page).toHaveClass('max-w-[1080px]')
    expect(screen.getByText(assistantT('lockedTitle'))).toBeVisible()
    expect(
      screen.getByRole('link', { name: assistantT('reviewSecurity') }),
    ).toHaveAttribute('href', '/security')
  })
})
