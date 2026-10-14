/**
 * Settings → AI → What other apps read, against PathKeep's real MCP server:
 * the command Settings shows, started as an outside app would start it,
 * spoken to over stdio (`support/mcp-client.ts`).
 *
 * Proves: with MCP on, each request an app makes is listed newest first
 * with what it asked and how many pages matched (the number the app was
 * given); a request limited to a site says so; and turning MCP off keeps
 * the list, since it records what already left PathKeep.
 */
import { expect, test } from '@playwright/test'
import { unlockIfLocked } from './support/app'
import { backend } from './support/fixture'
import { startMcpClient } from './support/mcp-client'

const QUERY = 'tokio runtime'

test('every request an outside app makes through MCP is listed in Settings', async ({
  page,
}, testInfo) => {
  await page.goto('/#/settings/ai')
  await unlockIfLocked(page)
  const mcp = page.getByRole('switch', { name: 'MCP server' })
  if (!(await mcp.isChecked())) await mcp.click()
  await expect(mcp).toBeChecked()

  const { mcpCommand } = await backend<{ mcpCommand: string }>(
    'preview_ai_integrations',
  )
  const client = await startMcpClient(mcpCommand)
  let total: number
  let onSite: number
  try {
    const status = await client.call('archive-status', {})
    expect(status.unlocked).toBe(true)
    total = (await client.call('search-history', { query: QUERY })).total
    onSite = (
      await client.call('search-history', {
        query: QUERY,
        domain: 'docs.rs',
      })
    ).total
  } finally {
    await client.close()
  }
  await testInfo.attach('totals', {
    body: JSON.stringify({ total, onSite }),
    contentType: 'application/json',
  })

  await page.reload()
  const log = page.getByRole('list', {
    name: 'Requests through the MCP server',
  })
  const rows = log.getByRole('listitem')
  await expect(rows.first()).toBeVisible({ timeout: 30_000 })
  const results = (count: number) => `${count} result${count === 1 ? '' : 's'}`
  // Newest first: the site-limited search, the search, the status check.
  await expect(rows.nth(0)).toContainText(`Searched your history · “${QUERY}”`)
  await expect(rows.nth(0)).toContainText(results(onSite))
  await expect(rows.nth(0)).toContainText('on docs.rs')
  await expect(rows.nth(1)).toContainText(`Searched your history · “${QUERY}”`)
  await expect(rows.nth(1)).toContainText(results(total))
  await expect(rows.nth(1)).not.toContainText('on docs.rs')
  await expect(rows.nth(2)).toContainText('Checked the archive’s status')

  // Off again: what was read stays listed.
  await mcp.click()
  await expect(mcp).not.toBeChecked()
  await page.reload()
  await expect(rows.nth(1)).toContainText(results(total))
})
