/**
 * This test file protects the Integrations-owned AI artifact review surface.
 *
 * Why this file exists:
 * - MCP and skill previews are manual-review contracts, so copy/open actions must stay wired through the route state.
 * - Integrations intentionally reuses Settings AI state; these focused tests catch render-only drift without booting the whole route.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions on review visibility and handler calls rather than decorative panel structure.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { AiProvidersSectionState } from '../settings/ai-providers-section'
import { AiIntegrationReviewSection } from './ai-integration-review-section'

const baseState = (
  patch: Partial<AiProvidersSectionState> = {},
): AiProvidersSectionState =>
  ({
    aiApiKeys: {},
    aiStatus: null,
    configDirty: false,
    copyFeedback: null,
    currentSettings: {
      enabled: true,
      assistantEnabled: false,
      semanticIndexEnabled: false,
      mcpEnabled: true,
      skillEnabled: true,
      autoIndexAfterBackup: false,
      jobQueuePaused: false,
      jobQueueConcurrency: 1,
      enrichmentEnabled: false,
      enrichmentPlugins: [],
      retrievalTopK: 6,
      assistantSystemPrompt: '',
      llmProviders: [],
      embeddingProviders: [],
    },
    indexMeta: null,
    integrationError: null,
    integrationPreview: null,
    noProviders: false,
    persistedProviderIds: new Set(),
    providerTranslations: {} as never,
    saving: false,
    onAddProvider: vi.fn(),
    onApiKeyChange: vi.fn(),
    onClearAiApiKey: vi.fn(),
    onCopyIntegrationValue: vi.fn(),
    onOpenPath: vi.fn(),
    onRemoveProvider: vi.fn(),
    onResetAiConfig: vi.fn(),
    onSaveAiApiKey: vi.fn(),
    onSaveAiConfig: vi.fn(),
    onSelectProvider: vi.fn(),
    onToggleAi: vi.fn(),
    onUpdateProvider: vi.fn(),
    ...patch,
  }) as AiProvidersSectionState

function renderSection(state: AiProvidersSectionState) {
  return render(
    <I18nProvider>
      <MemoryRouter>
        <AiIntegrationReviewSection state={state} />
      </MemoryRouter>
    </I18nProvider>,
  )
}

const previewFixture = (overrides = {}) => ({
  mcpCommand: 'pathkeep mcp serve',
  consentSummary: 'Manual localhost bridge preview only.',
  manualSteps: ['Review the generated file before use.'],
  capabilityNotes: ['Read-only local history lookup.'],
  scopeBoundary: ['No cloud upload.'],
  auditTrace: ['Generated during Settings review.'],
  generatedFiles: [
    {
      relativePath: 'integrations/pathkeep-mcp.json',
      absolutePath: '/tmp/pathkeep/integrations/pathkeep-mcp.json',
      purpose: 'MCP JSON',
      contents: '{"command":"pathkeep"}',
    },
  ],
  warnings: ['Keep the local server disabled when unused.'],
  ...overrides,
})

describe('AiIntegrationReviewSection', () => {
  test('renders nothing until current AI settings exist', () => {
    const { container } = render(
      <I18nProvider>
        <MemoryRouter>
          <AiIntegrationReviewSection
            state={baseState({ currentSettings: null })}
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expect(container).toBeEmptyDOMElement()
  })

  test('shows the honest loading state until a preview is available', () => {
    renderSection(baseState())

    expect(screen.getByText('Preparing integration preview')).toBeVisible()
    // No roadmap copy leaks into the live surface.
    expect(screen.queryByText('Coming in v0.3')).toBeNull()
  })

  test('renders the live MCP + generated-file review and wires copy/open', () => {
    const onCopyIntegrationValue = vi.fn().mockResolvedValue(undefined)
    const onOpenPath = vi.fn()
    renderSection(
      baseState({
        onCopyIntegrationValue,
        onOpenPath,
        integrationPreview: previewFixture(),
      }),
    )

    // The real consent summary + MCP command + generated files render.
    expect(
      screen.getByText('Manual localhost bridge preview only.'),
    ).toBeVisible()
    expect(screen.getByText('pathkeep mcp serve')).toBeVisible()
    expect(screen.getByText('MCP command')).toBeVisible()
    expect(screen.getByText('Generated files')).toBeVisible()
    expect(screen.getByText('Read-only local history lookup.')).toBeVisible()

    // The artifact viewer routes copy/open through the route-owned handlers.
    fireEvent.click(screen.getByRole('button', { name: 'Open path' }))
    expect(onOpenPath).toHaveBeenCalledWith(
      '/tmp/pathkeep/integrations/pathkeep-mcp.json',
    )
    // Copy buttons (path + contents) route to onCopyIntegrationValue.
    screen
      .getAllByRole('button', { name: 'Copy' })
      .forEach((button) => fireEvent.click(button))
    expect(onCopyIntegrationValue).toHaveBeenCalled()
  })

  test('surfaces a preview error honestly', () => {
    renderSection(
      baseState({
        integrationError: 'local preview is unavailable',
      }),
    )

    expect(screen.getByText('local preview is unavailable')).toBeVisible()
    expect(screen.getByText('Integration preview unavailable')).toBeVisible()
  })

  test('shows the localized copy-failed message when a copy fails', () => {
    renderSection(
      baseState({
        integrationPreview: previewFixture(),
        copyFeedback: {
          key: 'contents:integrations/pathkeep-mcp.json',
          tone: 'error',
        },
      }),
    )

    expect(screen.getByText("Couldn't copy that artifact.")).toBeVisible()
  })

  test('renders without a generated-file viewer when none are produced', () => {
    renderSection(
      baseState({
        integrationPreview: previewFixture({
          generatedFiles: [],
          warnings: [],
        }),
      }),
    )

    expect(screen.getByText('pathkeep mcp serve')).toBeVisible()
    expect(screen.queryByRole('button', { name: 'Open path' })).toBeNull()
  })
})
