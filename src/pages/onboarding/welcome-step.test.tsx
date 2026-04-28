/**
 * @file welcome-step.test.tsx
 * @description Verifies the onboarding welcome hero's build-label and CTA contract.
 * @module pages/onboarding
 *
 * ## Responsibilities
 * - Keep the welcome hero's version fallback behavior explicit.
 * - Verify the route-owned begin action is called from the primary CTA.
 *
 * ## Not responsible for
 * - Re-testing onboarding route step transitions.
 * - Re-testing the shared brand mark implementation.
 *
 * ## Dependencies
 * - Uses the real i18n provider so the visible version line matches shipped copy.
 *
 * ## Performance notes
 * - The component is static, so the test stays render-only except for the CTA click.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { i18nStorageKey } from '../../lib/i18n/context'
import type { AppBuildInfo } from '../../lib/types'
import { WelcomeStep } from './welcome-step'

const buildInfo: AppBuildInfo = {
  productName: 'PathKeep',
  version: '1.2.3',
  gitCommitShort: 'abc123',
  gitCommitFull: 'abc123def456',
  gitDirty: false,
}

function renderWelcomeStep(
  props: {
    buildInfo?: AppBuildInfo | null
    buildRevision?: string | null
    buildTitle?: string | null
    onBegin?: () => void
  } = {},
) {
  window.localStorage.setItem(i18nStorageKey, 'en')
  const onBegin = props.onBegin ?? vi.fn()

  return {
    onBegin,
    ...render(
      <I18nProvider>
        <WelcomeStep
          buildInfo={props.buildInfo ?? null}
          buildRevision={props.buildRevision ?? null}
          buildTitle={props.buildTitle ?? null}
          onBegin={onBegin}
        />
      </I18nProvider>,
    ),
  }
}

afterEach(() => {
  window.localStorage.removeItem(i18nStorageKey)
})

describe('WelcomeStep', () => {
  test('renders explicit build metadata and calls the begin handler', () => {
    const onBegin = vi.fn()
    renderWelcomeStep({
      buildInfo,
      buildRevision: 'abc123',
      buildTitle: 'PathKeep 1.2.3 abc123',
      onBegin,
    })

    expect(
      screen.getByText('v1.2.3 · abc123 · Tauri desktop app'),
    ).toHaveAttribute('title', 'PathKeep 1.2.3 abc123')

    fireEvent.click(screen.getByRole('button', { name: 'Get Started →' }))

    expect(onBegin).toHaveBeenCalledTimes(1)
  })

  test('falls back between preview and release version labels', () => {
    const { rerender } = renderWelcomeStep({
      buildInfo: null,
      buildRevision: 'dev',
      buildTitle: null,
    })

    expect(
      screen.getByText('vpreview · dev · Tauri desktop app'),
    ).not.toHaveAttribute('title')

    rerender(
      <I18nProvider>
        <WelcomeStep
          buildInfo={buildInfo}
          buildRevision={null}
          buildTitle={null}
          onBegin={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('v1.2.3 · Tauri desktop app')).not.toHaveAttribute(
      'title',
    )

    rerender(
      <I18nProvider>
        <WelcomeStep
          buildInfo={null}
          buildRevision={null}
          buildTitle={null}
          onBegin={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(
      screen.getByText('vpreview · Tauri desktop app'),
    ).not.toHaveAttribute('title')
  })
})
