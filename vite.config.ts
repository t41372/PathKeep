import { defineConfig } from 'vite'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

const rootDir = path.dirname(fileURLToPath(import.meta.url))
const devServerPort = Number(process.env.PATHKEEP_DEV_SERVER_PORT || 1420)

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.join(rootDir, 'src'),
    },
  },
  build: {
    manifest: true,
  },
  server: {
    host: '127.0.0.1',
    port: devServerPort,
    strictPort: true,
    watch: {
      ignored: [
        '**/src-tauri/target/**',
        '**/var/playwright/**',
        '**/cargo-target/**',
        // Agent worktrees and the design prototypes are full of HTML that
        // would otherwise trigger page reloads. Anchored to this checkout, so
        // a dev server started inside a worktree still sees its own files.
        path.join(rootDir, '.claude/**'),
        path.join(rootDir, 'docs/**'),
      ],
    },
  },
})
