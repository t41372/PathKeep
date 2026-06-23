import { fireEvent, render, screen, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type {
  AiIndexStatus,
  AiProviderConfig,
  AiProviderConnectionTestReport,
  AiSettings,
} from '../../lib/types'
import {
  AiProvidersSection,
  type AiProvidersSectionState,
} from './ai-providers-section'
import type { SettingsSectionNavItem } from './section-nav-items'

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
      // Both providers persisted + a typed key so save/clear key are enabled.
      // No provider is pre-selected so the radios are unchecked and clicking
      // them fires onSelect.
      aiApiKeys: { 'llm-1': '  secret  ', 'embed-1': '  embed-secret  ' },
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
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

  test('probes a persisted provider and shows reachable latency + error inline', () => {
    const handlers = handlerFixture()
    const { rerender } = renderSection({
      ...handlers,
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
      currentSettings: settingsFixture({ enabled: true }),
    })

    // The Test-connection button is enabled for the persisted provider and
    // routes to the per-purpose probe handler. The first button belongs to the
    // LLM editor list, the second to the embedding editor list.
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
              persistedProviderIds: new Set(['llm-1', 'embed-1']),
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

  test('disables the Test-connection probe for unsaved providers and surfaces a save-first hint', () => {
    // Provider not yet persisted: probe disabled (the backend probes saved
    // config by id, never the in-flight draft) AND a visible inline hint tells
    // the user the next step instead of leaving a silent dead button.
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      persistedProviderIds: new Set(),
      currentSettings: settingsFixture({ enabled: true }),
    })
    const probe = screen.getAllByRole('button', { name: 'Test connection' })[0]
    expect(probe).toBeDisabled()
    // The hint renders as real visible copy (not just an aria attribute), once
    // per editor list (LLM + embedding), and clicking the dead button is inert.
    const hints = screen.getAllByText('Save this provider first to test it.')
    expect(hints.length).toBe(2)
    expect(hints[0]).toBeVisible()
    fireEvent.click(probe)
    expect(handlers.onProviderProbe).not.toHaveBeenCalled()
  })

  test('disables Save key for an unsaved provider but surfaces a save-first hint once a key is typed', () => {
    // The "I typed a key but it never saved" dead end: the backend stores the
    // secret by provider id, which does not exist in saved config until Save
    // settings runs. With a typed key but an UNSAVED provider, Save key is
    // disabled AND the inline hint tells the user to save settings first
    // (mentioning the key is optional for local servers).
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      aiApiKeys: { 'llm-1': 'sk-typed', 'embed-1': 'sk-typed' },
      persistedProviderIds: new Set(),
      currentSettings: settingsFixture({ enabled: true }),
    })
    const save = screen.getAllByRole('button', { name: 'Save key' })[0]
    expect(save).toBeDisabled()
    const hints = screen.getAllByTestId(/^save-key-hint-/)
    expect(hints.length).toBe(2)
    expect(hints[0]).toBeVisible()
    fireEvent.click(save)
    expect(handlers.onSaveAiApiKey).not.toHaveBeenCalled()
  })

  test('omits the Save-key save-first hint when no key has been typed yet (empty field is not a dead end)', () => {
    // An empty field is a transient, self-resolving state — not a misconfigured
    // dead end — so the hint must stay quiet until the user actually types a key.
    renderSection({
      aiApiKeys: {},
      persistedProviderIds: new Set(),
      currentSettings: settingsFixture({ enabled: true }),
    })
    expect(screen.queryByTestId(/^save-key-hint-/)).toBeNull()
  })

  test('probes a persisted provider even while the AI master toggle is OFF (you test before opting in)', () => {
    // BUG B regression: testing a connection must not require AI to be enabled —
    // the user validates an endpoint BEFORE committing to turning AI on. With a
    // persisted provider and AI off, the probe is ENABLED, fires onProbe, and
    // shows NO save-first hint.
    const handlers = handlerFixture()
    renderSection({
      ...handlers,
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
      currentSettings: settingsFixture({ enabled: false }),
    })
    const probe = screen.getAllByRole('button', { name: 'Test connection' })[0]
    expect(probe).toBeEnabled()
    expect(
      screen.queryByText('Save this provider first to test it.'),
    ).toBeNull()
    fireEvent.click(probe)
    expect(handlers.onProviderProbe).toHaveBeenLastCalledWith('llm', 'llm-1')
  })

  test('keeps the probe disabled WITHOUT a save-first hint while another probe is in flight', () => {
    // A transient gate (another probe running) disables every probe button, but
    // it resolves on its own and the in-flight button already shows "Testing…",
    // so we must NOT mislead the user with the save-first hint here even though
    // the providers are persisted.
    renderSection({
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
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
      persistedProviderIds: new Set(['llm-1', 'embed-1']),
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

  test('drives save/reset from configDirty + saving', () => {
    const handlers = handlerFixture()
    const { rerender } = renderSection({
      ...handlers,
      configDirty: false,
      currentSettings: settingsFixture({ enabled: true }),
      saving: false,
    })

    // Clean draft: save + reset are disabled.
    expect(screen.getByTestId('ai-save-config')).toBeDisabled()
    expect(screen.getByTestId('ai-reset-config')).toBeDisabled()

    // Dirty draft: both enable and route to their handlers.
    rerender(
      <MemoryRouter>
        <I18nProvider>
          <AiProvidersSection
            navItem={navItem}
            state={{
              ...baseState(),
              ...handlers,
              configDirty: true,
              currentSettings: settingsFixture({ enabled: true }),
              saving: false,
            }}
          />
        </I18nProvider>
      </MemoryRouter>,
    )
    const save = screen.getByTestId('ai-save-config')
    const reset = screen.getByTestId('ai-reset-config')
    expect(save).toBeEnabled()
    expect(reset).toBeEnabled()
    fireEvent.click(save)
    expect(handlers.onSaveAiConfig).toHaveBeenCalledTimes(1)
    fireEvent.click(reset)
    expect(handlers.onResetAiConfig).toHaveBeenCalledTimes(1)
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

  test('shows the saving label on the config save button while saving', () => {
    renderSection({
      configDirty: true,
      currentSettings: settingsFixture({ enabled: true }),
      saving: true,
    })

    // The save button reflects the in-flight save and stays disabled.
    expect(screen.getByTestId('ai-save-config')).toBeDisabled()
    expect(screen.getByTestId('ai-save-config').textContent).toBe('Saving…')
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
})

function baseState(): AiProvidersSectionState {
  return {
    aiApiKeys: {},
    aiStatus: aiStatusFixture(),
    configDirty: false,
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
    persistedProviderIds: new Set(['llm-1']),
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
    onAddProvider: vi.fn(),
    onApiKeyChange: vi.fn(),
    onClearAiApiKey: vi.fn().mockResolvedValue(undefined),
    onCopyIntegrationValue: vi.fn().mockResolvedValue(undefined),
    onOpenPath: vi.fn(),
    onProviderProbe: vi.fn().mockResolvedValue(undefined),
    onRemoveProvider: vi.fn(),
    onResetAiConfig: vi.fn(),
    onResetSearchTuning: vi.fn(),
    onSaveAiApiKey: vi.fn().mockResolvedValue(undefined),
    onSaveAiConfig: vi.fn().mockResolvedValue(undefined),
    onSearchTuningChange: vi.fn(),
    onSelectProvider: vi.fn(),
    onToggleAi: vi.fn(),
    onToggleAssistant: vi.fn(),
    onToggleGpu: vi.fn(),
    onToggleMcp: vi.fn(),
    onToggleSkill: vi.fn(),
    onToggleSemanticIndex: vi.fn(),
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
