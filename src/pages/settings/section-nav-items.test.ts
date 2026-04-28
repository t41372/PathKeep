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
      ['general', 'remote'],
    )

    expect(items).toEqual([
      {
        id: 'settings-general',
        icon: 'settings',
        key: 'general',
        label: 'translated:settings.general',
      },
      {
        id: 'settings-remote',
        icon: 'cloud_upload',
        key: 'remote',
        label: 'translated:settings.remoteBackup',
      },
    ])
    expect(getSettingsSectionNavItem(items, 'remote').id).toBe(
      'settings-remote',
    )
    expect(() => getSettingsSectionNavItem(items, 'derived')).toThrow(
      'Missing settings section nav item: derived',
    )
  })
})
