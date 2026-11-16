// @ts-check
/**
 * PathKeep's public site: static pages only, built by Astro.
 * Port 4391 because 4321 is often taken by other local servers.
 */
import { defineConfig } from 'astro/config'

export default defineConfig({
  trailingSlash: 'always',
  build: { format: 'directory' },
  server: { host: '127.0.0.1', port: 4391 },
  devToolbar: { enabled: false },
})
