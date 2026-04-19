/**
 * Guards the section metadata drawer against malformed backend envelopes.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { IntelligenceSectionMeta } from './section-meta'
import { createNamespaceTranslator, I18nProvider } from '../../lib/i18n'
import type { CoreIntelligenceSectionMeta as SectionMeta } from '../../lib/core-intelligence'

const intelligenceT = createNamespaceTranslator('en', 'intelligence')

describe('IntelligenceSectionMeta', () => {
  test('degrades malformed window metadata instead of crashing', () => {
    const malformedMeta = {
      sectionId: 'digest-summary',
      generatedAt: null,
      window: {
        kind: 'date-range',
      },
      moduleIds: [],
      sourceTables: [],
      includesEnrichment: false,
      state: 'ready',
      stateReason: null,
      notes: [],
    } as unknown as SectionMeta

    expect(() =>
      render(
        <I18nProvider>
          <IntelligenceSectionMeta
            meta={malformedMeta}
            scopeLabel="All profiles"
          />
        </I18nProvider>,
      ),
    ).not.toThrow()

    expect(
      screen.getByText(intelligenceT('sectionMetaStateDegraded')),
    ).toBeVisible()
    expect(
      screen.getByText(intelligenceT('sectionMetaMetadataFallback')),
    ).toBeVisible()
  })
})
