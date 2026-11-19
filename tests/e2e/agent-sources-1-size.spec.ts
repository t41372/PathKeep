/**
 * The archive's size on the Agent backup page comes after the page
 * (`data-spaces.md` §4, the footer): counting it reads the archive files,
 * so the page is drawn first and the size joins the footer's sentence
 * without moving it.
 *
 * The size's answer (`list_data_kinds` with `includeArchiveBytes`) is held
 * at the bridge until the page has been looked at, so the wait can be
 * seen; the answer itself is the real backend's. Needs the state the specs
 * before leave: Agent conversations on, with saved conversations.
 *
 * Proves:
 * - the overview the page asks for first carries no size;
 * - the page, its sources and its footer are drawn while the size is
 *   still being counted; the footer says so to a screen reader and holds
 *   a placeholder in its sentence;
 * - once the answer comes, the sentence shows the backend's size and the
 *   source's, and the footer line keeps its place and its height.
 */
import { expect, test, type Route } from '@playwright/test'
import { resolveDesktopBridgeEnv } from '../../scripts/pathkeep-dev-desktop-bridge.mjs'
import { openBackupPage } from './support/agent-backup'
import { formatBytes } from './support/agent-tools'
import { backend } from './support/fixture'

interface KindRow {
  kind: string
  archiveBytes?: string
}

test('the size in the archive fills in after the page, without moving it', async ({
  page,
}, testInfo) => {
  const overview = await backend<Record<string, unknown>>(
    'get_agent_overview',
    { request: { purpose: 'backup' } },
  )
  expect(overview).not.toHaveProperty('archiveBytes')

  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const pattern = `${devIpcUrl}/commands/list_data_kinds`
  const held: Route[] = []
  await page.route(pattern, (route) => {
    const body = route.request().postDataJSON() as {
      request?: { includeArchiveBytes?: boolean }
    } | null
    if (body?.request?.includeArchiveBytes) held.push(route)
    else void route.continue()
  })

  try {
    await openBackupPage(page)
    await expect(
      page.getByRole('region', { name: 'Sources' }).locator('li[data-tool]'),
    ).not.toHaveCount(0)
    await expect.poll(() => held.length).toBeGreaterThan(0)
    const line = page.locator('[data-archive-size]')
    await expect(line).toHaveAttribute('data-archive-size', 'counting')
    await expect(line).toContainText('Agent conversations take about')
    await expect(line).toContainText('Counting the size in the archive…')
    const before = (await line.boundingBox())!
    await testInfo.attach('the page while the size is counted', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })

    await page.unroute(pattern)
    for (const route of held) await route.continue()

    const sizes = await backend<{ kinds: KindRow[] }>('list_data_kinds', {
      request: { includeArchiveBytes: true },
    })
    const archive = Number(
      sizes.kinds.find((row) => row.kind === 'agent-history')?.archiveBytes,
    )
    expect(archive).toBeGreaterThan(0)
    const source = Number(overview.sourceBytes)
    await expect(line).toHaveAttribute('data-archive-size', 'counted')
    await expect(line).toHaveText(
      `Agent conversations take about ${formatBytes(archive)} in the archive (${formatBytes(source)} at the source).`,
    )
    const after = (await line.boundingBox())!
    expect(after.y).toBeCloseTo(before.y, 0)
    expect(after.height).toBeCloseTo(before.height, 0)
    await testInfo.attach('the page with the size', {
      body: await page.screenshot({ fullPage: true }),
      contentType: 'image/png',
    })
  } finally {
    await page.unroute(pattern).catch(() => undefined)
  }
})
