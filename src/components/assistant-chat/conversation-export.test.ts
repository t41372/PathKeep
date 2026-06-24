/**
 * @file conversation-export.test.ts
 * @description Asserts the Markdown / JSON serializers produce the expected sections and a lossless
 *              round-trip, against the generated strings (not a mock).
 */

import { describe, expect, test } from 'vitest'
import {
  CONVERSATION_EXPORT_SCHEMA_VERSION,
  buildConversationJson,
  buildConversationMarkdown,
  defaultConversationExportName,
  type ConversationExportContext,
  type ConversationExportDocument,
  type ConversationExportLabels,
} from './conversation-export'
import type { ChatMessage } from './use-ai-chat-stream'

const LABELS: ConversationExportLabels = {
  title: 'PathKeep conversation',
  model: 'Model',
  exported: 'Exported',
  modelUnknown: 'Keyword only',
  user: 'You',
  assistant: 'Assistant',
  reasoning: 'Reasoning',
  tools: 'Tools used',
  citations: 'Citations',
  usage: 'Tokens: {prompt} prompt · {completion} completion',
  noAnswer: '(no answer)',
  errorSuffix: '(error)',
  cancelledSuffix: '(stopped)',
}

const EXPORTED_AT = new Date('2026-06-24T10:30:12.000Z')

function ctx(
  overrides: Partial<ConversationExportContext> = {},
): ConversationExportContext {
  return {
    modelLabel: 'LM Studio / qwen3',
    exportedAt: EXPORTED_AT,
    labels: LABELS,
    ...overrides,
  }
}

/** A multi-turn transcript exercising reasoning, search + code tools, citations, and usage. */
function multiTurnConversation(): ChatMessage[] {
  return [
    {
      id: 'u1',
      role: 'user',
      content: 'When did I last read about SQLite?',
    },
    {
      id: 'a1',
      role: 'assistant',
      content: 'You last read about SQLite on June 1st.',
      reasoning: 'Let me search the archive.\nThen summarize.',
      status: 'done',
      usage: { promptTokens: 120, completionTokens: 45 },
      toolCalls: [
        {
          id: 't1',
          name: 'query_history',
          arguments: '{"query":"sqlite"}',
          callId: 'c1',
          status: 'success',
          result: 'Found 3 matching pages about SQLite internals.',
        },
        {
          id: 't2',
          name: 'run_code',
          arguments: '{}',
          callId: 'c2',
          status: 'success',
          codeSource:
            'const rows = await query_history("sqlite")\nreturn rows.length\n',
          hostCalls: [
            {
              function: 'query_history',
              query: 'sqlite',
              plane: 'hybrid',
              limit: 10,
              argsSummary: 'query_history(sqlite)',
              rowCount: 3,
            },
            {
              function: 'fetch_visits',
              requestedIds: 3,
              argsSummary: 'fetch_visits(3)',
              rowCount: 3,
            },
          ],
          limitsHit: 'time',
          result: '3',
        },
      ],
      citations: [
        {
          historyId: 1,
          profileId: 'p1',
          url: 'https://sqlite.org/wal.html',
          title: 'Write-Ahead Logging',
          visitedAt: '2026-06-01T08:00:00Z',
          canonicalUrl: 'https://sqlite.org/wal.html',
        },
        {
          historyId: 2,
          profileId: 'p1',
          url: 'https://example.com/no-title',
          visitedAt: '2026-05-20T09:00:00Z',
        },
      ],
    },
  ]
}

