/**
 * @file external-outputs-shared.test.ts
 * @description Protects locale-owned external-output preview helpers.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify backend-authored Core Intelligence card copy is localized before display.
 * - Keep known dynamic card-body patterns covered without rendering the full route.
 *
 * ## Not responsible for
 * - Testing the external-output route, tabs, or backend payload fetching.
 * - Validating raw JSON payload content.
 *
 * ## Dependencies
 * - Depends on the Settings namespace translator so helper assertions exercise real locale keys.
 *
 * ## Performance notes
 * - Pure helper tests only; no DOM render or backend fixture setup.
 */

import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import {
  localizeOutputCardBody,
  localizeOutputCardEyebrow,
  localizeOutputCardTitle,
} from './external-outputs-shared'

describe('external-output preview localization helpers', () => {
  test('localizes known backend card strings for zh-CN previews', () => {
    const t = createNamespaceTranslator('zh-CN', 'settings')

    expect(localizeOutputCardTitle('Visits', t)).toBe('访问')
    expect(localizeOutputCardTitle('On This Day · 2025', t)).toBe(
      '历史今日 · 2025',
    )
    expect(localizeOutputCardEyebrow('STABLE SOURCE', t)).toBe('稳定来源')
    expect(
      localizeOutputCardBody(
        'Total visits in the selected intelligence window.',
        t,
      ),
    ).toBe('这个智能时间窗口内的总访问次数。')
    expect(
      localizeOutputCardBody(
        'This page kept resurfacing across 348 days and 1089 trails.',
        t,
      ),
    ).toBe('这个页面在 348 天、1089 条轨迹中反复出现。')
    expect(
      localizeOutputCardBody(
        'github.com often resolves trails as a reference source.',
        t,
      ),
    ).toBe('github.com 经常作为参考来源帮助收束浏览轨迹。')
    expect(localizeOutputCardBody('Mostly browsing linux.do', t)).toBe(
      '主要在浏览 linux.do',
    )
  })

  test('leaves unknown future backend strings untouched', () => {
    const t = createNamespaceTranslator('en', 'settings')

    expect(localizeOutputCardTitle('Custom signal', t)).toBe('Custom signal')
    expect(localizeOutputCardEyebrow('EXPERIMENT', t)).toBe('EXPERIMENT')
    expect(localizeOutputCardBody('Future backend sentence.', t)).toBe(
      'Future backend sentence.',
    )
  })
})
