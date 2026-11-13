/**
 * Design prototype for v0.6.0 (agent history and more data types).
 * Runs the app's real components, tokens and charts over invented data.
 * Not part of the app build; see README.md for how to run it.
 */
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const here = path.dirname(fileURLToPath(import.meta.url))
const repo = path.resolve(here, '../../..')

export default defineConfig({
  root: here,
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.join(repo, 'src'),
      '~': path.join(here, 'src'),
    },
  },
  server: {
    host: '127.0.0.1',
    port: 1431,
    fs: { allow: [repo] },
  },
})
