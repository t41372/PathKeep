/**
 * This module contains the front-end helpers that speak to the Stronghold-backed archive-key flows.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `storeDatabaseKeyStronghold`
 * - `readDatabaseKeyStronghold`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { isTauri } from '@tauri-apps/api/core'
import { Stronghold } from '@tauri-apps/plugin-stronghold'

const CLIENT_NAME = 'pathkeep'
const STORE_KEY = 'database-key'
const LOCAL_STORAGE_KEY = 'pathkeep.database-key'

/**
 * Loads stronghold.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
async function loadStronghold(password: string, strongholdPath: string) {
  const stronghold = await Stronghold.load(strongholdPath, password)

  let client
  try {
    client = await stronghold.loadClient(CLIENT_NAME)
  } catch {
    client = await stronghold.createClient(CLIENT_NAME)
  }

  return {
    stronghold,
    store: client.getStore(),
  }
}

/**
 * Explains how store database key stronghold works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function storeDatabaseKeyStronghold(
  password: string,
  databaseKey: string,
  strongholdPath: string,
) {
  if (!isTauri()) {
    window.localStorage.setItem(LOCAL_STORAGE_KEY, databaseKey)
    return
  }

  const { stronghold, store } = await loadStronghold(password, strongholdPath)
  await store.insert(
    STORE_KEY,
    Array.from(new TextEncoder().encode(databaseKey)),
  )
  await stronghold.save()
  await stronghold.unload()
}

/**
 * Reads database key stronghold from the current runtime.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function readDatabaseKeyStronghold(
  password: string,
  strongholdPath: string,
) {
  if (!isTauri()) {
    return window.localStorage.getItem(LOCAL_STORAGE_KEY)
  }

  const { stronghold, store } = await loadStronghold(password, strongholdPath)
  const value = await store.get(STORE_KEY)
  await stronghold.unload()

  if (!value) {
    return null
  }

  return new TextDecoder().decode(value)
}
