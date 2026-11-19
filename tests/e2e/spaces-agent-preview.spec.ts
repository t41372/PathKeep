/**
 * The dialog that turns Agent conversations on (`?enable=agent`,
 * `data-spaces.md` §3.2): its preview and choose steps, on the real preview
 * the backend streams, against the synthetic tool folders
 * (`CHB_AGENT_TOOLS_HOME`; what each folder should count is in
 * `support/agent-tools.ts`).
 *
 * Proves:
 * - every folder's row (files that look like conversations, subagent files,
 *   size, earliest file date) and every tool's line in the choose step equal
 *   what the fixture wrote; the note says nothing was opened; the numbers
 *   say the growth is an estimate and the encryption is the archive's own;
 *   with every tool ticked, "Start saving" can be pressed (this spec does
 *   not press it: the agent-backup specs save, after the change project);
 * - stopping a preview while it runs leaves the dialog usable: it says what
 *   happened, and the next preview finishes with the same numbers. The
 *   stop reaches the backend with the stream's id;
 * - with no tool folder on this computer the dialog says so, where it
 *   looked, and finds them again on "Check again";
 * - a folder PathKeep cannot read says why; a tool with another folder it
 *   can read can still be ticked, and one without cannot (POSIX only);
 * - the ticks and the `.git` switch are kept as device settings: they come
 *   back when the dialog opens again, after a reload too;
 * - locking the app while a preview runs ends it: nothing of the dialog
 *   shows over the lock screen, and after unlocking a new preview runs
 *   (a new stream) with nothing from the old one;
 * - the dialog in Traditional and Simplified Chinese: its words, with no
 *   message key left untranslated.
 *
 * A running preview is held at the transport for as long as a test needs
 * (`page.route` on the bridge's private event stream, as
 * spaces-shell-tasks.spec holds a backup): the walk itself is over in
 * milliseconds on this fixture. Runs after lock.spec, so App Lock is on.
 */
import {
  expect,
  test,
  type Page,
  type Request,
  type Route,
} from '@playwright/test'
import type { AppSnapshot } from '../../src/lib/types'
import { setAgentToolReadable } from '../../scripts/fixtures/synthetic-agents.mjs'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { unlockIfLocked } from './support/app'
import {
  agentSettings,
  enableDialog,
  expectedFolders,
  expectedTool,
  expectFolderRow,
  folderRow,
  hideAllToolFolders,
  openEnableDialog,
  previewTable,
  resetAgentSettings,
  TOOL_ORDER,
  toolChoice,
  toolSummary,
  toolTick,
  waitForPreview,
} from './support/agent-tools'
import { backend, expectCount, fixture } from './support/fixture'

const PASSCODE = '4826'

/** Holds every preview's event stream until the returned `release`. */
async function holdPreviewStreams(page: Page) {
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const held: Route[] = []
  const pattern = `${devIpcUrl}/previews/*/events`
  await page.route(pattern, (route) => {
    held.push(route)
  })
  return {
    held,
    release: async () => {
      await page.unroute(pattern)
      // A stream the page closed meanwhile is gone; the rest go on.
      for (const route of held) await route.continue().catch(() => undefined)
    },
  }
}

/** The stream id the next preview starts with. */
function nextStreamId(page: Page) {
  return page
    .waitForResponse((response) =>
      response.url().endsWith('/commands/preview_agent_sources'),
    )
    .then(async (response) => {
      expect(response.ok()).toBe(true)
      return ((await response.json()) as { streamId: string }).streamId
    })
}

async function expectEveryFolder(page: Page) {
  const folders = expectedFolders()
  await expect(previewTable(page).locator('tbody tr')).toHaveCount(
    folders.length,
  )
  for (const folder of folders) await expectFolderRow(page, folder)
}

test.afterEach(async () => {
  await resetAgentSettings()
})

