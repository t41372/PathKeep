import { useEffect } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend, backendTestHarness } from '../lib/backend'
import { I18nContext, type I18nContextValue } from '../lib/i18n/context'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../lib/i18n'
import type { AppConfig, AppSnapshot } from '../lib/types'
import { ShellDataProvider } from './shell-data'
import { useShellData } from './shell-data-context'

const baseConfig: AppConfig = {
  initialized: false,
  archiveMode: 'Encrypted',
  preferredLanguage: 'system',
  dueAfterHours: 72,
  scheduleCheckIntervalHours: 6,
  checkpointDays: 90,
  captureFavicons: true,
  selectedProfileIds: ['chrome:Default'],
  gitEnabled: true,
  rememberDatabaseKeyInKeyring: false,
  appAutostart: false,
  remoteBackup: {
    enabled: false,
    bucket: '',
    region: 'us-east-1',
    endpoint: null,
    prefix: 'pathkeep',
    pathStyle: true,
    uploadAfterBackup: false,
    credentialsSaved: false,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    lastError: null,
  },
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt: 'Evidence only.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

function createI18nValue(
  language: ResolvedLanguage,
  setLanguagePreference = vi.fn(),
): I18nContextValue {
  return {
    language,
    preference: language,
    setLanguagePreference,
    t: createTranslator(language),
    ns: (namespace) => createNamespaceTranslator(language, namespace),
  }
}

async function seedSnapshot() {
  await backend.initializeArchive(baseConfig, 'vault-passphrase')
  const snapshot = structuredClone(await backend.getAppSnapshot())
  const dashboard = structuredClone(await backend.loadDashboardSnapshot())
  return { dashboard, snapshot }
}

function ShellProbe({ onReady }: { onReady?: () => void }) {
  const shell = useShellData()

  useEffect(() => {
    if (!shell.loading) {
      onReady?.()
    }
  }, [onReady, shell.loading])

  return (
    <div>
      <div data-testid="loading">{String(shell.loading)}</div>
      <div data-testid="refresh-key">{shell.refreshKey}</div>
      <div data-testid="notice">{shell.notice ?? 'none'}</div>
      <div data-testid="error">{shell.error ?? 'none'}</div>
      <div data-testid="snapshot-language">
        {shell.snapshot?.config.preferredLanguage ?? 'none'}
      </div>
      <button
        type="button"
        onClick={() => {
          void shell.refreshAppData().catch(() => undefined)
        }}
      >
        refresh
      </button>
      <button
        type="button"
        onClick={() =>
          void shell
            .saveConfig({
              ...(shell.snapshot?.config ?? baseConfig),
              preferredLanguage: 'zh-CN',
            })
            .catch(() => undefined)
        }
      >
        save
      </button>
      <button
        type="button"
        onClick={() =>
          void shell
            .initializeArchive({
              ...baseConfig,
              preferredLanguage: 'zh-TW',
            })
            .catch(() => undefined)
        }
      >
        initialize
      </button>
      <button
        type="button"
        onClick={() => {
          void shell.runBackup().catch(() => undefined)
        }}
      >
        backup
      </button>
      <button type="button" onClick={() => shell.clearNotice()}>
        clear
      </button>
    </div>
  )
}

describe('ShellDataProvider', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    backendTestHarness.reset()
  })

  test('loads and mutates shell data through provider actions', async () => {
    const user = userEvent.setup()
    const languageSpy = vi.fn()
    const { dashboard, snapshot } = await seedSnapshot()
    const savedSnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, preferredLanguage: 'zh-CN' },
    }
    const initializedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        initialized: true,
        preferredLanguage: 'zh-TW',
      },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'saveConfig').mockResolvedValue(savedSnapshot)
    vi.spyOn(backend, 'initializeArchive').mockResolvedValue(
      initializedSnapshot,
    )
    vi.spyOn(backend, 'runBackupNow').mockResolvedValue({
      dueSkipped: false,
      run: {
        id: 42,
        startedAt: '2026-04-07T00:00:00Z',
        finishedAt: '2026-04-07T00:05:00Z',
        status: 'success',
        manifestHash: 'manifest-42',
        profileScope: ['chrome:Default'],
        profilesProcessed: 1,
        newVisits: 2,
        newUrls: 1,
        newDownloads: 0,
        runType: 'backup',
      },
      profiles: [],
      warnings: [],
      remoteBackup: null,
    })

    render(
      <I18nContext.Provider value={createI18nValue('en', languageSpy)}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(languageSpy).toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).toHaveTextContent(
        'zh-CN',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).not.toHaveTextContent('none'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(/run #42/i),
    )

    await user.click(screen.getByRole('button', { name: 'clear' }))
    expect(screen.getByTestId('notice')).toHaveTextContent('none')

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('refresh-key')).not.toHaveTextContent('0'),
    )
  })

  test('surfaces provider errors and context misuse clearly', async () => {
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined)
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'saveConfig').mockRejectedValue(new Error('save failed'))

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('save failed'),
    )

    expect(() => render(<ShellProbe />)).toThrow(
      'useShellData must be used inside ShellDataProvider',
    )

    consoleError.mockRestore()
  })
})
