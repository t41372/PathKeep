/**
 * This module implements the Vite Env.d front-end surface.
 *
 * Why this file exists:
 * - It is part of the active `src/` tree and should explain its own role without forcing the next reader to scan unrelated files first.
 * - When this file changes, the surrounding comments should keep the intent, boundaries, and main declarations easy to see at a glance.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep the implementation aligned with the accepted product, design, and architecture documents.
 * - Prefer explicit structure over cleverness so the codebase stays navigable as the front-end keeps growing.
 */

/// <reference types="vite/client" />

/**
 * Defines the typed shape for import meta env.
 *
 * Keeping this declaration named and documented is part of making the front-end codebase navigable without a separate documentation site.
 */
interface ImportMetaEnv {
  readonly VITE_PATHKEEP_DEV_IPC_URL?: string
}
