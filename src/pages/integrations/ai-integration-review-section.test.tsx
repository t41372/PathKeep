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

import { render, screen } from '@testing-library/react'
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

  test('shows deferred MCP and skill artifact placeholders', () => {
    const onCopyIntegrationValue = vi.fn().mockResolvedValue(undefined)
    const onOpenPath = vi.fn()
    renderSection(
      baseState({
        onCopyIntegrationValue,
        onOpenPath,
        integrationPreview: {
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
        },
      }),
    )

    expect(screen.getByText('AI integrations are coming later')).toBeVisible()
    expect(
      screen.getByText(
        'MCP commands and skill files depend on the same assistant and embedding runtime. They stay visible here for the roadmap, but v0.2 does not generate or install them.',
      ),
    ).toBeVisible()
    expect(screen.getByText('MCP command')).toBeVisible()
    expect(screen.getByText('Generated files')).toBeVisible()
    expect(screen.getAllByText('Coming in v0.3')).toHaveLength(3)

    expect(onCopyIntegrationValue).not.toHaveBeenCalled()
    expect(onOpenPath).not.toHaveBeenCalled()
  })

  test('ignores preview errors and generated files while AI integrations are deferred', () => {
    const { rerender } = renderSection(
      baseState({
        integrationError: 'local preview is unavailable',
      }),
    )

    expect(screen.queryByText('local preview is unavailable')).toBeNull()
    expect(screen.getByText('AI integrations are coming later')).toBeVisible()

    rerender(
      <I18nProvider>
        <MemoryRouter>
          <AiIntegrationReviewSection
            state={baseState({
              integrationPreview: {
                mcpCommand: 'pathkeep mcp serve',
                consentSummary: 'Manual localhost bridge preview only.',
                manualSteps: ['Review the generated file before use.'],
                capabilityNotes: ['Read-only local history lookup.'],
                scopeBoundary: ['No cloud upload.'],
                auditTrace: ['Generated during Settings review.'],
                generatedFiles: [],
                warnings: [],
              },
            })}
          />
        </MemoryRouter>
      </I18nProvider>,
    )

    expect(screen.getByText('AI integrations are coming later')).toBeVisible()
    expect(screen.queryByText('Open path')).toBeNull()
  })
})
