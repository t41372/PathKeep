import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { i18nStorageKey } from '../../lib/i18n/context'
import type {
  AiIndexStatus,
  AiProviderConfig,
  AiProviderConnectionTestReport,
  AiSettings,
  StaticEmbeddingStatus,
} from '../../lib/types'
import {
  AiProvidersSection,
  type AiProvidersSectionState,
} from './ai-providers-section'
import type { SettingsSectionNavItem } from './section-nav-items'
// Type-only import (the runtime module is mocked below) so the hook mock's return value is typed to
// the real progress contract and `mockReturnValue` accepts every phase, not just idle/ready.
import type { ModelDownloadProgress } from '../../lib/ipc/model-download'

// The Build-index CTA, reset, and static-model-download commands enqueue backend work via the
// client, so mock them here. `vi.mock` is hoisted above the imports, so the spies it
// references must be created in a hoisted block too (a plain top-level const is not yet
// initialized when the factory runs).
const {
  buildAiIndex,
  downloadStaticEmbeddingModel,
  cancelStaticEmbeddingModelDownload,
  resetAiIndexBuild,
} = vi.hoisted(() => ({
  buildAiIndex: vi.fn(() => Promise.resolve(undefined)),
  downloadStaticEmbeddingModel: vi.fn(() => Promise.resolve(undefined)),
  cancelStaticEmbeddingModelDownload: vi.fn(() => Promise.resolve(undefined)),
  resetAiIndexBuild: vi.fn(() => Promise.resolve(undefined)),
}))
vi.mock('../../lib/backend-client', () => ({
  backend: {
    buildAiIndex,
    downloadStaticEmbeddingModel,
    cancelStaticEmbeddingModelDownload,
    resetAiIndexBuild,
    // The nested GPU section (collapsed by default in these tests) reads these
    // lazily only when its disclosure is opened; stub them so the module mock is
    // complete and any future test that opens it does not hit an undefined method.
    estimateReembed: vi.fn(() =>
      Promise.resolve({
        scope: 'working-set',
        pageCount: 0,
        estMinutesCpu: 0,
        estMinutesGpu: 0,
        gpuAvailable: false,
      }),
    ),
    loadAiQueueStatus: vi.fn(() => Promise.resolve({ queued: 0, running: 0 })),
  },
}))

// The static download panel reads a process-global in-flight latch and a phase/byte progress object
// from `lib/ipc/model-download`. Mock the module so tests can (a) deterministically control + reset
// the latch and (b) drive the panel by setting the hook's RETURNED progress per scenario. The hook's
// own event/state-machine (byte accumulation, done/error/cancel mapping, subscription lifecycle) is
// covered directly in model-download.test.ts, where the REAL hook runs against a mocked Tauri event
// channel; here we only assert the panel's phase → UI mapping for a given progress object.
const {
  markModelDownloadStarted,
  markModelDownloadSettled,
  isModelDownloadInFlight,
  useModelDownloadProgress,
  resetDownloadHarness,
} = vi.hoisted(() => {
  let inFlight = false
  // Default return: mirror the real hook's authoritative `modelDownloaded → ready` contract, and
  // idle otherwise. Tests that need a live downloading / failed / ready-via-event state override
  // this with `useModelDownloadProgress.mockReturnValue(...)`.
  const defaultImpl = (modelDownloaded: boolean): ModelDownloadProgress => ({
    phase: modelDownloaded ? 'ready' : 'idle',
    downloadedBytes: 0,
    totalBytes: 0,
    currentFile: null,
    error: null,
  })
  const useModelDownloadProgress = vi.fn(defaultImpl)
  return {
    markModelDownloadStarted: vi.fn(() => {
      inFlight = true
    }),
    markModelDownloadSettled: vi.fn(() => {
      inFlight = false
    }),
    isModelDownloadInFlight: vi.fn(() => inFlight),
    useModelDownloadProgress,
    // `vi.clearAllMocks()` clears call history but NOT a mock's implementation/return value, so a
    // per-test `mockReturnValue` would leak forward. Reset the hook to its default impl (and clear
    // the latch) before every test.
    resetDownloadHarness: () => {
      inFlight = false
      useModelDownloadProgress.mockReset()
      useModelDownloadProgress.mockImplementation(defaultImpl)
    },
  }
})
vi.mock('../../lib/ipc/model-download', () => ({
  markModelDownloadStarted,
  markModelDownloadSettled,
  isModelDownloadInFlight,
  useModelDownloadProgress,
}))

const navItem: SettingsSectionNavItem = {
  id: 'settings-ai',
  icon: 'smart_toy',
  key: 'ai',
  label: 'AI providers',
}

const providerTranslations = {
  providerName: 'Provider name',
  providerId: 'Provider ID',
  requestFormat: 'Request format',
  baseUrl: 'Base URL',
  baseUrlPlaceholder: 'http://localhost:11434/v1',
  defaultModel: 'Default model',
  modelCatalog: 'Model catalog',
  modelCatalogHint: 'model-a, model-b',
  enabled: 'Provider enabled',
  temperature: 'Temperature',
  maxTokens: 'Max tokens',
  dimensions: 'Dimensions',
  notes: 'Notes',
  apiKey: 'API key',
  apiKeyPlaceholder: 'Paste API key',
  keySaved: 'saved',
  keyNotSaved: 'not saved',
  saveKey: 'Save key',
  clearKey: 'Clear key',
  remove: 'Remove',
  testConnection: 'Test connection',
  testingConnection: 'Testing…',
  probeReachable: 'Connected',
  probeUnreachable: 'Connection issue',
  requestFormatLabels: {
    openai: 'OpenAI compatible',
    anthropic: 'Anthropic',
    google: 'Google',
    ollama: 'Ollama',
    'lm-studio': 'LM Studio',
  },
}