describe('buildConversationMarkdown', () => {
  test('renders header + user/assistant/reasoning/tools/citations/usage sections', () => {
    const md = buildConversationMarkdown(multiTurnConversation(), ctx())

    // Header.
    expect(md).toContain('# PathKeep conversation')
    expect(md).toContain('**Model:** LM Studio / qwen3')
    expect(md).toContain('**Exported:** 2026-06-24T10:30:12.000Z')

    // User turn.
    expect(md).toContain('## You')
    expect(md).toContain('When did I last read about SQLite?')

    // Assistant turn + reasoning quote.
    expect(md).toContain('## Assistant')
    expect(md).toContain('### Reasoning')
    expect(md).toContain('> Let me search the archive.')
    expect(md).toContain('> Then summarize.')

    // Tools: search call (name + args + result) and code run (source + host calls + limit).
    expect(md).toContain('### Tools used')
    expect(md).toContain('`query_history`')
    expect(md).toContain('args: `{"query":"sqlite"}`')
    expect(md).toContain(
      'result: Found 3 matching pages about SQLite internals.',
    )
    expect(md).toContain('`run_code`')
    expect(md).toContain('source:')
    expect(md).toContain('const rows = await query_history("sqlite")')
    expect(md).toContain('“sqlite” (hybrid, 3 rows)')
    expect(md).toContain('fetch_visits (3 rows)')
    expect(md).toContain('limit: `time`')

    // Answer + usage.
    expect(md).toContain('You last read about SQLite on June 1st.')
    expect(md).toContain('Tokens: 120 prompt · 45 completion')

    // Citations (titled + untitled fall back to url).
    expect(md).toContain('### Citations')
    expect(md).toContain(
      '[Write-Ahead Logging](https://sqlite.org/wal.html) — 2026-06-01',
    )
    expect(md).toContain(
      '[https://example.com/no-title](https://example.com/no-title) — 2026-05-20',
    )

    // Terminates with exactly one trailing newline.
    expect(md.endsWith('\n')).toBe(true)
    expect(md.endsWith('\n\n')).toBe(false)
  })

  test('falls back to the unknown-model label and no-answer placeholder', () => {
    const messages: ChatMessage[] = [
      { id: 'u', role: 'user', content: '   ' },
      { id: 'a', role: 'assistant', content: '', status: 'done' },
    ]
    const md = buildConversationMarkdown(messages, ctx({ modelLabel: null }))
    expect(md).toContain('**Model:** Keyword only')
    // Both the blank user prompt and the empty assistant answer render the placeholder.
    expect(md.match(/\(no answer\)/g)?.length).toBe(2)
  })

  test('marks error and cancelled assistant turns and skips empty tool/citation sections', () => {
    const errored: ChatMessage[] = [
      {
        id: 'a',
        role: 'assistant',
        content: 'partial',
        status: 'error',
        toolCalls: [],
        citations: [],
      },
    ]
    expect(buildConversationMarkdown(errored, ctx())).toContain(
      '## Assistant (error)',
    )

    const cancelled: ChatMessage[] = [
      { id: 'a', role: 'assistant', content: 'partial', status: 'cancelled' },
    ]
    const md = buildConversationMarkdown(cancelled, ctx())
    expect(md).toContain('## Assistant (stopped)')
    // No tools / citations headings when those arrays are empty/absent.
    expect(md).not.toContain('### Tools used')
    expect(md).not.toContain('### Citations')
    expect(md).not.toContain('### Reasoning')
  })

  test('renders a tool call with no args and a long result preview is truncated', () => {
    const longResult = 'x'.repeat(400)
    const messages: ChatMessage[] = [
      {
        id: 'a',
        role: 'assistant',
        content: 'ok',
        status: 'done',
        toolCalls: [
          {
            id: 't',
            name: 'no_args_tool',
            arguments: '   ',
            isError: true,
            result: longResult,
          },
        ],
      },
    ]
    const md = buildConversationMarkdown(messages, ctx())
    // Blank args produce no args line.
    expect(md).not.toContain('args: `')
    // Error result is flagged and truncated with an ellipsis.
    expect(md).toContain('result (error): ')
    expect(md).toContain('…')
    expect(md).toContain('### Tools used')
  })

  test('renders a tool call with no result and a host call with no plane', () => {
    const messages: ChatMessage[] = [
      {
        id: 'a',
        role: 'assistant',
        content: 'ok',
        status: 'done',
        toolCalls: [
          {
            id: 't',
            name: 'pending_tool',
            arguments: '{"x":1}',
            // No result at all — exercises the falsy-result branch (no result line emitted).
            hostCalls: [
              {
                // A query host call whose plane was not reported falls back to `?`.
                function: 'query_history',
                query: 'rust',
                argsSummary: 'query_history(rust)',
                rowCount: 0,
              },
            ],
          },
          {
            id: 't2',
            name: 'empty_result_tool',
            arguments: '{}',
            // An all-whitespace result trims to empty → also skips the result line.
            result: '   ',
          },
        ],
      },
    ]
    const md = buildConversationMarkdown(messages, ctx())
    expect(md).toContain('`pending_tool`')
    expect(md).toContain('args: `{"x":1}`')
    expect(md).toContain('“rust” (?, 0 rows)')
    expect(md).not.toContain('result:')
    expect(md).not.toContain('result (error):')
  })

  test('renders a code run with blank source and no host calls', () => {
    const messages: ChatMessage[] = [
      {
        id: 'a',
        role: 'assistant',
        content: 'done',
        status: 'done',
        toolCalls: [
          {
            id: 't',
            name: 'run_code',
            arguments: '{}',
            // An empty codeSource still marks the step as code (presence, not content), but emits
            // no source block.
            codeSource: '   ',
          },
        ],
      },
    ]
    const md = buildConversationMarkdown(messages, ctx())
    expect(md).toContain('`run_code`')
    expect(md).not.toContain('source:')
  })

  test('uses the live clock by default when no exportedAt is supplied', () => {
    const md = buildConversationMarkdown(
      [{ id: 'u', role: 'user', content: 'hi' }],
      {
        modelLabel: 'm',
        labels: LABELS,
      },
    )
    expect(md).toContain('**Exported:** ')
  })
})

