/**
 * Opening links: PathKeep opens a file on this computer only if it is a page
 * in the user's history.
 *
 * Proves: the plain launcher refuses every file:// URL, whatever it names; the
 * page opener, which History, a page's screen and links in Ask answers use,
 * refuses one that is not in the archive with the code the screen turns into
 * a sentence, however the scheme is spelled. A file:// URL can launch an
 * application, and a link in an Ask answer is written by a model that may
 * have read a hostile page. Opening an allowed page would start a browser on
 * the test machine, so the test stops at the refusals.
 */
import { expect, test } from '@playwright/test'
import { backend } from './support/fixture'

const application = 'file:///Applications/Calculator.app'

test('a local file opens only if it is a page in the history', async () => {
  await expect(
    backend('open_external_url', { url: application }),
  ).rejects.toThrow(/Only http/)
  for (const url of [
    application,
    '  FILE:///etc/passwd',
    'File://localhost/etc/hosts',
  ]) {
    await expect(backend('open_visited_page', { url })).rejects.toThrow(
      /^open_visited_page: not-in-history: /,
    )
  }
})
