/**
 * Follows a schema update that runs while the archive opens; the session gate
 * shows it (`SchemaUpdateScreen`).
 */
import { useEffect, useState } from 'react'
import { subscribeToArchiveUpgradeProgress } from '@/lib/ipc/archive-upgrade-progress'
import type { ArchiveUpgradeProgress } from '@/lib/types'

/**
 * The latest step of a schema update that runs while the archive opens (a
 * newer version adding a database version, `vault/upgrade.rs`), whatever
 * opened it: launch, the keychain or an unlock. `null` once it is done, or
 * while none is running.
 */
export function useSchemaUpdate() {
  const [progress, setProgress] = useState<ArchiveUpgradeProgress | null>(null)
  useEffect(() => {
    const subscription = subscribeToArchiveUpgradeProgress((event) => {
      const over =
        event.done || (event.total > 0 && event.processed >= event.total)
      // A new or already updated archive reports nothing to copy (0 of 0).
      const work = event.phase === 'schemaMigration' && event.total > 0
      setProgress(!over && work ? event : null)
    })
    return () => void subscription.then((stop) => stop?.())
  }, [])
  return progress
}