describe('AiProvidersSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    buildAiIndex.mockResolvedValue(undefined)
    downloadStaticEmbeddingModel.mockResolvedValue(undefined)
    cancelStaticEmbeddingModelDownload.mockResolvedValue(undefined)
    // The download latch + captured listener are module-scoped (they survive a remount by
    // design), so reset them between tests to avoid cross-test leakage.
    resetDownloadHarness()
  })

  afterEach(() => {
    window.localStorage.removeItem(i18nStorageKey)
  })

  test('does not render until the route state has a current AI draft', () => {
    const { container } = renderSection({
      currentSettings: null,
    })

    expect(container.firstChild).toBeNull()
  })

  test('renders the live provider editor and wires the route-owned handlers', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
    })

    // Master AI consent toggle reflects the persisted draft and is editable.
    const master = screen.getByRole('checkbox', { name: 'Enable AI features' })
    expect(master).toBeChecked()
    fireEvent.click(master)
    expect(handlers.onToggleAi).toHaveBeenCalledTimes(1)

    // The seeded providers appear as editable fields (real AiProviderEditorList).
    expect(screen.getByDisplayValue('Local LLM')).toBeVisible()
    expect(screen.getByDisplayValue('Local Embeddings')).toBeVisible()

    // The Add-provider preset choosers route to the per-purpose handler with the
    // chosen format. LM Studio is the headline preset for the chat provider.
    const presetPickers = screen.getAllByRole('combobox', {
      name: 'Start from a preset',
    })
    fireEvent.change(presetPickers[0], { target: { value: 'lm-studio' } })
    expect(handlers.onAddProvider).toHaveBeenLastCalledWith('llm', 'lm-studio')
    fireEvent.change(presetPickers[1], { target: { value: 'ollama' } })
    expect(handlers.onAddProvider).toHaveBeenLastCalledWith(
      'embedding',
      'ollama',
    )
  })

  test('exposes every Add-provider preset and ignores the placeholder option', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
    })

    const presetPicker = screen.getAllByRole('combobox', {
      name: 'Start from a preset',
    })[0]
    // All five presets are offered, with LM Studio first (headline local path).
    const optionLabels = Array.from(
      presetPicker.querySelectorAll('option'),
    ).map((option) => option.textContent)
    expect(optionLabels).toEqual([
      'Add chat provider',
      'LM Studio',
      'Ollama',
      'OpenAI',
      'Anthropic',
      'Google',
    ])
    // Re-selecting the disabled placeholder must not fire onAddProvider.
    fireEvent.change(presetPicker, { target: { value: '' } })
    expect(handlers.onAddProvider).not.toHaveBeenCalled()
  })

  test('routes every per-provider editor callback to the right purpose', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      // A typed key so save/clear key are enabled (providers always persist now).
      // No provider is pre-selected so the radios are unchecked and clicking
      // them fires onSelect.
      aiApiKeys: { 'llm-1': '  secret  ', 'embed-1': '  embed-secret  ' },
      currentSettings: settingsFixture({
        enabled: true,
        llmProviderId: null,
        embeddingProviderId: null,
      }),
    })

    // Select (radio), update (name input), and remove for the LLM provider.
    const llmName = screen.getByDisplayValue('Local LLM')
    fireEvent.change(llmName, { target: { value: 'Edited LLM' } })
    expect(handlers.onUpdateProvider).toHaveBeenLastCalledWith('llm', 'llm-1', {
      name: 'Edited LLM',
    })
    const radios = screen.getAllByRole('radio')
    fireEvent.click(radios[0])
    expect(handlers.onSelectProvider).toHaveBeenLastCalledWith('llm', 'llm-1')
    fireEvent.click(radios[1])
    expect(handlers.onSelectProvider).toHaveBeenLastCalledWith(
      'embedding',
      'embed-1',
    )

    // Update for the embedding provider routes with the embedding purpose.
    fireEvent.change(screen.getByDisplayValue('Local Embeddings'), {
      target: { value: 'Edited Embeddings' },
    })
    expect(handlers.onUpdateProvider).toHaveBeenLastCalledWith(
      'embedding',
      'embed-1',
      { name: 'Edited Embeddings' },
    )

    // Save key + Clear key for both purposes.
    const saveButtons = screen.getAllByRole('button', { name: 'Save key' })
    const clearButtons = screen.getAllByRole('button', { name: 'Clear key' })
    saveButtons.forEach((button) => fireEvent.click(button))
    clearButtons.forEach((button) => fireEvent.click(button))
    expect(handlers.onSaveAiApiKey).toHaveBeenCalledWith('llm-1')
    expect(handlers.onSaveAiApiKey).toHaveBeenCalledWith('embed-1')
    expect(handlers.onClearAiApiKey).toHaveBeenCalledWith('llm-1')
    expect(handlers.onClearAiApiKey).toHaveBeenCalledWith('embed-1')

    // Remove for both purposes.
    const removeButtons = screen.getAllByRole('button', { name: 'Remove' })
    removeButtons.forEach((button) => fireEvent.click(button))
    expect(handlers.onRemoveProvider).toHaveBeenCalledWith('llm', 'llm-1')
    expect(handlers.onRemoveProvider).toHaveBeenCalledWith(
      'embedding',
      'embed-1',
    )

    // API key typing routes to onApiKeyChange.
    fireEvent.change(screen.getAllByPlaceholderText('Paste API key')[0], {
      target: { value: 'new-key' },
    })
    expect(handlers.onApiKeyChange).toHaveBeenCalledWith('llm-1', 'new-key')
  })

  test('always shows the AI consent disclosure', () => {
    renderSection({ currentSettings: settingsFixture({ enabled: false }) })

    const disclosure = screen.getByTestId('ai-consent-disclosure')
    expect(within(disclosure).getByText('How AI uses your data')).toBeVisible()
    // The disclosure lists no-provider, egress, and local-only bullets.
    expect(
      within(disclosure).getByText(/PathKeep ships no AI provider/),
    ).toBeVisible()
    expect(
      within(disclosure).getByText(/chat transcripts are excluded from export/),
    ).toBeVisible()
    // W-AI-8 WU-3: the disclosure also states the (default-enabled) sandboxed code-mode plainly —
    // read-only, no network/file access, bounded, and the exact code + queries are always shown.
    const codeBullet = within(disclosure).getByText(
      /write and run a small program over your history/,
    )
    expect(codeBullet).toBeVisible()
    expect(codeBullet).toHaveTextContent(/sandboxed and read-only/)
    expect(codeBullet).toHaveTextContent(
      /cannot reach the network or your files/,
    )
    expect(codeBullet).toHaveTextContent(
      /shows the exact code and the queries it ran/,
    )
  })

  test('keeps the provider editors visible but inert while AI is off', () => {
    renderSection({ currentSettings: settingsFixture({ enabled: false }) })

    // Editors are still shown (the user sees what they can configure)…
    expect(screen.getByDisplayValue('Local LLM')).toBeVisible()
    // …but the provider name input is disabled until AI is enabled.
    expect(screen.getByDisplayValue('Local LLM')).toBeDisabled()
    expect(
      screen.getAllByRole('combobox', { name: 'Start from a preset' })[0],
    ).toBeDisabled()
    // The master toggle itself stays usable so the user can opt in.
    expect(
      screen.getByRole('checkbox', { name: 'Enable AI features' }),
    ).toBeEnabled()
  })

  test('gates the assistant + semantic sub-toggles on the master switch and routes them', () => {
    const handlers = handlerFixture()
    // Master OFF: the two sub-toggles are visible but disabled, and the
    // "enable AI above" hint appears so the user sees what is available.
    const { rerender } = renderSection({
      ...handlers,
      currentSettings: settingsFixture({
        enabled: false,
        assistantEnabled: false,
        semanticIndexEnabled: false,
      }),
    })

    const assistantOff = screen.getByRole('checkbox', {
      name: 'AI assistant (chat)',
    })
    const semanticOff = screen.getByRole('checkbox', { name: 'Smart search' })
    const mcpOff = screen.getByRole('checkbox', {
      name: 'External tool access (MCP)',
    })
    const skillOff = screen.getByRole('checkbox', {
      name: 'Usage guide for external tools',
    })
    expect(assistantOff).not.toBeChecked()
    expect(assistantOff).toBeDisabled()
    expect(semanticOff).not.toBeChecked()
    expect(semanticOff).toBeDisabled()
    // The outward MCP surface is gated behind the master switch like the rest.
    expect(mcpOff).not.toBeChecked()
    expect(mcpOff).toBeDisabled()
    // The skill / usage-guide toggle is gated behind the master switch too.
    expect(skillOff).not.toBeChecked()
    expect(skillOff).toBeDisabled()
    expect(
      screen.getByText('Enable AI features above to turn these on.'),
    ).toBeVisible()
    // The semantic helper narrates the one-time index build requirement.
    expect(screen.getByText(/one-time index build/i)).toBeVisible()

    // Master ON: both become interactive and route to their own handlers,
    // independent of the master toggle (no cascade).
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              ...handlers,
              currentSettings: settingsFixture({
                enabled: true,
                assistantEnabled: false,
                semanticIndexEnabled: false,
              }),
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    const assistantOn = screen.getByRole('checkbox', {
      name: 'AI assistant (chat)',
    })
    const semanticOn = screen.getByRole('checkbox', { name: 'Smart search' })
    const mcpOn = screen.getByRole('checkbox', {
      name: 'External tool access (MCP)',
    })
    const skillOn = screen.getByRole('checkbox', {
      name: 'Usage guide for external tools',
    })
    expect(assistantOn).toBeEnabled()
    expect(semanticOn).toBeEnabled()
    expect(mcpOn).toBeEnabled()
    expect(skillOn).toBeEnabled()
    // The outward data-surface toggle starts OFF and reflects the draft.
    expect(mcpOn).not.toBeChecked()
    // The skill toggle is independent and also starts OFF.
    expect(skillOn).not.toBeChecked()
    // The disclosure spells out the read-only / audited / opt-in boundary and
    // ties to the toggle for screen readers.
    expect(mcpOn).toHaveAttribute('aria-describedby', 'ai-mcp-disclosure')
    const disclosure = screen.getByTestId('ai-mcp-disclosure')
    expect(disclosure).toHaveTextContent('read-only')
    expect(disclosure).toHaveTextContent('Every external query is recorded')
    expect(disclosure).toHaveTextContent(
      'nothing is exposed until you turn this on',
    )
    // The audit promise is actionable: a link points the user at the ledger
    // where their recorded external-query (mcp_query) runs are reviewable.
    const auditLink = within(disclosure).getByRole('link', {
      name: 'Review external-query activity',
    })
    expect(auditLink).toHaveAttribute('href', '/audit')
    // The skill disclosure is honest: it is guidance only AND is only reachable
    // when the MCP server above is on (it never claims to expose data alone).
    expect(skillOn).toHaveAttribute('aria-describedby', 'ai-skill-disclosure')
    const skillDisclosure = screen.getByTestId('ai-skill-disclosure')
    expect(skillDisclosure).toHaveTextContent('guidance only')
    expect(skillDisclosure).toHaveTextContent(
      'only reachable when External tool access (MCP) above is also on',
    )
    fireEvent.click(assistantOn)
    expect(handlers.onToggleAssistant).toHaveBeenCalledTimes(1)
    expect(handlers.onToggleAi).not.toHaveBeenCalled()
    fireEvent.click(semanticOn)
    expect(handlers.onToggleSemanticIndex).toHaveBeenCalledTimes(1)
    expect(handlers.onToggleAi).not.toHaveBeenCalled()
    fireEvent.click(mcpOn)
    expect(handlers.onToggleMcp).toHaveBeenCalledTimes(1)
    expect(handlers.onToggleAi).not.toHaveBeenCalled()
    // The skill toggle routes to its own handler with no cascade to the master.
    fireEvent.click(skillOn)
    expect(handlers.onToggleSkill).toHaveBeenCalledTimes(1)
    expect(handlers.onToggleAi).not.toHaveBeenCalled()
  })

  test('ties the master AI toggle to the consent disclosure for screen readers', () => {
    renderSection({ currentSettings: settingsFixture({ enabled: false }) })

    const master = screen.getByRole('checkbox', { name: 'Enable AI features' })
    expect(master).toHaveAttribute('aria-describedby', 'ai-consent-disclosure')
    expect(document.getElementById('ai-consent-disclosure')).not.toBeNull()
  })

  test('probes a provider and shows reachable latency + error inline', () => {
    const handlers = handlerFixture()
    const { rerender } = renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
    })

    // The Test-connection button is enabled (every provider on screen is already
    // saved config) and routes to the per-purpose probe handler. The first button
    // belongs to the LLM editor list, the second to the embedding editor list.
    const testButtons = screen.getAllByRole('button', {
      name: 'Test connection',
    })
    fireEvent.click(testButtons[0])
    expect(handlers.onProviderProbe).toHaveBeenLastCalledWith('llm', 'llm-1')
    // The embedding list's probe arm must route with the embedding purpose, not
    // the LLM purpose (distinct AiProviderEditorList instance).
    fireEvent.click(testButtons[1])
    expect(handlers.onProviderProbe).toHaveBeenLastCalledWith(
      'embedding',
      'embed-1',
    )

    // A reachable probe shows the latency line; an unreachable one shows the
    // error message + action hint.
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              ...handlers,
              currentSettings: settingsFixture({ enabled: true }),
              providerProbes: {
                'llm-1': probeReportFixture({ ok: true }),
                'embed-1': probeReportFixture({
                  ok: false,
                  message: 'Connection refused',
                  actionHint: 'Start LM Studio and load a model.',
                }),
              },
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    expect(screen.getByText('Connected')).toBeVisible()
    expect(screen.getByText('local-model · 42 ms')).toBeVisible()
    expect(screen.getByText('Connection issue')).toBeVisible()
    expect(screen.getByText('Connection refused')).toBeVisible()
    expect(screen.getByText('Start LM Studio and load a model.')).toBeVisible()

    // The reachable result renders on the LLM provider's card and the failure
    // result on the embedding provider's card, never crossed between lists.
    const llmCard = screen.getByDisplayValue('Local LLM').closest('article')
    const embedCard = screen
      .getByDisplayValue('Local Embeddings')
      .closest('article')
    expect(within(llmCard as HTMLElement).getByText('Connected')).toBeVisible()
    expect(
      within(llmCard as HTMLElement).queryByText('Connection issue'),
    ).toBeNull()
    expect(
      within(embedCard as HTMLElement).getByText('Connection issue'),
    ).toBeVisible()
    expect(within(embedCard as HTMLElement).queryByText('Connected')).toBeNull()
  })

  test('enables Test connection and Save key immediately after adding a provider, with no "save first" hint', () => {
    // All-auto-save: adding a provider persists it instantly, so Test connection
    // works right away and the old "save settings first" dead-end hints are gone.
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      aiApiKeys: { 'llm-1': 'sk-typed', 'embed-1': 'sk-typed' },
      currentSettings: settingsFixture({ enabled: true }),
    })

    const probe = screen.getAllByRole('button', { name: 'Test connection' })[0]
    expect(probe).toBeEnabled()
    fireEvent.click(probe)
    expect(handlers.onProviderProbe).toHaveBeenLastCalledWith('llm', 'llm-1')

    const save = screen.getAllByRole('button', { name: 'Save key' })[0]
    expect(save).toBeEnabled()
    fireEvent.click(save)
    expect(handlers.onSaveAiApiKey).toHaveBeenCalledWith('llm-1')

    // The removed "save first" hints must not render under any provider.
    expect(
      screen.queryByText('Save this provider first to test it.'),
    ).toBeNull()
    expect(screen.queryByTestId(/^save-key-hint-/)).toBeNull()
    expect(screen.queryByTestId(/^probe-hint-/)).toBeNull()
  })

  test('keeps Save key disabled until a key is typed (its only legitimate transient gate)', () => {
    // No "save first" hint anymore; the only remaining gate is "type a key first".
    renderSection({
      aiApiKeys: {},
      currentSettings: settingsFixture({ enabled: true }),
    })
    expect(
      screen.getAllByRole('button', { name: 'Save key' })[0],
    ).toBeDisabled()
    expect(screen.queryByTestId(/^save-key-hint-/)).toBeNull()
  })

  test('probes a provider even while the AI master toggle is OFF (you test before opting in)', () => {
    // Testing a connection must not require AI to be enabled — the user validates
    // an endpoint BEFORE committing to turning AI on.
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: false }),
    })
    const probe = screen.getAllByRole('button', { name: 'Test connection' })[0]
    expect(probe).toBeEnabled()
    fireEvent.click(probe)
    expect(handlers.onProviderProbe).toHaveBeenLastCalledWith('llm', 'llm-1')
  })

  test('keeps every probe disabled while another probe is in flight (transient gate, no hint)', () => {
    // A transient gate (another probe running) disables every probe button, but
    // it resolves on its own and the in-flight button already shows "Testing…".
    // No "save first" hint exists anymore (providers always persist on add).
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      testingProviderId: 'llm-1',
    })
    screen
      .getAllByRole('button', { name: /Test connection|Testing/ })
      .forEach((button) => expect(button).toBeDisabled())
    expect(
      screen.queryByText('Save this provider first to test it.'),
    ).toBeNull()
  })

  test('relabels the probe button while a probe is in flight', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      testingProviderId: 'llm-1',
    })

    // The in-flight provider shows the testing label; the other keeps the idle
    // label but is disabled (single probe at a time).
    expect(screen.getByText('Testing…')).toBeVisible()
    screen
      .getAllByRole('button', { name: /Test connection|Testing/ })
      .forEach((button) => expect(button).toBeDisabled())
  })

  test('is all-auto-save: no Save AI config / Reset draft controls, toggles persist immediately', async () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
    })

    // The staged-draft Save / Reset controls are gone in the all-auto-save model.
    expect(screen.queryByTestId('ai-save-config')).toBeNull()
    expect(screen.queryByTestId('ai-reset-config')).toBeNull()
    expect(screen.queryByText('You have unsaved changes')).toBeNull()
    expect(screen.queryByText('Settings are up to date')).toBeNull()

    // Toggling the master / assistant / semantic switches persists immediately.
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'Enable AI features' }),
    )
    expect(handlers.onToggleAi).toHaveBeenCalledTimes(1)
    fireEvent.click(
      screen.getByRole('checkbox', { name: 'AI assistant (chat)' }),
    )
    expect(handlers.onToggleAssistant).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('checkbox', { name: 'Smart search' }))
    expect(handlers.onToggleSemanticIndex).toHaveBeenCalledTimes(1)

    // The quiet "Saved" chip flashes after a successful auto-save.
    expect(await screen.findByText('Saved')).toBeInTheDocument()
  })

  test('commits in-progress provider field edits on blur (auto-save), typing stays local', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
    })

    const llmName = screen.getByDisplayValue('Local LLM')
    // Typing updates the local buffer only — no commit yet.
    fireEvent.change(llmName, { target: { value: 'Edited LLM' } })
    expect(handlers.onUpdateProvider).toHaveBeenLastCalledWith('llm', 'llm-1', {
      name: 'Edited LLM',
    })
    expect(handlers.onCommitProviders).not.toHaveBeenCalled()
    // Blurring out of the card commits (auto-save), for both editor lists.
    fireEvent.blur(llmName)
    expect(handlers.onCommitProviders).toHaveBeenCalledTimes(1)
    fireEvent.blur(screen.getByDisplayValue('Local Embeddings'))
    expect(handlers.onCommitProviders).toHaveBeenCalledTimes(2)
  })

  test('auto-saves the search-tuning knobs and the GPU toggle, flashing the Saved chip', async () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      // A drifted knob (lexicalWeight != default) so the Reset control is enabled.
      currentSettings: settingsFixture({
        enabled: true,
        semanticIndexEnabled: true,
        lexicalWeight: 2,
      }),
    })

    // Search tuning: a knob edit + reset both auto-save. The disclosure content is
    // in the DOM even while collapsed, so the controls are directly queryable.
    fireEvent.change(
      screen.getByTestId('ai-search-tuning-lexicalWeight-input'),
      { target: { value: '1.5' } },
    )
    expect(handlers.onSearchTuningChange).toHaveBeenCalledWith(
      'lexicalWeight',
      1.5,
    )
    fireEvent.click(screen.getByTestId('ai-search-tuning-reset'))
    expect(handlers.onResetSearchTuning).toHaveBeenCalledTimes(1)

    // GPU: flip the toggle (auto-save).
    fireEvent.click(screen.getByTestId('ai-gpu-toggle'))
    expect(handlers.onToggleGpu).toHaveBeenCalledTimes(1)

    // A successful auto-save flashes the quiet "Saved" chip.
    expect(await screen.findByText('Saved')).toBeInTheDocument()
  })

  test('does not flash the Saved chip when an auto-save is a no-op (resolves false)', async () => {
    // A handler that resolves false (e.g. nothing changed) must stay silent — no
    // misleading "Saved" confirmation.
    const handlers = handlerFixture()
    handlers.onToggleAi.mockResolvedValue(false)
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
    })

    fireEvent.click(
      screen.getByRole('checkbox', { name: 'Enable AI features' }),
    )
    expect(handlers.onToggleAi).toHaveBeenCalledTimes(1)
    // Give any microtask a chance to resolve; the chip must remain hidden.
    await Promise.resolve()
    expect(
      screen.getByTestId('settings-saved-chip').getAttribute('data-visible'),
    ).toBe('false')
  })

  test('keeps the Saved chip hidden and swallows the rejection when an auto-save fails', async () => {
    // persistAi re-throws on a failed saveConfig (the shell already set the error
    // banner). flashOnSave must swallow that rejection so there is no unhandled
    // rejection on every failing toggle, and the chip must stay hidden.
    const handlers = handlerFixture()
    handlers.onToggleAi.mockRejectedValue(new Error('save failed'))
    const unhandled = vi.fn()
    process.on('unhandledRejection', unhandled)
    try {
      renderSection({
        ...handlers,
        currentSettings: settingsFixture({ enabled: true }),
      })

      fireEvent.click(
        screen.getByRole('checkbox', { name: 'Enable AI features' }),
      )
      expect(handlers.onToggleAi).toHaveBeenCalledTimes(1)
      // Let the rejected promise settle through the .catch.
      await Promise.resolve()
      await Promise.resolve()
      expect(
        screen.getByTestId('settings-saved-chip').getAttribute('data-visible'),
      ).toBe('false')
      expect(unhandled).not.toHaveBeenCalled()
    } finally {
      process.off('unhandledRejection', unhandled)
    }
  })

  test('surfaces index health when status + meta are present', () => {
    renderSection({
      aiStatus: aiStatusFixture({ indexedItems: 4321 }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: {
        label: 'Ready',
        tone: 'success',
        description: '4,321 rows indexed.',
      },
    })

    expect(screen.getByText('4,321 rows indexed.')).toBeVisible()
    expect(screen.getByText('4,321')).toBeVisible()
  })

  test.each(['success', 'warning', 'blocked', 'info'] as const)(
    'maps the %s index-health tone to a callout',
    (tone) => {
      const { container } = renderSection({
        aiStatus: aiStatusFixture(),
        currentSettings: settingsFixture({ enabled: true }),
        indexMeta: {
          label: tone,
          tone,
          description: `tone-${tone}`,
        },
      })

      expect(screen.getByText(`tone-${tone}`)).toBeVisible()
      // The status-callout tone class is derived from the index-health tone.
      const expectedClass =
        tone === 'success'
          ? 'status-callout--success'
          : tone === 'warning'
            ? 'status-callout--warning'
            : tone === 'blocked'
              ? 'status-callout--blocked'
              : 'status-callout--info'
      expect(container.querySelector(`.${expectedClass}`)).not.toBeNull()
    },
  )

  test('localizes the index-health warning CODE and weaves interpolation params (M-7)', () => {
    // Review-fix M-7: a stable `warningCode` is resolved to localized copy for EVERY variant — never
    // an English-sentence match. The interpolated variant must weave its structural param.
    const { rerender } = renderSection({
      aiStatus: aiStatusFixture({
        warning: 'Enable provider My Embed before using semantic retrieval.',
        warningCode: {
          code: 'embeddingProviderDisabled',
          providerName: 'My Embed',
        },
      }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: { label: 'Warning', tone: 'warning', description: 'warn' },
    })

    expect(screen.getByText('Current index warning')).toBeVisible()
    // The localized copy is composed from the structural `providerName` param, not pre-baked English.
    // It renders EXACTLY ONCE — in the dedicated warning box; the callout body no longer repeats a
    // coded warning (dedupe), so a regression that re-duplicated it would make getByText throw.
    expect(
      screen.getByText(
        'Enable provider My Embed before using semantic retrieval.',
      ),
    ).toBeVisible()

    // A `buildFailed` code carries its opaque transport reason verbatim, wrapped in localized copy.
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              aiStatus: aiStatusFixture({
                warning: 'The last index build failed: disk full',
                warningCode: { code: 'buildFailed', reason: 'disk full' },
              }),
              currentSettings: settingsFixture({ enabled: true }),
              indexMeta: {
                label: 'Failed',
                tone: 'blocked',
                description: 'failed',
              },
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    expect(
      screen.getByText('The last index build failed: disk full'),
    ).toBeVisible()

    // An older payload with only the legacy English `warning` (no code) falls back verbatim.
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              aiStatus: aiStatusFixture({
                warning: 'Indexer paused by policy.',
              }),
              currentSettings: settingsFixture({ enabled: true }),
              indexMeta: {
                label: 'Blocked',
                tone: 'blocked',
                description: 'blocked',
              },
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    expect(screen.getByText('Indexer paused by policy.')).toBeVisible()
  })

  test('offers a Build-index CTA when an embedding provider is configured and the index is empty, firing the from-scratch full build', async () => {
    // Bug 1: the empty-index state with a configured embedding provider is exactly
    // when a from-scratch build is actionable, so the CTA must appear right here in
    // the index-health box (not only in the collapsed GPU section).
    renderSection({
      aiStatus: aiStatusFixture({
        state: 'empty',
        indexedItems: 0,
        warningCode: { code: 'indexNotBuilt' },
      }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: { label: 'No index yet', tone: 'info', description: 'empty' },
    })

    const build = screen.getByTestId('ai-index-build')
    expect(build).toHaveTextContent('Build index')
    expect(build).toBeEnabled()

    fireEvent.click(build)
    // The build enqueues the from-scratch full backfill — the same shape the GPU
    // section uses for a full re-embed.
    expect(buildAiIndex).toHaveBeenCalledTimes(1)
    expect(buildAiIndex).toHaveBeenCalledWith({
      fullRebuild: true,
      clearOnly: false,
      scope: 'full',
    })
    // While the enqueue is in flight the button is disabled + relabeled so a
    // double-click cannot double-enqueue.
    expect(build).toBeDisabled()
    expect(build).toHaveTextContent('Building index…')

    // Once the enqueue resolves it settles to an honest "queued in the background"
    // confirmation (never claims the minutes-long index is already built).
    expect(
      await screen.findByTestId('ai-index-build-queued'),
    ).toBeInTheDocument()
    expect(screen.getByTestId('ai-index-build')).toBeEnabled()
  })

  test('also offers the Build-index CTA when the index is stale', () => {
    renderSection({
      aiStatus: aiStatusFixture({ state: 'stale' }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: { label: 'Stale', tone: 'warning', description: 'stale' },
    })

    expect(screen.getByTestId('ai-index-build')).toBeVisible()
  })

  test('surfaces an honest error if the index-build enqueue fails', async () => {
    buildAiIndex.mockRejectedValueOnce(new Error('queue offline'))
    renderSection({
      aiStatus: aiStatusFixture({
        state: 'empty',
        indexedItems: 0,
        warningCode: { code: 'indexNotBuilt' },
      }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: { label: 'No index yet', tone: 'info', description: 'empty' },
    })

    fireEvent.click(screen.getByTestId('ai-index-build'))
    expect(
      await screen.findByTestId('ai-index-build-error'),
    ).toBeInTheDocument()
    // The button is re-enabled so the user can retry.
    expect(screen.getByTestId('ai-index-build')).toBeEnabled()
  })

  test('does NOT offer the Build-index CTA when no embedding provider is configured (no nag)', () => {
    // optional-AI-no-nag: when nothing is configured there is nothing to build, so
    // the CTA must stay hidden rather than nag the user toward an action that would
    // immediately fail.
    renderSection({
      aiStatus: aiStatusFixture({
        state: 'empty',
        indexedItems: 0,
        warningCode: { code: 'noEmbeddingProvider' },
      }),
      currentSettings: settingsFixture({
        enabled: true,
        embeddingProviders: [],
        embeddingProviderId: null,
      }),
      indexMeta: { label: 'No index yet', tone: 'info', description: 'empty' },
    })

    expect(screen.queryByTestId('ai-index-build')).toBeNull()
    expect(buildAiIndex).not.toHaveBeenCalled()
  })

  test('does NOT offer the Build-index CTA when the index is already ready', () => {
    // A ready index has nothing to build from the health box.
    renderSection({
      aiStatus: aiStatusFixture({ state: 'ready' }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: { label: 'Ready', tone: 'success', description: 'ready' },
    })

    expect(screen.queryByTestId('ai-index-build')).toBeNull()
  })

  test('renders the index-health description in zh-CN (localized CODE, not raw English) for the empty state', () => {
    // Bug 2: for any state carrying a warning CODE, the index-health description
    // must resolve to localized copy — never the backend's raw English `warning`.
    window.localStorage.setItem(i18nStorageKey, 'zh-CN')
    renderSection({
      aiStatus: aiStatusFixture({
        state: 'empty',
        indexedItems: 0,
        // The legacy English sentence the backend still ships — must NOT leak.
        warning:
          'Run Build index after configuring an embedding provider to enable semantic search.',
        warningCode: { code: 'indexNotBuilt' },
      }),
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: {
        label: 'No index yet',
        tone: 'info',
        // Even the intelligence-namespace fallback description is English; the fix
        // must override it via the CODE so this never reaches the zh-CN screen.
        description:
          'Build the search index first to enable natural language search.',
      },
    })

    // The localized zh-CN copy is shown EXACTLY ONCE — in the dedicated warning box;
    // the callout body no longer repeats a coded warning (dedupe), and the English
    // never leaks.
    expect(
      screen.getByText('配置好向量模型后，运行“构建索引”即可启用语义搜索。'),
    ).toBeVisible()
    // …and neither English sentence leaks into the zh UI.
    expect(
      screen.queryByText(
        'Run Build index after configuring an embedding provider to enable semantic search.',
      ),
    ).toBeNull()
    expect(
      screen.queryByText(
        'Build the search index first to enable natural language search.',
      ),
    ).toBeNull()
    // The CTA copy is localized too.
    expect(screen.getByTestId('ai-index-build')).toHaveTextContent('构建索引')
  })

  test('freezes the provider editors while an auto-save is in flight', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      saving: true,
    })

    // While a write is in flight the editors are inert so a save can't be raced.
    expect(screen.getByDisplayValue('Local LLM')).toBeDisabled()
    expect(
      screen.getAllByRole('combobox', { name: 'Start from a preset' })[0],
    ).toBeDisabled()
  })

  test('skips index health when status or meta are missing', () => {
    renderSection({
      aiStatus: null,
      currentSettings: settingsFixture({ enabled: true }),
      indexMeta: null,
    })

    expect(screen.queryByText(/rows indexed/)).toBeNull()
    // The provider editor still renders without the index-health block.
    expect(screen.getByDisplayValue('Local LLM')).toBeVisible()
  })

  test('shows a getting-started callout when no providers exist', () => {
    renderSection({
      currentSettings: settingsFixture({
        embeddingProviders: [],
        embeddingProviderId: null,
        llmProviderId: null,
        llmProviders: [],
        enabled: true,
      }),
      noProviders: true,
    })

    expect(screen.getByText('No AI providers configured yet')).toBeVisible()
  })

  // ─── Static embedding tier ────────────────────────────────────────────────

  test('static provider does NOT appear in the editable external embedding list', () => {
    // The static provider has a dedicated BaseTierPanel — it must never also appear as an
    // editable card in the upgrade-tier list, which would falsely frame it as a peer alternative.
    renderSection({
      currentSettings: settingsFixture({
        enabled: true,
        embeddingProviders: [staticProviderFixture()],
        embeddingProviderId: 'static-in-app',
      }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({
          modelDownloaded: true,
          selected: true,
        }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    // The built-in badge that used to appear on the card in the list must be gone.
    expect(
      screen.queryByTestId('provider-builtin-badge-static-in-app'),
    ).toBeNull()
    // No "Static (in-app)" name input appears as a writable field.
    expect(screen.queryByDisplayValue('Static (in-app)')).toBeNull()
    // The base-tier panel is the only place it appears.
    expect(screen.getByTestId('ai-static-embedding-panel')).toBeVisible()
  })

  test('renders the two-tier vector model layout with tier labels, connector, and upgrade body', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({
          modelDownloaded: true,
          selected: true,
        }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    expect(screen.getByTestId('ai-vector-model-section')).toBeVisible()
    expect(screen.getByText('BASE TIER — ON-DEVICE')).toBeInTheDocument()
    expect(
      screen.getByText('UPGRADE TIER — OPTIONAL EXTERNAL MODELS'),
    ).toBeInTheDocument()
    // Tier connector sentence
    expect(screen.getByText(/recommended starting point/i)).toBeInTheDocument()
    // Upgrade tier body
    expect(
      screen.getByText(/higher precision for large archives/i),
    ).toBeInTheDocument()
  })

  // ─── Static model download panel (BaseTierPanel) ─────────────────────────

  test('shows the static embedding panel when aiStatus.staticEmbedding is present', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    expect(screen.getByTestId('ai-static-embedding-panel')).toBeVisible()
    // Panel title and description are shown
    expect(screen.getByText('Built-in · On-device model')).toBeVisible()
    expect(screen.getByText(/lower precision/i)).toBeVisible()
  })

  test('shows 256 dimensions from staticEmbedding.dimensions (not the 1536 fallback)', () => {
    // Regression: the old code passed the static provider through the generic editor which
    // fell back to 1536. The new BaseTierPanel reads dimensions directly from the backend status.
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({
          modelDownloaded: false,
          dimensions: 256,
        }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    // The specs line shows the real 256 from the backend, never 1536.
    expect(screen.getByText(/256 dimensions/i)).toBeInTheDocument()
    expect(screen.queryByText(/1536/)).toBeNull()
  })

  test('shows download button in idle state and size hint', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    expect(screen.getByTestId('ai-static-model-download')).toBeVisible()
    expect(screen.getByTestId('ai-static-model-download')).toHaveTextContent(
      'Download model',
    )
    // The pre-download size hint is shown. Anchor on its distinctive "≈ <size>, runs in the
    // background." ending so it isn't confused with the collapsed GPU re-embed copy that also
    // mentions running in the background.
    expect(screen.getByText(/, runs in the background\.$/i)).toBeInTheDocument()
  })

  test('clicking Download model calls downloadStaticEmbeddingModel and shows progress bar', () => {
    // Keep the download pending so we can assert the in-flight state.
    downloadStaticEmbeddingModel.mockReturnValue(new Promise(() => {}))

    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    fireEvent.click(screen.getByTestId('ai-static-model-download'))

    expect(downloadStaticEmbeddingModel).toHaveBeenCalledTimes(1)
    // Progress bar appears (indeterminate until fileStarted carries totalBytes)
    expect(screen.getByTestId('ai-base-tier-progress-bar')).toBeInTheDocument()
    // Cancel button appears
    expect(screen.getByTestId('ai-static-model-cancel')).toBeVisible()
    // Restart note appears during download
    expect(screen.getByTestId('ai-base-tier-restart-note')).toBeInTheDocument()
    // The download button disappears while downloading (no double-fire)
    expect(screen.queryByTestId('ai-static-model-download')).toBeNull()
  })

  test('shows ✓ Ready text when modelDownloaded is true (no download button shown)', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({
          modelDownloaded: true,
          selected: true,
        }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    expect(screen.getByTestId('ai-base-tier-ready')).toHaveTextContent(
      '✓ Ready',
    )
    // No download button when the model is already present
    expect(screen.queryByTestId('ai-static-model-download')).toBeNull()
  })

  test('shows download failed state and re-enables the retry button on command error', async () => {
    downloadStaticEmbeddingModel.mockRejectedValueOnce(
      new Error('network error'),
    )

    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    fireEvent.click(screen.getByTestId('ai-static-model-download'))

    // After the download command fails, show the failed message and re-enable retry
    expect(
      await screen.findByTestId('ai-base-tier-download-failed'),
    ).toBeInTheDocument()
    expect(
      screen.getByTestId('ai-base-tier-download-failed'),
    ).toHaveTextContent(
      'Download failed — check your connection and try again.',
    )
    // Retry button is the same testid as the download button
    expect(screen.getByTestId('ai-static-model-download')).toBeVisible()
    expect(screen.getByTestId('ai-static-model-download')).toHaveTextContent(
      'Retry download',
    )
  })

  test('shows the indeterminate bar and current file while downloading with an unknown total', () => {
    // The hook owns event → state translation (covered in model-download.test.ts). Here we assert the
    // panel's downloading render for a progress object whose total is unknown (totalBytes 0): an
    // indeterminate (aria-busy, no valuenow) bar, the current-file basename, the Cancel button, and
    // no Download button (no double-fire).
    useModelDownloadProgress.mockReturnValue({
      phase: 'downloading',
      downloadedBytes: 0,
      totalBytes: 0,
      currentFile: 'model.safetensors',
      error: null,
    })
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    const bar = screen.getByTestId('ai-base-tier-progress-bar')
    expect(bar).toBeInTheDocument()
    // Indeterminate: aria-busy true, no valuenow
    expect(bar).toHaveAttribute('aria-busy', 'true')
    expect(bar).not.toHaveAttribute('aria-valuenow')
    // Current file shown as basename (mono)
    expect(
      screen.getByTestId('ai-static-model-current-file'),
    ).toHaveTextContent('model.safetensors')
    expect(screen.getByTestId('ai-static-model-cancel')).toBeVisible()
    // Download button is gone while in flight (no double-fire)
    expect(screen.queryByTestId('ai-static-model-download')).toBeNull()
  })

  test('renders a real determinate progress bar that reflects the downloaded bytes', () => {
    // Core bar-reflects-bytes assertion: a progress object with a known total makes the bar
    // determinate and the value tracks downloadedBytes.
    useModelDownloadProgress.mockReturnValue({
      phase: 'downloading',
      downloadedBytes: 50_000_000,
      totalBytes: 100_000_000,
      currentFile: 'model.safetensors',
      error: null,
    })
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    const bar = screen.getByTestId('ai-base-tier-progress-bar')
    // Determinate: has aria-valuenow + aria-valuemax
    expect(bar).toHaveAttribute('aria-valuenow', '50000000')
    expect(bar).toHaveAttribute('aria-valuemax', '100000000')
    expect(bar).not.toHaveAttribute('aria-busy')
    // Bytes label shown
    expect(
      screen.getByTestId('ai-base-tier-progress-label'),
    ).toBeInTheDocument()
  })

  test('surfaces the honest failed state when the hook reports a non-cancel error', () => {
    useModelDownloadProgress.mockReturnValue({
      phase: 'failed',
      downloadedBytes: 0,
      totalBytes: 0,
      currentFile: null,
      error: 'disk full',
    })
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    expect(
      screen.getByTestId('ai-base-tier-download-failed'),
    ).toBeInTheDocument()
    // The retry button (same testid as Download) is offered so the user can try again.
    expect(screen.getByTestId('ai-static-model-download')).toBeVisible()
    expect(screen.getByTestId('ai-static-model-download')).toHaveTextContent(
      'Retry download',
    )
  })

  test('shows ready with no spinner and no download button when the hook reports ready', () => {
    // The "done → ready immediately (no stuck spinner)" behavior lives in the hook; the panel's job
    // is to render the ready state cleanly: ready text, and never a leftover spinner or Download CTA.
    useModelDownloadProgress.mockReturnValue({
      phase: 'ready',
      downloadedBytes: 0,
      totalBytes: 0,
      currentFile: null,
      error: null,
    })
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    expect(screen.getByTestId('ai-base-tier-ready')).toBeVisible()
    // No download button, no progress bar in the ready state.
    expect(screen.queryByTestId('ai-static-model-download')).toBeNull()
    expect(screen.queryByTestId('ai-base-tier-progress-bar')).toBeNull()
  })

  test('Cancel stops the download and returns to idle without looking like a failure', () => {
    // Clicking Cancel fires the cancel IPC and optimistically returns the panel to idle (the
    // Download button is back and no failure is shown). The hook-level guarantee that a cancel's
    // terminal error maps to idle rather than failed is covered in model-download.test.ts.
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    fireEvent.click(screen.getByTestId('ai-static-model-download'))
    expect(screen.getByTestId('ai-static-model-cancel')).toBeVisible()

    fireEvent.click(screen.getByTestId('ai-static-model-cancel'))
    expect(cancelStaticEmbeddingModelDownload).toHaveBeenCalledTimes(1)
    expect(markModelDownloadSettled).toHaveBeenCalled()
    // Immediately back to idle (Download button visible, no error)
    expect(screen.getByTestId('ai-static-model-download')).toBeVisible()
    expect(screen.queryByTestId('ai-base-tier-download-failed')).toBeNull()
  })

  test('Cancel stays robust even if the cancel command itself rejects', async () => {
    cancelStaticEmbeddingModelDownload.mockRejectedValueOnce(
      new Error('ipc offline'),
    )

    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    fireEvent.click(screen.getByTestId('ai-static-model-download'))
    await act(async () => {
      fireEvent.click(screen.getByTestId('ai-static-model-cancel'))
      await Promise.resolve()
    })

    // The UI returns to idle optimistically; a rejected cancel command is swallowed (the download
    // will still terminate on its own) rather than surfacing an unhandled rejection.
    expect(cancelStaticEmbeddingModelDownload).toHaveBeenCalledTimes(1)
    expect(screen.queryByTestId('ai-base-tier-download-failed')).toBeNull()
    expect(screen.getByTestId('ai-static-model-download')).toBeVisible()
  })

  test('shows Active pill when the static tier is the selected embedding model', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({
          modelDownloaded: true,
          selected: true,
        }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    // The Active pill is in the panel header (not a status text element)
    expect(screen.getByTestId('ai-static-model-active')).toHaveTextContent(
      'Active',
    )
    // No "Use built-in model" button when it is already active
    expect(screen.queryByTestId('ai-static-model-select')).toBeNull()
  })

  test('offers a "Use built-in model" button when ready and not the active model', () => {
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({
          modelDownloaded: true,
          selected: false,
        }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    // Active pill is absent
    expect(screen.queryByTestId('ai-static-model-active')).toBeNull()
    // "Use built-in model" button is shown (renamed from "Use this model")
    expect(screen.getByTestId('ai-static-model-select')).toHaveTextContent(
      'Use built-in model',
    )

    fireEvent.click(screen.getByTestId('ai-static-model-select'))
    expect(handlers.onSelectProvider).toHaveBeenCalledWith(
      'embedding',
      'static-in-app',
    )
  })

  test('a remount while a download is in flight shows the progress bar and never re-enables Download', () => {
    // Simulate the durable latch reporting an in-flight download (e.g. the panel just remounted
    // after a snapshot poll briefly dropped staticEmbedding).
    markModelDownloadStarted()

    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    // Initialized straight into the downloading state — progress bar visible, no Download button
    expect(screen.getByTestId('ai-base-tier-progress-bar')).toBeInTheDocument()
    expect(screen.queryByTestId('ai-static-model-download')).toBeNull()
    expect(screen.getByTestId('ai-static-model-cancel')).toBeVisible()
  })

  test('stays downloading with no current-file line between files (current file cleared)', () => {
    // Between files the hook reports a downloading phase with currentFile null; the panel must keep
    // showing the progress bar and drop the current-file line. (The current-file-present case is
    // covered by the indeterminate-bar test above.)
    useModelDownloadProgress.mockReturnValue({
      phase: 'downloading',
      downloadedBytes: 1_000,
      totalBytes: 0,
      currentFile: null,
      error: null,
    })
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({ modelDownloaded: false }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    expect(screen.getByTestId('ai-base-tier-progress-bar')).toBeInTheDocument()
    expect(screen.queryByTestId('ai-static-model-current-file')).toBeNull()
  })

  test('falls back to default specs (256 dimensions, ≈ 96 MB) when the backend omits them', () => {
    // Older backends omit `dimensions` / `modelSizeBytes`; the panel must fall back to the static
    // model's known 256 dims and the constant ≈ 96 MB size estimate rather than render blanks.
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        staticEmbedding: staticEmbeddingFixture({
          modelDownloaded: false,
          dimensions: undefined,
          modelSizeBytes: undefined,
        }),
      }),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    // The specs line carries both fallbacks: 256 dimensions and the ≈ 96 MB estimate.
    expect(screen.getByText(/256 dimensions/i)).toHaveTextContent(/96 MB/)
  })

  test('does not show the static embedding panel when aiStatus.staticEmbedding is absent', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture(),
      indexMeta: { label: 'Ready', tone: 'success', description: '' },
    })

    expect(screen.queryByTestId('ai-static-embedding-panel')).toBeNull()
  })

  // ─── Honest index health: semanticVectorCount ─────────────────────────────

  test('shows the real semanticVectorCount in the index health grid when present', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({ semanticVectorCount: 5000 }),
      indexMeta: { label: 'Ready', tone: 'success', description: 'ready' },
    })

    expect(screen.getByTestId('ai-semantic-vector-count-row')).toBeVisible()
    expect(
      screen.getByTestId('ai-semantic-vector-count-row'),
    ).toHaveTextContent('5,000')
  })

  test('does not show a semanticVectorCount row when the field is absent', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({ semanticVectorCount: null }),
      indexMeta: { label: 'Ready', tone: 'success', description: 'ready' },
    })

    expect(screen.queryByTestId('ai-semantic-vector-count-row')).toBeNull()
  })

  // ─── Degraded / IndexVectorsMissing honest rendering ─────────────────────

  test('shows the IndexVectorsMissing warning honestly when state is degraded', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        state: 'degraded',
        indexedItems: 1200,
        semanticVectorCount: 0,
        warningCode: { code: 'indexVectorsMissing' },
      }),
      indexMeta: {
        label: 'Degraded',
        tone: 'blocked',
        description: 'degraded',
      },
    })

    // The honest warning message is shown (not a false "indexed N successfully").
    expect(screen.getByText('Current index warning')).toBeVisible()
    expect(
      screen.getByText(/the embedding provider wrote no output/i),
    ).toBeVisible()

    // The semanticVectorCount of 0 is shown so the user sees "0 vectors" clearly.
    expect(
      screen.getByTestId('ai-semantic-vector-count-row'),
    ).toHaveTextContent('0')
  })

  test('does not show a green "ready" callout when vectors=0 (degraded honest state)', () => {
    const { container } = renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        state: 'degraded',
        semanticVectorCount: 0,
        warningCode: { code: 'indexVectorsMissing' },
      }),
      indexMeta: {
        label: 'Degraded',
        tone: 'blocked',
        description: 'degraded',
      },
    })

    // The callout must be blocked-tone (never success) for the degraded state.
    expect(container.querySelector('.status-callout--success')).toBeNull()
    expect(container.querySelector('.status-callout--blocked')).not.toBeNull()
  })

  // ─── IndexResetButton ─────────────────────────────────────────────────────

  test('shows the Reset button when index state is failed', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({ state: 'failed' }),
      indexMeta: { label: 'Failed', tone: 'blocked', description: 'failed' },
    })

    expect(screen.getByTestId('ai-index-reset')).toBeVisible()
    expect(screen.getByTestId('ai-index-reset')).toHaveTextContent(
      'Clear stuck build & rebuild',
    )
  })

  test('shows the Reset button when index state is degraded', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({
        state: 'degraded',
        warningCode: { code: 'indexVectorsMissing' },
      }),
      indexMeta: {
        label: 'Degraded',
        tone: 'blocked',
        description: 'degraded',
      },
    })

    expect(screen.getByTestId('ai-index-reset')).toBeVisible()
  })

  test('does NOT show the Reset button for non-stuck states (ready, stale, queued)', () => {
    for (const state of [
      'ready',
      'stale',
      'queued',
      'paused',
      'disabled',
      'blocked',
    ] as const) {
      const { unmount } = renderSection({
        currentSettings: settingsFixture({ enabled: true }),
        aiStatus: aiStatusFixture({ state }),
        indexMeta: { label: state, tone: 'info', description: state },
      })
      expect(screen.queryByTestId('ai-index-reset')).toBeNull()
      unmount()
    }
  })

  test('reset button shows confirm prompt on first click, then calls resetAiIndexBuild on confirm', async () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({ state: 'failed' }),
      indexMeta: { label: 'Failed', tone: 'blocked', description: 'failed' },
    })

    // First click shows the confirm prompt.
    fireEvent.click(screen.getByTestId('ai-index-reset'))
    expect(screen.getByTestId('ai-index-reset-confirm')).toBeVisible()
    expect(screen.getByTestId('ai-index-reset-confirm-yes')).toBeVisible()
    expect(screen.getByTestId('ai-index-reset-confirm-no')).toBeVisible()

    // Confirm yes: calls resetAiIndexBuild.
    fireEvent.click(screen.getByTestId('ai-index-reset-confirm-yes'))
    expect(resetAiIndexBuild).toHaveBeenCalledTimes(1)

    // Settles to the "queued" confirmation.
    expect(
      await screen.findByTestId('ai-index-reset-queued'),
    ).toBeInTheDocument()
    expect(screen.getByTestId('ai-index-reset-queued')).toHaveTextContent(
      'Cleared — full rebuild queued in the background.',
    )
  })

  test('reset confirm-no cancels back to idle state', () => {
    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({ state: 'failed' }),
      indexMeta: { label: 'Failed', tone: 'blocked', description: 'failed' },
    })

    fireEvent.click(screen.getByTestId('ai-index-reset'))
    expect(screen.getByTestId('ai-index-reset-confirm')).toBeVisible()

    fireEvent.click(screen.getByTestId('ai-index-reset-confirm-no'))
    // Back to idle: confirm dialog is gone, reset button is back.
    expect(screen.queryByTestId('ai-index-reset-confirm')).toBeNull()
    expect(screen.getByTestId('ai-index-reset')).toBeVisible()
    expect(resetAiIndexBuild).not.toHaveBeenCalled()
  })

  test('reset button shows an honest error when resetAiIndexBuild fails', async () => {
    resetAiIndexBuild.mockRejectedValueOnce(new Error('backend offline'))

    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({ state: 'failed' }),
      indexMeta: { label: 'Failed', tone: 'blocked', description: 'failed' },
    })

    fireEvent.click(screen.getByTestId('ai-index-reset'))
    fireEvent.click(screen.getByTestId('ai-index-reset-confirm-yes'))

    expect(
      await screen.findByTestId('ai-index-reset-error'),
    ).toBeInTheDocument()
    expect(screen.getByTestId('ai-index-reset-error')).toHaveTextContent(
      'Could not reset the build. Please try again.',
    )
    // The button is re-enabled so the user can retry.
    expect(screen.getByTestId('ai-index-reset')).toBeVisible()
  })

  test('reset button re-enters confirming state when clicked from the error state', async () => {
    // Exercises the `onClick` in the error-state render path (gap 3 coverage).
    // After a failure, clicking the button again must return to "confirming" so the
    // user can attempt a retry without a full page refresh.
    resetAiIndexBuild.mockRejectedValueOnce(new Error('backend offline'))

    renderSection({
      currentSettings: settingsFixture({ enabled: true }),
      aiStatus: aiStatusFixture({ state: 'failed' }),
      indexMeta: { label: 'Failed', tone: 'blocked', description: 'failed' },
    })

    // Drive to error state.
    fireEvent.click(screen.getByTestId('ai-index-reset'))
    fireEvent.click(screen.getByTestId('ai-index-reset-confirm-yes'))
    expect(
      await screen.findByTestId('ai-index-reset-error'),
    ).toBeInTheDocument()

    // Click the retry button — should move back to confirming.
    fireEvent.click(screen.getByTestId('ai-index-reset'))
    expect(screen.getByTestId('ai-index-reset-confirm')).toBeVisible()
    expect(screen.queryByTestId('ai-index-reset-error')).toBeNull()
  })
})