test('the preview counts what the fixture wrote, opens nothing, and the last step can start', async ({
  page,
}, testInfo) => {
  // Guards the numbers below: a folder missing from the fake home would make
  // the expectations shrink with it.
  expect(expectedFolders().map((folder) => folder.root)).toEqual([
    'claude-code',
    'claude-desktop',
    'codex',
    'pi',
    'deepseek',
  ])

  await openEnableDialog(page)
  const dialog = enableDialog(page)
  await expect(
    dialog.getByRole('heading', {
      name: 'Keep Agent conversations: first, see what PathKeep would read',
    }),
  ).toBeVisible()
  await expect(
    dialog.getByRole('list', { name: 'Steps' }).getByRole('listitem').nth(0),
  ).toHaveAttribute('aria-current', 'step')

  await waitForPreview(page)
  await expectEveryFolder(page)
  for (const folder of expectedFolders()) {
    await expectCount(
      testInfo,
      `${folder.root}: conversation files`,
      Number(
        (
          await folderRow(page, folder.path)
            .getByRole('cell')
            .nth(2)
            .innerText()
        )
          .split('\n')[0]
          .replace(/[^\d]/g, ''),
      ),
      folder.conversationFiles,
    )
  }
  await expect(
    dialog.getByText(
      'So far PathKeep has only counted files and looked at their sizes and dates. It hasn’t opened a single conversation.',
    ),
  ).toBeVisible()
  await expect(dialog.getByText('The archive grows by')).toBeVisible()
  await expect(dialog.getByText('estimate', { exact: true })).toBeVisible()
  await expect(dialog.getByText(/^about [\d.,]+ (B|KB|MB|GB)$/)).toBeVisible()
  const snapshot = await backend<AppSnapshot>('app_snapshot')
  await expect(
    dialog.getByText(
      snapshot.archiveStatus.encrypted
        ? 'Same as the archive now: encrypted'
        : 'Same as the archive now: not encrypted',
    ),
  ).toBeVisible()
  await testInfo.attach('preview', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })

  // Choose: every tool found, ticked, with its folders added up.
  await dialog.getByRole('button', { name: 'Next' }).click()
  await expect(
    dialog.getByRole('heading', {
      name: 'Which tools’ conversations should PathKeep keep?',
    }),
  ).toBeVisible()
  for (const tool of TOOL_ORDER) {
    const totals = expectedTool(tool)
    await expect(toolTick(page, tool)).toBeChecked()
    await expect(toolChoice(page, tool)).toContainText(
      toolSummary(totals.conversationFiles, totals.bytes),
    )
  }
  await expect(
    dialog.getByRole('switch', { name: 'Note each project’s git details' }),
  ).toBeChecked()
  const start = dialog.getByRole('button', { name: 'Start saving' })
  await expect(start).toBeEnabled()
  await testInfo.attach('choose', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })

  // Nothing was turned on.
  const kinds = await backend<{ kinds: { kind: string; state: string }[] }>(
    'list_data_kinds',
    { request: {} },
  )
  expect(kinds.kinds.find((row) => row.kind === 'agent-history')?.state).toBe(
    'off',
  )
})

test('stopping a running preview leaves the dialog usable, and the next one finishes', async ({
  page,
}, testInfo) => {
  const streams = await holdPreviewStreams(page)
  const firstStream = nextStreamId(page)
  await openEnableDialog(page)
  const dialog = enableDialog(page)
  const stream = await firstStream
  await expect(previewTable(page)).toHaveAttribute('aria-busy', 'true')
  await expect(dialog.getByText('Counting files…')).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Next' })).toBeDisabled()

  const cancelled = page.waitForRequest((request: Request) =>
    request.url().endsWith('/commands/cancel_agent_read'),
  )
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  const request = await cancelled
  expect(request.postDataJSON()).toEqual({
    request: { requestIdOrStreamId: stream },
  })
  await expect(
    dialog.getByText('The preview was stopped. Nothing was saved.'),
  ).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Next' })).toBeDisabled()
  await testInfo.attach('stopped', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })

  await streams.release()
  const secondStream = nextStreamId(page)
  await dialog.getByRole('button', { name: 'Run the preview again' }).click()
  expect(await secondStream).not.toBe(stream)
  await waitForPreview(page)
  await expectEveryFolder(page)
  await testInfo.attach('the next preview', {
    body: await dialog.screenshot(),
    contentType: 'image/png',
  })
})

test('with no tool folder on this computer the dialog says so, and finds them again', async ({
  page,
}, testInfo) => {
  const restore = hideAllToolFolders()
  try {
    await openEnableDialog(page)
    const dialog = enableDialog(page)
    await expect(
      dialog.getByText('No Agent tool folders on this computer'),
    ).toBeVisible({ timeout: 60_000 })
    await expect(dialog.getByText('Looked in:')).toBeVisible()
    for (const folder of expectedFoldersWhileHidden()) {
      await expect(dialog.getByText(folder, { exact: true })).toBeVisible()
    }
    await expect(previewTable(page)).toHaveCount(0)
    await expect(dialog.getByRole('button', { name: 'Next' })).toBeDisabled()
    await testInfo.attach('nothing found', {
      body: await dialog.screenshot(),
      contentType: 'image/png',
    })
  } finally {
    restore()
  }
  await enableDialog(page).getByRole('button', { name: 'Check again' }).click()
  await waitForPreview(page)
  await expectEveryFolder(page)
})

/** Where the app looks under the fake home: every folder the fixture can write. */
function expectedFoldersWhileHidden() {
  return Object.values(fixture().agents.roots)
}

