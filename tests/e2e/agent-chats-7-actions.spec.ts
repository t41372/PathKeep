/**
 * The reader's actions that leave PathKeep (`agent-history.md` §4.2, §4.3):
 * Export ▸, Show in Finder / File Explorer / file manager, and Continue.
 *
 * Proves:
 * - the long conversation (112 turns, about 2 MB) exported as Markdown and
 *   as HTML: each file is where the app said, as big as the backend said,
 *   starts with the conversation's title, and holds its first, middle and
 *   last turn; the HTML has one section per row the reader shows (as the
 *   backend counts them), opens nothing from the network and uses the
 *   window's language; JSON Lines is one event per line;
 * - Stop on a running export: the notice offers it while the file is
 *   written; the backend takes it, the export ends as stopped, the notice
 *   says nothing was written and a file already there is as it was, and
 *   the save folder holds exactly what it held before (that file, byte for
 *   byte, and nothing half written);
 * - the original files of a conversation with three subagents: the tar
 *   holds the conversation's transcript and every subagent's, byte for
 *   byte what the tool wrote, under the tool's name;
 * - the result's "Show in …" and the menu's "Show in …" ask the system to
 *   select the file itself (`reveal_file` with that path), not to open a
 *   folder; the OS side can't be watched here, so the call is answered at
 *   the transport;
 * - Continue on a Claude Code conversation recorded by a CLI release whose
 *   resume command was checked shows and copies exactly `cd <folder> &&
 *   CLAUDE_CONFIG_DIR=<root> claude --resume <session>`; a Cowork task (the
 *   Claude desktop app, not the CLI) and a Codex conversation started in
 *   the desktop app only copy their session ID.
 *
 * In the agent-chats project, on the conversations the projects before it
 * backed up. Exports go to PATHKEEP_TEST_SAVE_DIR under the name the save
 * dialog suggests (`conversation.<ext>`); each test deletes the file first.
 * The backend holds each export two seconds before it puts the file in
 * place (`PATHKEEP_TEST_AGENT_EXPORT_DELAY_US` in playwright.config.ts), so
 * Stop reaches the export while it can still stop.
 */
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { expect, test, type Page } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { TOOL_NAMES } from './support/agent-tools'
import {
  agentChat,
  exportConversation as exportAs,
  openChat,
  reader,
  savedChat,
  savedDir,
  toast,
  wireHeader,
  wireList,
} from './support/agent-chats'
import { backend, fixture } from './support/fixture'

test.use({ permissions: ['clipboard-read', 'clipboard-write'] })

const REVEAL = /^Show in (Finder|File Explorer|file manager)$/

/**
 * Answers `reveal_file` at the transport (the OS's file manager can't be
 * watched from here) and collects the paths it was asked to select.
 */
async function watchReveals(page: Page) {
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const asked: string[] = []
  const folders: string[] = []
  await page.route(`${devIpcUrl}/commands/reveal_file`, async (route) => {
    const { path: file } = route.request().postDataJSON() as { path: string }
    asked.push(file)
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(file),
    })
  })
  await page.route(
    `${devIpcUrl}/commands/open_path_in_file_manager`,
    async (route) => {
      const { path: folder } = route.request().postDataJSON() as {
        path: string
      }
      folders.push(folder)
      await route.fulfill({ status: 200, body: '""' })
    },
  )
  return { asked, folders }
}

/** The members of a tar archive (ustar and GNU long names), by name. */
function tarMembers(bytes: Buffer) {
  const members = new Map<string, Buffer>()
  let longName: string | null = null
  for (let at = 0; at + 512 <= bytes.length;) {
    const header = bytes.subarray(at, at + 512)
    if (header.every((byte) => byte === 0)) break
    const field = (from: number, length: number) =>
      header
        .subarray(from, from + length)
        .toString('utf8')
        .replace(/\0.*$/s, '')
    const size = parseInt(field(124, 12).trim() || '0', 8)
    const type = field(156, 1)
    const body = bytes.subarray(at + 512, at + 512 + size)
    at += 512 + Math.ceil(size / 512) * 512
    if (type === 'L') {
      longName = body.toString('utf8').replace(/\0.*$/s, '')
      continue
    }
    const prefix = field(345, 155)
    const name =
      longName ?? (prefix ? `${prefix}/${field(0, 100)}` : field(0, 100))
    longName = null
    members.set(name, Buffer.from(body))
  }
  return members
}