function baseState(): AiProvidersSectionState {
  return {
    aiApiKeys: {},
    aiStatus: aiStatusFixture(),
    copyFeedback: null,
    currentSettings: settingsFixture(),
    indexMeta: {
      label: 'Ready',
      tone: 'success',
      description: '1,200 rows indexed.',
    },
    integrationError: null,
    integrationPreview: null,
    noProviders: false,
    providerProbes: {},
    providerTranslations,
    saving: false,
    testingProviderId: null,
    ...handlerFixture(),
  }
}

function renderSection(overrides: Partial<AiProvidersSectionState> = {}) {
  const state: AiProvidersSectionState = {
    ...baseState(),
    ...overrides,
  }

  return render(
    <MemoryRouter>
      <I18nProvider>
        <AiProvidersSection navItem={navItem} state={state} />
      </I18nProvider>
    </MemoryRouter>,
  )
}

function handlerFixture() {
  return {
    onAddProvider: vi.fn().mockResolvedValue(true),
    onApiKeyChange: vi.fn(),
    onClearAiApiKey: vi.fn().mockResolvedValue(undefined),
    onCommitProviders: vi.fn().mockResolvedValue(true),
    onCopyIntegrationValue: vi.fn().mockResolvedValue(undefined),
    onOpenPath: vi.fn(),
    onProviderProbe: vi.fn().mockResolvedValue(undefined),
    onRemoveProvider: vi.fn().mockResolvedValue(true),
    onResetSearchTuning: vi.fn().mockResolvedValue(true),
    onSaveAiApiKey: vi.fn().mockResolvedValue(undefined),
    onSearchTuningChange: vi.fn().mockResolvedValue(true),
    onSelectProvider: vi.fn().mockResolvedValue(true),
    onToggleAi: vi.fn().mockResolvedValue(true),
    onToggleAssistant: vi.fn().mockResolvedValue(true),
    onToggleGpu: vi.fn().mockResolvedValue(true),
    onToggleMcp: vi.fn().mockResolvedValue(true),
    onToggleSkill: vi.fn().mockResolvedValue(true),
    onToggleSemanticIndex: vi.fn().mockResolvedValue(true),
    onUpdateProvider: vi.fn(),
  }
}