test('a folder PathKeep cannot read says why, and only tools it can read can be ticked', async ({
  page,
}, testInfo) => {
  test.skip(
    process.platform === 'win32',
    'Windows chmod does not stop reads; an unreadable folder needs an ACL there',
  )
  test.skip(
    process.getuid?.() === 0,
    'root can read a folder with no permissions',
  )
  let agents = fixture().agents
  // Claude Code keeps a second, readable folder (the desktop app's); Codex has only this one.
  agents = setAgentToolReadable(agents, 'claude-code', false)
  agents = setAgentToolReadable(agents, 'codex', false)
  try {
    await openEnableDialog(page)
    await waitForPreview(page)
    const dialog = enableDialog(page)
    const reason =
      'PathKeep can’t read this folder: it has no permission, or the disk isn’t connected.'
    for (const root of ['claude-code', 'codex'] as const) {
      const row = folderRow(page, agents.roots[root])
      await expect(row).toContainText(reason)
      await expect(row.getByRole('cell').nth(2)).toHaveText('—')
      await expect(row.getByRole('cell').nth(3)).toHaveText('—')
    }
    for (const folder of expectedFolders()) {
      if (folder.root === 'claude-code' || folder.root === 'codex') continue
      await expectFolderRow(page, folder)
    }
    await testInfo.attach('unreadable folders', {
      body: await dialog.screenshot(),
      contentType: 'image/png',
    })

    await dialog.getByRole('button', { name: 'Next' }).click()
    await expect(toolTick(page, 'codex')).toBeDisabled()
    await expect(toolTick(page, 'codex')).not.toBeChecked()
    await expect(toolChoice(page, 'codex')).toContainText(
      'Can’t read its folder',
    )
    const claude = expectedTool('claude-code', ['claude-code'])
    await expect(toolTick(page, 'claude-code')).toBeEnabled()
    await expect(toolTick(page, 'claude-code')).toBeChecked()
    await expect(toolChoice(page, 'claude-code')).toContainText(
      toolSummary(claude.conversationFiles, claude.bytes),
    )
    for (const tool of ['pi', 'deepseek'] as const) {
      await expect(toolTick(page, tool)).toBeEnabled()
      await expect(toolTick(page, tool)).toBeChecked()
    }
    await testInfo.attach('choose with unreadable folders', {
      body: await dialog.screenshot(),
      contentType: 'image/png',
    })
  } finally {
    agents = setAgentToolReadable(agents, 'claude-code', true)
    setAgentToolReadable(agents, 'codex', true)
  }
})

test('the ticks and the .git switch come back when the dialog opens again', async ({
  page,
}, testInfo) => {
  await openEnableDialog(page)
  await waitForPreview(page)
  const dialog = enableDialog(page)
  await dialog.getByRole('button', { name: 'Next' }).click()
  await toolTick(page, 'pi').click()
  await toolTick(page, 'deepseek').click()
  const git = dialog.getByRole('switch', {
    name: 'Note each project’s git details',
  })
  await git.click()
  await expect(git).not.toBeChecked()

  // Kept as the device's settings, not only on screen.
  await expect
    .poll(async () => {
      const { policy } = await agentSettings()
      return { tools: policy.enabledTools, readGit: policy.readGit }
    })
    .toEqual({ tools: ['claude-code', 'codex'], readGit: false })

  const expectChosen = async () => {
    await openOrWait(page)
    await enableDialog(page).getByRole('button', { name: 'Next' }).click()
    await expect(toolTick(page, 'claude-code')).toBeChecked()
    await expect(toolTick(page, 'codex')).toBeChecked()
    await expect(toolTick(page, 'pi')).not.toBeChecked()
    await expect(toolTick(page, 'deepseek')).not.toBeChecked()
    await expect(
      enableDialog(page).getByRole('switch', {
        name: 'Note each project’s git details',
      }),
    ).not.toBeChecked()
  }

  // Closed and opened again from the Home card.
  await page.keyboard.press('Escape')
  await expect(enableDialog(page)).toBeHidden()
  await page
    .getByRole('button', { name: 'Preview what would be saved' })
    .click()
  await expectChosen()
  await testInfo.attach('choices after reopening', {
    body: await enableDialog(page).screenshot(),
    contentType: 'image/png',
  })

  // And after the window reloads.
  await page.reload()
  await unlockIfLocked(page)
  await expectChosen()
})

/** The dialog is open (or opening): waits for its preview to finish. */
async function openOrWait(page: Page) {
  await expect(enableDialog(page)).toBeVisible()
  await waitForPreview(page)
}

