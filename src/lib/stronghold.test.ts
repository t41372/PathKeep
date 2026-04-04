import { beforeEach, describe, expect, test, vi } from 'vitest'

const { isTauri, strongholdLoad } = vi.hoisted(() => ({
  isTauri: vi.fn(() => false),
  strongholdLoad: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  isTauri,
}))

vi.mock('@tauri-apps/plugin-stronghold', () => ({
  Stronghold: {
    load: strongholdLoad,
  },
}))

import {
  readDatabaseKeyStronghold,
  storeDatabaseKeyStronghold,
} from './stronghold'

describe('stronghold helpers', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    strongholdLoad.mockReset()
  })

  test('falls back to localStorage outside the Tauri shell', async () => {
    await storeDatabaseKeyStronghold('pw', 'db-key', '/tmp/vault.hold')
    await expect(
      readDatabaseKeyStronghold('pw', '/tmp/vault.hold'),
    ).resolves.toBe('db-key')
  })

  test('stores and loads keys through Stronghold in Tauri mode', async () => {
    const insert = vi.fn().mockResolvedValue(undefined)
    const get = vi.fn().mockResolvedValue(new TextEncoder().encode('tauri-key'))
    const unload = vi.fn().mockResolvedValue(undefined)
    const save = vi.fn().mockResolvedValue(undefined)
    const loadClient = vi.fn().mockResolvedValue({
      getStore: () => ({
        insert,
        get,
      }),
    })

    strongholdLoad.mockResolvedValue({
      loadClient,
      createClient: vi.fn(),
      save,
      unload,
    })
    isTauri.mockReturnValue(true)

    await storeDatabaseKeyStronghold('pw', 'tauri-key', '/tmp/vault.hold')
    await expect(
      readDatabaseKeyStronghold('pw', '/tmp/vault.hold'),
    ).resolves.toBe('tauri-key')
    expect(loadClient).toHaveBeenCalledWith('browser-history-backup')
    expect(insert).toHaveBeenCalledOnce()
    expect(save).toHaveBeenCalledOnce()
    expect(unload).toHaveBeenCalledTimes(2)
  })

  test('creates a missing Stronghold client and returns null when the key is absent', async () => {
    const get = vi.fn().mockResolvedValue(null)
    const createClient = vi.fn().mockResolvedValue({
      getStore: () => ({
        insert: vi.fn(),
        get,
      }),
    })

    strongholdLoad.mockResolvedValue({
      loadClient: vi.fn().mockRejectedValue(new Error('missing')),
      createClient,
      save: vi.fn(),
      unload: vi.fn().mockResolvedValue(undefined),
    })
    isTauri.mockReturnValue(true)

    await expect(
      readDatabaseKeyStronghold('pw', '/tmp/vault.hold'),
    ).resolves.toBeNull()
    expect(createClient).toHaveBeenCalledWith('browser-history-backup')
  })
})
