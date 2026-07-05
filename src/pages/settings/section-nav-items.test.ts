/**
 * @file section-nav-items.test.ts
 * @description Contract tests for the shared Settings section descriptor helpers.
 * @module pages/settings
 */

import { describe, expect, test } from 'vitest'
import {
  createSettingsSectionNavItems,
  getSettingsSectionNavItem,
} from './section-nav-items'

describe('settings section nav item helpers', () => {
  test('builds translated descriptors and throws on missing section lookups', () => {
    const items = createSettingsSectionNavItems(
      (key) => `translated:${key}`,
      ['general', 'migration'],
    )

    expect(items).toEqual([
      {
        id: 'settings-general',
        icon: 'settings',
        key: 'general',
        label: 'translated:settings.general',
      },
      {
        id: 'settings-migration',
        icon: 'download',
        key: 'migration',
        label: 'translated:settings.migrationTitle',
      },
    ])
    expect(getSettingsSectionNavItem(items, 'migration').id).toBe(
      'settings-migration',
    )
    expect(() => getSettingsSectionNavItem(items, 'derived')).toThrow(
      'Missing settings section nav item: derived',
    )
  })

  test('ENR-1: exposes the content-fetch consent nav entry with the section anchor id', () => {
    const items = createSettingsSectionNavItems(
      (key) => `translated:${key}`,
      ['contentFetch'],
    )

    // The id MUST equal the ContentFetchSection container anchor so the sticky
    // nav scroll lands on the consent card.
    expect(getSettingsSectionNavItem(items, 'contentFetch')).toEqual({
      id: 'content-fetch',
      icon: 'public',
      key: 'contentFetch',
      label: 'translated:settings.contentFetchNavLabel',
    })
  })
})
