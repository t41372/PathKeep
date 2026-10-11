/**
 * A website on this computer for `link-preview.spec`: HTTPS on 127.0.0.1,
 * with a certificate from the test authority in `tls/`. The E2E backend
 * trusts that authority (`PATHKEEP_TEST_PREVIEW_SERVER_CA`, debug builds
 * only), so link previews are downloaded from here for real, through the same
 * address checks as any site. Every path asked for is recorded.
 */
import { readFileSync } from 'node:fs'
import { createServer } from 'node:https'
import type { AddressInfo } from 'node:net'
import path from 'node:path'

/** Relative to the repository, where Playwright runs; the config names `ca.pem` to the backend. */
const TLS = path.resolve('tests/e2e/support/tls')

/** The visits `link-preview.spec` adds to Chrome's history, for `wipe.spec` to count. */
export const PREVIEW_VISIT_COUNT = 2

export interface Response {
  status?: number
  headers?: Record<string, string>
  body?: string | Buffer
}

export interface PreviewServer {
  origin: string
  /** Paths requested so far, in order. */
  requests: string[]
  close: () => Promise<void>
}

/** Serves `routes` (path → response, given the server's origin); anything else is a 404. */
export async function startPreviewServer(
  routes: Record<string, (origin: string) => Response>,
): Promise<PreviewServer> {
  const requests: string[] = []
  let origin = ''
  const server = createServer(
    {
      cert: readFileSync(path.join(TLS, 'server.pem')),
      key: readFileSync(path.join(TLS, 'server-key.pem')),
    },
    (request, response) => {
      const at = request.url ?? '/'
      requests.push(at)
      const route = routes[at]
      if (!route) {
        response.writeHead(404).end()
        return
      }
      const { status = 200, headers = {}, body = '' } = route(origin)
      response.writeHead(status, headers).end(body)
    },
  )
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  origin = `https://127.0.0.1:${(server.address() as AddressInfo).port}`
  return {
    origin,
    requests,
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections()
        server.close(() => resolve())
      }),
  }
}
