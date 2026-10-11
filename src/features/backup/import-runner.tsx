/**
 * The Takeout and browser-file imports, one run each for the whole app.
 * Mounted above the router (`src/app/index.tsx`), so leaving Backup during a
 * long import keeps its progress and its Stop, and coming back shows it
 * again; in the Backup page's state it started over as idle while the import
 * ran on (`archive.md`: switching pages must not lose an import's progress).
 */
import { createContext, useContext, useMemo, type ReactNode } from 'react'
import { useBrowserImportRun } from './use-browser-import'
import { useTakeoutImportRun } from './use-takeout-import'

interface ImportRunnerValue {
  takeout: ReturnType<typeof useTakeoutImportRun>
  browser: ReturnType<typeof useBrowserImportRun>
}

const ImportRunnerContext = createContext<ImportRunnerValue | null>(null)

export function ImportRunnerProvider({ children }: { children: ReactNode }) {
  const takeout = useTakeoutImportRun()
  const browser = useBrowserImportRun()
  const value = useMemo(() => ({ takeout, browser }), [takeout, browser])
  return (
    <ImportRunnerContext.Provider value={value}>
      {children}
    </ImportRunnerContext.Provider>
  )
}

function useImportRunner() {
  const value = useContext(ImportRunnerContext)
  if (!value)
    throw new Error('Import runs must be used inside ImportRunnerProvider')
  return value
}

export function useTakeoutImport() {
  return useImportRunner().takeout
}

export function useBrowserImport() {
  return useImportRunner().browser
}