test('the long conversation exported as Markdown, HTML and JSON Lines', async ({
  page,
}, testInfo) => {
  const large = fixture().agents.scenes.large
  const chat = await savedChat(agentChat(large.conversation))
  const header = await wireHeader(chat.id)
  const reveals = await watchReveals(page)
  await openChat(page, chat.id)

  const markdown = await exportAs(page, /^Markdown/, 'md')
  await expect(toast(page, 'Exported as Markdown')).toContainText(markdown.path)
  const text = readFileSync(markdown.path, 'utf8')
  expect(text.split('\n')[0]).toBe(`# ${chat.title}`)
  for (const word of [large.firstWord, large.middleWord, large.lastWord])
    expect(text, word).toContain(word)
  expect(text.indexOf(large.firstWord)).toBeLessThan(
    text.indexOf(large.lastWord),
  )

  // The result's own "Show in …" selects the file it wrote.
  await toast(page, 'Exported as Markdown')
    .getByRole('button', { name: REVEAL })
    .click()
  await expect.poll(() => reveals.asked).toEqual([markdown.path])

  const html = await exportAs(page, /^HTML/, 'html')
  await expect(toast(page, 'Exported as HTML')).toBeVisible()
  const page1 = readFileSync(html.path, 'utf8')
  expect(page1.startsWith('<!doctype html>')).toBe(true)
  expect(page1).toContain("default-src 'none'")
  expect(page1).not.toMatch(/<script|<link|src="https?:/i)
  const escaped = chat
    .title!.replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
  expect(page1).toContain(`<h1>${escaped}</h1>`)
  // One section per row the reader shows, in the window's language.
  expect((page1.match(/<section><h2>/g) ?? []).length).toBe(
    header.visibleRecords,
  )
  expect(page1).toContain('<section><h2>You</h2>')
  for (const word of [large.firstWord, large.middleWord, large.lastWord])
    expect(page1, word).toContain(word)

  const jsonl = await exportAs(page, /^JSON Lines/, 'jsonl')
  const lines = readFileSync(jsonl.path, 'utf8').split('\n').filter(Boolean)
  expect(lines.length).toBeGreaterThanOrEqual(header.visibleRecords)
  for (const line of lines)
    expect(() => JSON.parse(line) as unknown).not.toThrow()
  expect(lines.join('\n')).toContain(large.lastWord)

  await testInfo.attach('the HTML export', {
    path: html.path,
    contentType: 'text/html',
  })
  await testInfo.attach('the reader after exporting', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
})

test('Stop ends an export: nothing is written, and a file already there is as it was', async ({
  page,
}, testInfo) => {
  const large = fixture().agents.scenes.large
  const chat = await savedChat(agentChat(large.conversation))
  await openChat(page, chat.id)
  const file = path.join(savedDir(), 'conversation.html')
  const earlier = '<!doctype html><title>An earlier export</title>\n'
  writeFileSync(file, earlier)
  const folder = readdirSync(savedDir()).sort()
  const stopped = page.waitForResponse((response) =>
    response.url().endsWith('/commands/cancel_agent_export'),
  )
  const exported = page.waitForResponse((response) =>
    response.url().endsWith('/commands/export_agent_conversation'),
  )
  await reader(page).getByRole('button', { name: 'More', exact: true }).click()
  await page.getByRole('menuitem', { name: 'Export', exact: true }).click()
  await page.getByRole('menuitem', { name: /^HTML/ }).click()
  await toast(page, 'Exporting as HTML')
    .getByRole('button', { name: 'Stop', exact: true })
    .click()
  const answer = (await (await stopped).json()) as { cancelled: boolean }
  expect(answer.cancelled, 'the backend took the Stop').toBe(true)
  const response = await exported
  expect(response.ok()).toBe(false)
  const body = (await response.json()) as { code?: string } | null
  expect(body?.code).toBe('cancelled')
  await expect(toast(page, 'Export stopped')).toContainText(
    'Nothing was written. A file that was already there is as it was.',
  )
  expect(readFileSync(file, 'utf8')).toBe(earlier)
  expect(readdirSync(savedDir()).sort()).toEqual(folder)
  await testInfo.attach('the export stopped', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
})

test('the original files: the transcript and every subagent’s, as the tool wrote them', async ({
  page,
}, testInfo) => {
  const conversation = agentChat(fixture().agents.scenes.subagents.conversation)
  expect(conversation.subagents.length).toBe(3)
  expect(conversation.appendedTurns, 'its files are as first saved').toBe(0)
  const chat = await savedChat(conversation)
  await openChat(page, chat.id)

  const tar = await exportAs(page, /^Original files/, 'tar')
  await expect(toast(page, 'Exported as Original files')).toBeVisible()
  const members = tarMembers(readFileSync(tar.path))
  const root = fixture().agents.roots['claude-code']
  const relative = (file: string) =>
    path.relative(root, file).split(path.sep).join('/')
  // Each member is `claude-code/<the folder's digest>/<path in the folder>`.
  const inFolder = (member: string) => member.split('/').slice(2).join('/')
  for (const name of members.keys()) {
    expect(name.startsWith('claude-code/'), name).toBe(true)
    const file = path.join(root, ...inFolder(name).split('/'))
    expect(
      members.get(name)!.equals(readFileSync(file)),
      `${name}: the bytes the tool wrote`,
    ).toBe(true)
  }
  const names = [...members.keys()].map(inFolder)
  for (const file of [
    ...conversation.files,
    ...conversation.subagents.map((subagent) => subagent.file),
  ])
    expect(names, relative(file)).toContain(relative(file))
  await testInfo.attach('tar members', {
    body: [...members.keys()].join('\n'),
    contentType: 'text/plain',
  })
})

test('Show in … selects the tool’s file', async ({ page }) => {
  const conversation = agentChat(fixture().agents.scenes.subagents.conversation)
  const chat = await savedChat(conversation)
  const info = await backend<{ sourcePath: string | null }>(
    'get_agent_conversation_info',
    { request: { id: chat.id } },
  )
  expect(info.sourcePath).toBe(conversation.files.at(-1))
  expect(existsSync(info.sourcePath!)).toBe(true)
  const reveals = await watchReveals(page)
  await openChat(page, chat.id)

  await reader(page).getByRole('button', { name: 'More', exact: true }).click()
  await page.getByRole('menuitem', { name: REVEAL }).click()
  await expect.poll(() => reveals.asked).toEqual([info.sourcePath])
  expect(reveals.folders, 'no folder opened instead').toEqual([])
  await expect(toast(page, 'Couldn’t show the file')).toHaveCount(0)
})

test('Continue copies the checked resume command, or else the session ID', async ({
  page,
}, testInfo) => {
  // Claude Code 2.1.322 from the command line: a checked release.
  const claude = agentChat(fixture().agents.scenes.subagents.conversation)
  const chat = await savedChat(claude)
  const header = await wireHeader(chat.id)
  const root = fixture().agents.roots['claude-code']
  const quote = (text: string) => `'${text.replace(/'/g, `'"'"'`)}'`
  if (process.platform !== 'win32')
    expect(header.resumeCommand).toBe(
      `cd ${quote(claude.projectFolder)} && CLAUDE_CONFIG_DIR=${quote(root)} claude --resume ${claude.nativeId}`,
    )
  await openChat(page, chat.id)
  await reader(page).getByRole('button', { name: 'Continue' }).click()
  let popover = page.getByRole('dialog')
  await expect(popover).toContainText(
    `Continue this conversation in a terminal with ${TOOL_NAMES['claude-code']}:`,
  )
  await expect(popover.locator('pre')).toHaveText(header.resumeCommand!)
  await expect(
    popover.getByRole('button', { name: 'Copy session ID' }),
  ).toHaveCount(0)
  await testInfo.attach('Continue with a checked command', {
    body: await page.screenshot(),
    contentType: 'image/png',
  })
  await popover.getByRole('button', { name: 'Copy command' }).click()
  await expect(
    toast(page, 'Command copied. Paste it in a terminal to continue.'),
  ).toBeVisible()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    header.resumeCommand,
  )
  await page.keyboard.press('Escape')

  // A Cowork task (the Claude desktop app) and a Codex conversation whose
  // release or origin wasn't checked (the fixture starts some in the Codex
  // desktop app): the session ID only.
  const cowork = agentChat(
    fixture().agents.scenes.claudeDesktop.cowork.conversation,
  )
  const codexDesktop = await (async () => {
    for (const row of await wireList({ harnesses: ['codex'] })) {
      const found = await wireHeader(row.id)
      if (
        !found.summary.onlyInPathKeep &&
        found.resumeCommand === null &&
        found.nativeSessionId
      )
        return row
    }
    return null
  })()
  expect(codexDesktop, 'a Codex conversation from the desktop app').not.toBe(
    null,
  )
  for (const other of [await savedChat(cowork), codexDesktop!]) {
    const otherHeader = await wireHeader(other.id)
    expect(otherHeader.resumeCommand).toBe(null)
    await openChat(page, other.id)
    await reader(page).getByRole('button', { name: 'Continue' }).click()
    popover = page.getByRole('dialog')
    await expect(popover.locator('pre')).toHaveCount(0)
    await popover.getByRole('button', { name: 'Copy session ID' }).click()
    await expect(toast(page, 'Session ID copied')).toBeVisible()
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
      otherHeader.nativeSessionId,
    )
    await page.keyboard.press('Escape')
  }
})
