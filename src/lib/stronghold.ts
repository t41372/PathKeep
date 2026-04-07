import { isTauri } from '@tauri-apps/api/core'
import { Stronghold } from '@tauri-apps/plugin-stronghold'

const CLIENT_NAME = 'pathkeep'
const STORE_KEY = 'database-key'
const LOCAL_STORAGE_KEY = 'pathkeep.database-key'

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
