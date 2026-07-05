/**
 * Focused coverage for the paper-shell sidebar's optional nav badge.
 *
 * Why this file exists:
 * - Shipped routes no longer carry a roadmap `badgeKey` (the assistant lost its
 *   "v0.3" badge once AI became a consent-gated, configurable feature), but the
 *   badge-rendering branch is a deliberate, reusable capability. This test pins
 *   it by mocking `sidebarSections` to include one badge-carrying item, so the
 *   truthy branch stays covered without faking the production nav schema.
 *
 * Source-of-truth notes:
 * - Keep assertions on badge visibility and the collapsed-rail suppression, not
 *   on decorative class names.
 */

import { describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { I18nProvider } from '@/lib/i18n'

vi.mock('@/app/router', () => ({
  sidebarSections: [
    {
      id: 'CORE',
      labelKey: 'navigation.coreSection',
      items: [
        {
          id: 'assistant',
          labelKey: 'navigation.assistantLabel',
          titleKey: 'navigation.assistantTitle',
          subtitleKey: 'navigation.assistantSubtitle',
          icon: 'smart_toy',
          href: '/assistant',
          badgeKey: 'navigation.assistantLabel',
          section: 'CORE',
        },
      ],
    },
  ],
}))

vi.mock('@/components/shell/pk-glyph', () => ({
  PKGlyph: () => <span data-testid="pk-glyph" />,
}))

vi.mock('@/components/shell/pk-brand-mark', () => ({
  PKBrandMark: () => <span data-testid="pk-brand" />,
}))

const { PKSidebar } = await import('./pk-sidebar')

function renderSidebar(collapsed: boolean) {
  return render(
    <I18nProvider>
      <MemoryRouter>
        <PKSidebar
          activeId="assistant"
          collapsed={collapsed}
          onToggleCollapse={() => {}}
          theme="light"
          onToggleTheme={() => {}}
          onLockNow={() => {}}
          buildVersion="v0.3.0"
          archiveHealthy
        />
      </MemoryRouter>
    </I18nProvider>,
  )
}

describe('PKSidebar nav badge', () => {
  test('renders a badge for a screen that still carries a badgeKey', () => {
    renderSidebar(false)

    const badge = document.querySelector('nav .border-border-default')
    expect(badge).not.toBeNull()
    // The badge resolves its i18n key (here reusing the assistant label key).
    expect(badge?.textContent).toBe('AI Assistant')
  })

  test('suppresses the badge (and labels) while the rail is collapsed', () => {
    renderSidebar(true)

    // Collapsed rail hides the label + badge entirely.
    expect(screen.queryByText('AI Assistant')).toBeNull()
    expect(document.querySelector('nav .border-border-default')).toBeNull()
  })
})
