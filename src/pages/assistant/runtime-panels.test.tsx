/**
 * @file runtime-panels.test.tsx
 * @description Focused render coverage for the extracted Assistant runtime panels.
 * @module pages/assistant
 *
 * ## Responsibilities
 * - Verify runtime status, provider probe, queued-job, and queue-sidebar rendering after the Assistant route split.
 *
 * ## Not responsible for
 * - Re-testing route-level gating, mutations, or conversation history behavior.
 *
 * ## Dependencies
 * - Depends on the shipped root translator for assistant/common strings.
 *
 * ## Performance notes
 * - Focused render tests keep panel coverage cheap without mounting the full Assistant route.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { createTranslator } from '../../lib/i18n'
import { AssistantQueueSidebar, AssistantRuntimePanels } from './runtime-panels'

const t = createTranslator('en')

describe('assistant runtime panels', () => {
  test('renders runtime status, provider probe, and queued jobs', () => {
    render(
      <MemoryRouter>
        <AssistantRuntimePanels
          activeProfileLabel="Chrome"
          aiMeta={{
            tone: 'warning',
            label: 'Queued',
            description: 'Assistant jobs are queued.',
          }}
          assistantT={t}
          llmProviderAvailable={true}
          llmProviderDisplay="OpenAI / gpt-4.1"
          llmProviderId="openai"
          embeddingProviderId="local-embeddings"
          language="en"
          onProviderProbe={vi.fn()}
          onRefreshQueue={vi.fn()}
          profileScopeLabel="Profile scope"
          profileScopeValue="Chrome"
          providerProbe={{
            providerId: 'openai',
            purpose: 'llm',
            model: 'gpt-4.1',
            ok: false,
            latencyMs: 1234,
            capabilities: {
              supportsChat: true,
              supportsEmbeddings: false,
              supportsStreaming: true,
              supportsToolUse: true,
              supportsStructuredOutput: true,
            },
            warnings: [],
            message: 'Connection needs attention.',
            actionHint: 'Check your API key.',
          }}
          queuedAssistantJobs={[
            {
              id: 42,
              jobType: 'assistant',
              state: 'queued',
              priority: 1,
              attempt: 0,
              maxAttempts: 3,
              queuedAt: '2026-04-01T00:00:00Z',
              availableAt: '2026-04-01T00:00:00Z',
              summary: 'Waiting for provider.',
            },
          ]}
          queuedCount={4}
          queueAction={null}
          runningCount={1}
        />
      </MemoryRouter>,
    )

    expect(screen.getByRole('heading', { name: 'Queued' })).toBeVisible()
    expect(screen.getByText('OpenAI / gpt-4.1')).toBeVisible()
    expect(screen.getAllByText('Chrome').length).toBeGreaterThan(0)
    expect(screen.getByText('Connection needs attention.')).toBeVisible()
    expect(screen.getByText('Check your API key.')).toBeVisible()
    expect(
      screen.getByText(t('assistant.queuedJobLabel', { id: 42 })),
    ).toBeVisible()
    expect(screen.getByText('Waiting for provider.')).toBeVisible()
  })

  test('hides runtime status when metadata is unavailable and omits optional provider hints', () => {
    const { rerender } = render(
      <MemoryRouter>
        <AssistantRuntimePanels
          activeProfileLabel={null}
          aiMeta={null}
          assistantT={t}
          llmProviderAvailable={false}
          llmProviderDisplay="Unavailable"
          llmProviderId="not configured"
          embeddingProviderId="not configured"
          language="en"
          onProviderProbe={vi.fn()}
          onRefreshQueue={vi.fn()}
          profileScopeLabel="Profile scope"
          profileScopeValue="Archive-wide"
          providerProbe={null}
          queuedAssistantJobs={[]}
          queuedCount={0}
          queueAction={null}
          runningCount={0}
        />
      </MemoryRouter>,
    )

    expect(screen.queryByText('Unavailable')).not.toBeInTheDocument()

    rerender(
      <MemoryRouter>
        <AssistantRuntimePanels
          activeProfileLabel={null}
          aiMeta={{
            tone: 'info',
            label: 'Ready',
            description: 'Assistant is ready.',
          }}
          assistantT={t}
          llmProviderAvailable={true}
          llmProviderDisplay="OpenAI / gpt-4.1"
          llmProviderId="openai"
          embeddingProviderId="local-embeddings"
          language="en"
          onProviderProbe={vi.fn()}
          onRefreshQueue={vi.fn()}
          profileScopeLabel="Profile scope"
          profileScopeValue="Archive-wide"
          providerProbe={{
            providerId: 'openai',
            purpose: 'llm',
            model: 'gpt-4.1',
            ok: true,
            latencyMs: 12,
            capabilities: {
              supportsChat: true,
              supportsEmbeddings: false,
              supportsStreaming: true,
              supportsToolUse: true,
              supportsStructuredOutput: true,
            },
            warnings: [],
            message: 'Reachable.',
            actionHint: null,
          }}
          queuedAssistantJobs={[]}
          queuedCount={0}
          queueAction="Refreshing"
          runningCount={0}
        />
      </MemoryRouter>,
    )

    expect(screen.getByRole('heading', { name: 'Ready' })).toBeVisible()
    expect(screen.getByText('Reachable.')).toBeVisible()
    expect(screen.queryByText('Check your API key.')).not.toBeInTheDocument()
  })

  test('renders queue sidebar progress when a queue action is active', () => {
    render(
      <AssistantQueueSidebar
        assistantT={t}
        queuedCount={8}
        queueAction={t('assistant.runningQueuedJobsAction')}
        runningCount={2}
      />,
    )

    expect(screen.getByText(t('assistant.queueBoundary'))).toBeVisible()
    expect(
      screen.getByText(t('assistant.runningQueuedJobsAction')),
    ).toBeVisible()
    expect(
      screen.getByText(
        t('assistant.queueProgressLabel', {
          queued: 8,
          running: 2,
        }),
      ),
    ).toBeVisible()
  })
})