describe('buildConversationJson', () => {
  test('round-trips the full message structure', () => {
    const messages = multiTurnConversation()
    const json = buildConversationJson(messages, ctx())
    const parsed = JSON.parse(json) as ConversationExportDocument

    expect(parsed.schemaVersion).toBe(CONVERSATION_EXPORT_SCHEMA_VERSION)
    expect(parsed.exportedAt).toBe('2026-06-24T10:30:12.000Z')
    expect(parsed.modelLabel).toBe('LM Studio / qwen3')
    expect(parsed.messages).toHaveLength(2)

    const assistant = parsed.messages[1]
    expect(assistant.role).toBe('assistant')
    expect(assistant.reasoning).toBe(
      'Let me search the archive.\nThen summarize.',
    )
    expect(assistant.usage).toEqual({ promptTokens: 120, completionTokens: 45 })
    expect(assistant.toolCalls).toHaveLength(2)
    expect(assistant.toolCalls?.[1].codeSource).toContain('query_history')
    expect(assistant.toolCalls?.[1].hostCalls?.[0].query).toBe('sqlite')
    expect(assistant.toolCalls?.[1].limitsHit).toBe('time')
    expect(assistant.citations).toHaveLength(2)
    expect(assistant.citations?.[0].url).toBe('https://sqlite.org/wal.html')

    // Pretty-printed (2-space indent) and newline-terminated.
    expect(json).toContain('\n  "schemaVersion"')
    expect(json.endsWith('\n')).toBe(true)
  })

  test('writes a null modelLabel when none is configured and defaults the clock', () => {
    const json = buildConversationJson([], { labels: LABELS })
    const parsed = JSON.parse(json) as ConversationExportDocument
    expect(parsed.modelLabel).toBeNull()
    expect(parsed.messages).toHaveLength(0)
    expect(typeof parsed.exportedAt).toBe('string')
  })
})

describe('defaultConversationExportName', () => {
  test('embeds a timestamp and the format extension', () => {
    expect(
      defaultConversationExportName(
        'markdown',
        new Date('2026-06-24T14:30:12'),
      ),
    ).toBe('pathkeep-conversation-2026-06-24-143012.md')
    expect(
      defaultConversationExportName('json', new Date('2026-01-02T03:04:05')),
    ).toBe('pathkeep-conversation-2026-01-02-030405.json')
  })

  test('defaults to the live clock', () => {
    expect(defaultConversationExportName('markdown')).toMatch(
      /^pathkeep-conversation-\d{4}-\d{2}-\d{2}-\d{6}\.md$/,
    )
  })
})
