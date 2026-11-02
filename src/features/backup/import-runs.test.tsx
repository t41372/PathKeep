/**
 * The Takeout and browser-file import runs, against a backend and a page
 * refresh that answer when the test says so.
 *
 * A unit test because the bug is a race the E2E cannot set up on demand
 * (`quality-matrix.md` §什麼時候才寫 unit test): the pages that read the
 * archive are still refreshing after an import when the next file is
 * inspected. Only the backend client, the refresh and the toasts are
 * replaced; the runs are the real hooks.
 *
 * Ways it can fail:
 * - the next file is inspected while the refresh after an import is still
 *   running; the run still counts itself busy and drops the request, and
 *   the card shows the path with nothing happening;
 * - the refresh after an import that succeeded fails; the run reports the
 *   import as failed and goes back to the start, though its visits are in
 *   the archive.
 */
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { toast } from 'sonner'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { I18nContext, type I18nValue } from '@/lib/i18n/context'
import { useBrowserImportRun } from './use-browser-import'
import { useTakeoutImportRun } from './use-takeout-import'

const page = vi.hoisted(() => ({
  refresh: (): Promise<void> => Promise.resolve(),
  inspected: [] as string[],
}))

vi.mock('@/app/backup-runner', () => ({
  refreshAfterArchiveChange: () => page.refresh(),
}))
vi.mock('@/lib/ipc/import-progress', () => ({
  subscribeToImportProgress: () => Promise.resolve(() => {}),
}))
vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }))
vi.mock('@/lib/backend-client/import', () => {
  const inspection = {
    candidateItems: 2,
    duplicateItems: 0,
    importedItems: 2,
    stopped: false,
    recognizedFiles: [],
    previewEntries: [],
  }
  const inspect = ({ sourcePath }: { sourcePath: string }) => {
    page.inspected.push(sourcePath)
    return Promise.resolve(inspection)
  }
  return {
    importClient: {
      inspectTakeout: inspect,
      inspectBrowserHistory: inspect,
      importTakeout: () => Promise.resolve(inspection),
      importBrowserHistory: () => Promise.resolve(inspection),
    },
  }
})

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

const i18n = {
  t: (key: string) => key,
  language: 'en',
  locale: 'en',
} as unknown as I18nValue

/** The two runs, seen through what this test does with them. */
const runs = [
  {
    name: 'a browser history file',
    use: () => {
      const run = useBrowserImportRun()
      return { ...run, inspect: (path: string) => run.inspect(path, 'detect') }
    },
  },
  {
    name: 'a Takeout export',
    use: () => {
      const run = useTakeoutImportRun()
      return { ...run, inspect: (path: string) => run.inspect(path) }
    },
  },
]

const roots: (() => void)[] = []

beforeEach(() => {
  page.refresh = () => Promise.resolve()
  page.inspected = []
  vi.clearAllMocks()
})

afterEach(() => {
  for (const unmount of roots.splice(0)) unmount()
})

/** Renders a run; the returned function reads it as last rendered. */
function mount<Run>(use: () => Run) {
  let latest: Run | undefined
  function Harness() {
    latest = use()
    return null
  }
  const root = createRoot(document.createElement('div'))
  act(() =>
    root.render(
      <I18nContext.Provider value={i18n}>
        <Harness />
      </I18nContext.Provider>,
    ),
  )
  roots.push(() => act(() => root.unmount()))
  return () => latest!
}

/** Lets promises and React's renders run. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0))
  })
}

describe.each(runs)('an import of $name', ({ use }) => {
  it('inspects the next file while the pages are still refreshing after an import', async () => {
    page.refresh = () => new Promise<void>(() => {})
    const run = mount(use)
    await act(() => run().inspect('/first'))
    void run().confirm()
    await settle()
    expect(run().state.step).toBe('done')
    act(() => run().reset())
    void run().inspect('/second')
    await settle()
    expect(page.inspected).toEqual(['/first', '/second'])
    expect(run().state.step).toBe('preview')
  })

  it('stays done when the refresh after a finished import fails', async () => {
    page.refresh = () => Promise.reject(new Error('archive locked'))
    const run = mount(use)
    await act(() => run().inspect('/first'))
    void run().confirm()
    await settle()
    expect(run().state.step).toBe('done')
    expect(toast.error).not.toHaveBeenCalled()
  })
})
