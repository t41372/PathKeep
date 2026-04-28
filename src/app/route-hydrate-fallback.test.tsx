/**
 * @file route-hydrate-fallback.test.tsx
 * @description Direct coverage for the shell route hydration fallback.
 * @module app
 *
 * ## Responsibilities
 * - Verify lazy-route hydration shows the shared loading primitive.
 * - Keep the loading label sourced from the shipping i18n catalog.
 *
 * ## Not responsible for
 * - Re-testing route registry placement or individual lazy routes.
 *
 * ## Dependencies
 * - Uses the real i18n provider and loading primitive.
 *
 * ## Performance notes
 * - Single render only; no router bootstrap is needed.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator, I18nProvider } from '../lib/i18n'
import { RouteHydrateFallback } from './route-hydrate-fallback'

const commonT = createNamespaceTranslator('en', 'common')

describe('RouteHydrateFallback', () => {
  test('renders compact localized loading status for lazy route hydration', () => {
    render(
      <I18nProvider>
        <RouteHydrateFallback />
      </I18nProvider>,
    )

    expect(screen.getByRole('status')).toHaveClass('loading-state--compact')
    expect(screen.getByText(commonT('loading'))).toBeVisible()
  })
})