function settingsFixture(overrides: Partial<AiSettings> = {}): AiSettings {
  return {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: true,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: true,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    enrichmentEnabled: false,
    enrichmentPlugins: [],
    llmProviderId: 'llm-1',
    embeddingProviderId: 'embed-1',
    retrievalTopK: 8,
    assistantSystemPrompt: '',
    llmProviders: [providerFixture('llm-1', 'llm')],
    embeddingProviders: [providerFixture('embed-1', 'embedding')],
    ...overrides,
  }
}

function providerFixture(
  id: string,
  purpose: AiProviderConfig['purpose'],
): AiProviderConfig {
  return {
    id,
    name: purpose === 'llm' ? 'Local LLM' : 'Local Embeddings',
    purpose,
    requestFormat: 'openai',
    enabled: true,
    baseUrl: 'http://localhost:11434/v1',
    apiKeySaved: id === 'llm-1',
    defaultModel: purpose === 'llm' ? 'gpt-test' : 'text-embedding-test',
    modelCatalog: [],
    temperature: purpose === 'llm' ? 0.7 : null,
    maxTokens: purpose === 'llm' ? 1200 : null,
    dimensions: purpose === 'embedding' ? 1536 : null,
    notes: null,
  }
}

function probeReportFixture(
  overrides: Partial<AiProviderConnectionTestReport> = {},
): AiProviderConnectionTestReport {
  return {
    providerId: 'llm-1',
    purpose: 'llm',
    model: 'local-model',
    ok: true,
    latencyMs: 42,
    capabilities: {
      supportsChat: true,
      supportsEmbeddings: false,
      supportsStreaming: true,
      supportsToolUse: true,
      supportsStructuredOutput: true,
    },
    llmCapabilities: null,
    errorCode: null,
    actionHint: null,
    retryHint: null,
    warnings: [],
    message: 'Reachable',
    ...overrides,
  }
}