test('locking the app during a preview ends it, and nothing of it shows after unlock', async ({
  page,
}, testInfo) => {
  const streams = await holdPreviewStreams(page)
  const firstStream = nextStreamId(page)
  await openEnableDialog(page)
  const stream = await firstStream
  await expect(enableDialog(page).getByText('Counting files…')).toBeVisible()

  await page.keyboard.press('ControlOrMeta+l')
  const lock = page.getByRole('heading', { name: 'PathKeep is locked' })
  await expect(lock).toBeVisible()
  // Nothing of the dialog over the lock screen.
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(page.getByRole('table')).toHaveCount(0)
  await testInfo.attach('locked during a preview', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  // The backend refuses the preview while locked.
  let refusal = ''
  try {
    await backend('preview_agent_sources', {
      request: { purpose: 'enable' },
    })
  } catch (error) {
    refusal = String(error)
  }
  expect(refusal).toContain('lock-required')

  await streams.release()
  const secondStream = nextStreamId(page)
  await page.getByPlaceholder('Passcode').fill(PASSCODE)
  await page.getByPlaceholder('Passcode').press('Enter')
  await expect(lock).toBeHidden()

  // The address still asks for the dialog: a new preview, from the start.
  expect(await secondStream).not.toBe(stream)
  await expect(enableDialog(page)).toBeVisible()
  await waitForPreview(page)
  await expectEveryFolder(page)
  await expect(
    enableDialog(page).getByText('The preview didn’t finish'),
  ).toHaveCount(0)
  await expect(
    enableDialog(page).getByText('The preview was stopped. Nothing was saved.'),
  ).toHaveCount(0)
  await testInfo.attach('after unlocking', {
    body: await enableDialog(page).screenshot(),
    contentType: 'image/png',
  })
})

const LOCALES = [
  {
    language: 'zh-TW',
    title: '保存 Agent 對話：先看看會讀什麼',
    steps: ['看會讀什麼', '選擇', '保存'],
    columns: ['工具', '資料夾', '對話檔', '大小', '最早'],
    next: '下一步',
    chooseTitle: '要保存哪些工具的對話？',
    start: '開始保存',
  },
  {
    language: 'zh-CN',
    title: '保存 Agent 对话：先看看会读什么',
    steps: ['看会读什么', '选择', '保存'],
    columns: ['工具', '文件夹', '对话文件', '大小', '最早'],
    next: '下一步',
    chooseTitle: '要保存哪些工具的对话？',
    start: '开始保存',
  },
] as const

/** A message key shown as itself, the translator's answer for a key it lacks. */
const MESSAGE_KEY =
  /\b(?:enableAgent|dataKinds|spaces|common)\.[a-zA-Z]+(?:\.[a-zA-Z]+)*/

for (const locale of LOCALES) {
  test(`the dialog in ${locale.language}`, async ({ page }, testInfo) => {
    const before = await backend<AppSnapshot>('app_snapshot')
    try {
      // The archive's choice and the window's own, so neither overrides the other.
      await backend('save_config', {
        config: { ...before.config, preferredLanguage: locale.language },
      })
      await page.addInitScript((language) => {
        localStorage.setItem('pathkeep-language-preference', language)
      }, locale.language)
      await page.goto('/#/browsing/home?enable=agent')
      await unlockIfLocked(page)
      const dialog = page.getByRole('dialog')
      await expect(
        dialog.getByRole('heading', { name: locale.title }),
      ).toBeVisible()
      for (const step of locale.steps)
        await expect(
          dialog.getByText(step, { exact: true }).first(),
        ).toBeVisible()
      await expect(
        dialog.getByRole('button', { name: locale.next }),
      ).toBeEnabled({ timeout: 60_000 })
      for (const column of locale.columns)
        await expect(
          dialog.getByRole('columnheader', { name: column, exact: true }),
        ).toBeVisible()
      expect(await dialog.innerText()).not.toMatch(MESSAGE_KEY)
      await testInfo.attach(`preview in ${locale.language}`, {
        body: await dialog.screenshot(),
        contentType: 'image/png',
      })

      await dialog.getByRole('button', { name: locale.next }).click()
      await expect(
        dialog.getByRole('heading', { name: locale.chooseTitle }),
      ).toBeVisible()
      // Every tool ticked (`resetAgentSettings`): it can start; this spec
      // does not press it.
      await expect(
        dialog.getByRole('button', { name: locale.start }),
      ).toBeEnabled()
      expect(await dialog.innerText()).not.toMatch(MESSAGE_KEY)
      await testInfo.attach(`choose in ${locale.language}`, {
        body: await dialog.screenshot(),
        contentType: 'image/png',
      })
    } finally {
      // Every spec after this one reads the English interface.
      const now = await backend<AppSnapshot>('app_snapshot')
      await backend('save_config', {
        config: {
          ...now.config,
          preferredLanguage: before.config.preferredLanguage,
        },
      })
    }
  })
}
