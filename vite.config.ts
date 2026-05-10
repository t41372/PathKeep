import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const devServerPort = Number(process.env.PATHKEEP_DEV_SERVER_PORT || 1420)
const browserPreviewDataset =
  process.env.PATHKEEP_BROWSER_PREVIEW_DATASET ||
  process.env.VITE_PATHKEEP_BROWSER_PREVIEW_DATASET ||
  (process.env.VERCEL ? 'showcase' : 'setup')

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  define: {
    __PATHKEEP_BROWSER_PREVIEW_DATASET__: JSON.stringify(browserPreviewDataset),
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
      ],
    },
  },
})
