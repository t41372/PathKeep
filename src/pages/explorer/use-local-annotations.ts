/**
 * localStorage-backed annotations hook for BROWSER PREVIEW only.
 *
 * The real annotations backend has shipped (`vault-core` annotations module +
 * `src-tauri/src/commands/annotations.rs`); desktop sessions use
 * `use-desktop-annotations.ts` against the canonical archive. This hook is
 * the preview-mode fallback so the Detail panel still demonstrates notes +
 * tags without a backend. Do not extend it with new contract surface.
 *
 * ## Responsibilities
 * - Keep one notes string and one tag-list per URL key, persisted under
 *   `pk.notes` / `pk.tags` in localStorage.
 * - Survive missing or corrupt localStorage payloads — return empty defaults.
 * - Provide an `updateNotes` / `updateTags` pair that writes both the local
 *   state and the persisted payload, so the next mount sees the same data.
 *
 * ## Not responsible for
 * - Cross-device sync.
 * - Desktop sessions or the canonical annotations table — preview-only data
 *   here never migrates into the archive.
 */

import { useCallback, useMemo, useState } from 'react'

const NOTES_KEY = 'pk.notes'
const TAGS_KEY = 'pk.tags'

function loadMap<TValue>(key: string): Record<string, TValue> {
  try {
    const raw = globalThis.localStorage?.getItem(key)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as unknown
    return parsed && typeof parsed === 'object'
      ? (parsed as Record<string, TValue>)
      : {}
  } catch {
    return {}
  }
}

function saveMap(key: string, value: Record<string, unknown>) {
  try {
    globalThis.localStorage?.setItem(key, JSON.stringify(value))
  } catch {
    // Quota / serialization errors are swallowed — the prototype tolerates
    // a transient persistence failure rather than blowing up the panel.
  }
}

export interface LocalAnnotations {
  notesFor(key: string | null | undefined): string
  tagsFor(key: string | null | undefined): string[]
  updateNotes(key: string, next: string): void
  updateTags(key: string, next: string[]): void
  /**
   * Last async failure from a hydration or write. The localStorage hook
   * never fails (so this is always `null`); the desktop hook populates it
   * when a backend GET/PUT rejects, so the detail panel can surface the
   * problem instead of silently pretending the edit was saved.
   */
  lastError?: string | null
}

export function useLocalAnnotations(): LocalAnnotations {
  const [notesMap, setNotesMap] = useState<Record<string, string>>(() =>
    loadMap<string>(NOTES_KEY),
  )
  const [tagsMap, setTagsMap] = useState<Record<string, string[]>>(() =>
    loadMap<string[]>(TAGS_KEY),
  )

  const notesFor = useCallback(
    (key: string | null | undefined) => (key ? (notesMap[key] ?? '') : ''),
    [notesMap],
  )
  const tagsFor = useCallback(
    (key: string | null | undefined) => (key ? (tagsMap[key] ?? []) : []),
    [tagsMap],
  )

  const updateNotes = useCallback((key: string, next: string) => {
    setNotesMap((prev) => {
      const merged = { ...prev, [key]: next }
      saveMap(NOTES_KEY, merged)
      return merged
    })
  }, [])

  const updateTags = useCallback((key: string, next: string[]) => {
    setTagsMap((prev) => {
      const merged = { ...prev, [key]: next }
      saveMap(TAGS_KEY, merged)
      return merged
    })
  }, [])

  return useMemo(
    () => ({ notesFor, tagsFor, updateNotes, updateTags, lastError: null }),
    [notesFor, tagsFor, updateNotes, updateTags],
  )
}