function aiStatusFixture(
  overrides: Partial<AiIndexStatus> = {},
): AiIndexStatus {
  return {
    enabled: true,
    assistantEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    state: 'ready',
    ready: true,
    indexedItems: 1200,
    lastIndexedAt: '2026-04-25T12:00:00Z',
    llmProviderId: 'llm-1',
    embeddingProviderId: 'embed-1',
    queuePaused: false,
    queueConcurrency: 1,
    queuedJobs: 0,
    runningJobs: 0,
    failedJobs: 0,
    recentJobs: [],
    semanticSidecarBytes: 2048,
    semanticMetadataBytes: 1024,
    estimatedEmbeddingTokens: 4096,
    warning: null,
    ...overrides,
  }
}

function staticProviderFixture(): AiProviderConfig {
  return {
    id: 'static-in-app',
    name: 'Static (in-app)',
    purpose: 'embedding',
    requestFormat: 'openai',
    enabled: true,
    baseUrl: 'static:in-app',
    apiKeySaved: false,
    defaultModel: 'minishlab/potion-retrieval-32M',
    modelCatalog: [],
    temperature: null,
    maxTokens: null,
    dimensions: 256,
    notes: null,
  }
}

function staticEmbeddingFixture(
  overrides: Partial<StaticEmbeddingStatus> = {},
): StaticEmbeddingStatus {
  return {
    providerId: 'static-in-app',
    modelRepo: 'minishlab/potion-retrieval-32M',
    modelDownloaded: false,
    selected: true,
    isDefault: true,
    dimensions: 256,
    modelSizeBytes: 100_663_296, // ≈ 96 MB
    ...overrides,
  }
}
