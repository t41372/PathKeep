/**
 * @file index.test.tsx
 * @description Guards the Assistant route's gated (AI-off) states on the paper shell.
 * @module pages/assistant
 *
 * ## Responsibilities
 * - Pin the AI-off gate to an honest, actionable "configure your AI provider" callout that
 *   deep-links to the AI settings section — never roadmap / "coming in v0.3" copy.
 * - Keep the setup and locked gates on the same paper wrapper while preserving their existing copy and deep links.
 *
 * ## Not responsible for
 * - The active-AI conversation surface, queue mutations, or provider probes; `intelligence-surfaces/assistant-and-shell.test.tsx` owns those (it mocks `optionalAiFeaturesAvailable` to true).
 * - Re-testing PaperCard internals, which `components/cards` owns.
 *
 * ## Dependencies
 * - Reuses the Intelligence-surface harness for archive seeding and the shipped provider stack.
 * - Runs against the real `release-capabilities` module (flag is on); the gate is exercised via the seeded archive's default-OFF `config.ai.enabled`.
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

describe('AssistantPage gated states', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
  })

  test('shows an actionable "configure your AI provider" gate when AI is off', async () => {
    const { snapshot } = await seedArchiveState()
    const assistantT = createNamespaceTranslator('en', 'assistant')

    // The seeded archive ships AI off by default (consent-gated), so the gate fires.
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

    // Honest, actionable copy — no roadmap / "coming in v0.3" framing.
    expect(screen.getByText(assistantT('disabledTitle'))).toBeVisible()
    expect(screen.getByText(assistantT('disabledBody'))).toBeVisible()
    expect(screen.queryByText(/v0\.3/)).toBeNull()

    // The callout deep-links straight to the AI settings section.
    expect(
      screen.getByRole('link', { name: assistantT('openSettings') }),
    ).toHaveAttribute('href', '/settings#settings-ai')

    // The setup panel reuses the empty-state copy in a PaperCard, not a `.panel`.
    const setupCard = screen.getByTestId('assistant-setup-panel')
    expect(setupCard).toBeVisible()
    expect(setupCard.querySelector('.panel')).toBeNull()
    expect(screen.getByText(assistantT('emptyEyebrow'))).toBeVisible()

    // The gated state must not leak the active-AI suggested-question rows.
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
