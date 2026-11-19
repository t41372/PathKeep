/**
 * The save step and the Agent backup page in Chinese (zh-TW, zh-CN); the
 * other agent-backup specs read them in English. Each turns Agent
 * conversations off through the backend (setting up, not what is tested)
 * and on again through the dialog, so its Run step and Done step show.
 *
 * Proves, per language: the Run step's title and per-tool list, the Done
 * step's title, and the backup page's title, "Back up now", sources and
 * recent backups are in that language, with no message key showing.
 */
import { expect, test } from '@playwright/test'
import type { AppSnapshot } from '../../src/lib/types'
import {
  agentKindState,
  newestRunRef,
  waitForRunAfter,
} from './support/agent-backup'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'

const LOCALES = [
  {
    language: 'zh-TW',
    next: '下一步',
    start: '開始保存',
    running: '正在保存',
    tools: '每個工具的保存進度',
    done: '存好了',
    close: '關閉',
    title: '備份',
    now: '立即備份',
    sources: '來源',
    runs: '最近的備份',
  },
  {
    language: 'zh-CN',
    next: '下一步',
    start: '开始保存',
    running: '正在保存',
    tools: '每个工具的保存进度',
    done: '存好了',
    close: '关闭',
    title: '备份',
    now: '立即备份',
    sources: '来源',
    runs: '最近的备份',
  },
] as const

/** A message key shown as itself, the translator's answer for a key it lacks. */
const MESSAGE_KEY =
  /\b(?:agentBackup|agentTasks|enableAgent|dataKinds|spaces|common)\.[a-zA-Z]+(?:\.[a-zA-Z]+)*/

async function turnOff() {
  const list = await backend<{ policyToken: string }>('list_data_kinds', {
    request: {},
  })
  await backend('set_data_kind_state', {
    request: {
      kind: 'agent-history',
      state: 'off',
      expectedPolicyToken: list.policyToken,
    },
  })
}

for (const locale of LOCALES) {
  test(`the save step and the backup page in ${locale.language}`, async ({
    page,
  }, testInfo) => {
    const before = await backend<AppSnapshot>('app_snapshot')
    try {
      await backend('save_config', {
        config: { ...before.config, preferredLanguage: locale.language },
      })
      await page.addInitScript((language) => {
        localStorage.setItem('pathkeep-language-preference', language)
      }, locale.language)
      // Read while still on: the ledger is not listed while the kind is off.
      const previous = await newestRunRef()
      await turnOff()

      await page.goto('/#/agent/backup?enable=agent')
      await unlockIfLocked(page)
      const dialog = page.getByRole('dialog')
      await expect(
        dialog.getByRole('button', { name: locale.next }),
      ).toBeEnabled({ timeout: 60_000 })
      await dialog.getByRole('button', { name: locale.next }).click()
      await dialog.getByRole('button', { name: locale.start }).click()
      await expect(
        dialog.getByRole('heading', { name: locale.running }),
      ).toBeVisible()
      await expect(
        dialog.getByRole('list', { name: locale.tools }),
      ).toBeVisible()
      expect(await dialog.innerText()).not.toMatch(MESSAGE_KEY)
      await testInfo.attach(`Run step in ${locale.language}`, {
        body: await dialog.screenshot(),
        contentType: 'image/png',
      })
      await expect(
        dialog.getByRole('heading', { name: locale.done }),
      ).toBeVisible({ timeout: 120_000 })
      await waitForRunAfter(previous)
      expect(await dialog.innerText()).not.toMatch(MESSAGE_KEY)
      await testInfo.attach(`Done step in ${locale.language}`, {
        body: await dialog.screenshot(),
        contentType: 'image/png',
      })
      // The footer's button; the × in the corner has the same name.
      await dialog.getByRole('button', { name: locale.close }).first().click()

      await expect(
        page.getByRole('heading', { name: locale.title, level: 1 }),
      ).toBeVisible()
      await expect(page.getByRole('button', { name: locale.now })).toBeVisible()
      await expect(
        page.getByRole('region', { name: locale.sources }),
      ).toBeVisible()
      await page
        .getByRole('button', { name: new RegExp(`^${locale.runs}`) })
        .click()
      expect(await page.getByRole('main').innerText()).not.toMatch(MESSAGE_KEY)
      await testInfo.attach(`backup page in ${locale.language}`, {
        body: await page.screenshot({ fullPage: true }),
        contentType: 'image/png',
      })
    } finally {
      // Every spec after this one reads the English interface, with
      // conversations on.
      const now = await backend<AppSnapshot>('app_snapshot')
      await backend('save_config', {
        config: {
          ...now.config,
          preferredLanguage: before.config.preferredLanguage,
        },
      })
      expect((await agentKindState())?.state).toBe('on')
    }
  })
}
