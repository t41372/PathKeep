/**
 * @file tool-call-block.test.tsx
 * @description Coverage for the inline, live tool-use timeline.
 *
 * Proves: renders nothing with no calls; renders each call as a numbered step with the resolved
 * "Ran {name}" label and its arguments in a mono block; omits the args block when arguments are
 * empty; shows a live "Running…" badge while pending; an honest "Done"/"Failed" word + collapsible
 * result once the result lands; the error result defaults OPEN (a failure is never hidden); the
 * `statusOf` fallback for a call missing an explicit status.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { ToolCallBlock, type ToolCallBlockCopy } from './tool-call-block'

const copy: ToolCallBlockCopy = {
  label: 'Tools used',
  ranTemplate: 'Ran {name}',
  runningLabel: 'Running…',
  doneLabel: 'Done',
  failedLabel: 'Failed',
  resultToggleLabel: 'Toggle tool result',
  code: {
    ranLabel: 'Wrote and ran a small program',
    sourceLabel: 'Code the assistant ran',
    sourceToggleLabel: 'Toggle the code the assistant ran',
    hostCallsLabel: 'What it looked up',
    queryRowTemplate:
      'Searched your history for “{query}” — {count} matches ({plane}, limit {limit})',
    fetchRowTemplate: 'Opened {ids} pages — {count} loaded',
    genericRowTemplate: '{fn} · {count} rows',
    limitLabel: 'Safety limit reached',
    limits: {
      time: 'Hit the time limit — this answer may be based on partial results',
      memory:
        'Hit the memory limit — this answer may be based on partial results',
      'host-calls':
        'Hit the query budget — this answer may be based on fewer results',
      output:
        'Output was truncated at the size limit — this answer may be incomplete',
      cancelled: 'Cancelled before it finished — this answer may be incomplete',
    },
  },
}

describe('ToolCallBlock', () => {
  test('renders nothing when there are no tool calls', () => {
    const { container } = render(<ToolCallBlock calls={[]} copy={copy} />)
    expect(container.firstChild).toBeNull()
  })

  test('renders each call as a numbered step with name and arguments', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{"q":"rust"}',
            status: 'pending',
          },
          {
            id: 't2',
            name: 'fetch_visits',
            arguments: '{"ids":[1,2]}',
            status: 'pending',
          },
        ]}
      />,
    )
    expect(screen.getByText('Tools used')).toBeVisible()
    expect(screen.getByText('Ran search_bm25')).toBeVisible()
    expect(screen.getByText('Ran fetch_visits')).toBeVisible()
    expect(screen.getByTestId('tools-args-0')).toHaveTextContent('{"q":"rust"}')
    expect(screen.getByTestId('tools-args-1')).toHaveTextContent(
      '{"ids":[1,2]}',
    )
    expect(screen.getByTestId('tools-step-0')).toBeVisible()
    expect(screen.getByTestId('tools-step-1')).toBeVisible()
    // The visible ordinal is 1-based: a mutant `index + 1` → `index` (rendering "0.") must fail.
    expect(screen.getByTestId('tools-step-0')).toHaveTextContent('1.')
    expect(screen.getByTestId('tools-step-1')).toHaveTextContent('2.')
    expect(screen.getByTestId('tools-step-0')).not.toHaveTextContent('0.')
  })

  test('renders without a testId', () => {
    render(
      <ToolCallBlock
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'query_visits',
            arguments: '{"limit":5}',
            status: 'pending',
          },
        ]}
      />,
    )
    expect(screen.getByText('Ran query_visits')).toBeVisible()
    expect(screen.getByText('{"limit":5}')).toBeVisible()
  })

  test('omits the arguments block when a call has no arguments', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          { id: 't1', name: 'list_domains', arguments: '', status: 'pending' },
        ]}
      />,
    )
    expect(screen.getByText('Ran list_domains')).toBeVisible()
    expect(screen.queryByTestId('tools-args-0')).not.toBeInTheDocument()
  })

  test('shows a live Running badge while a call is pending (no result yet)', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{}',
            status: 'pending',
          },
        ]}
      />,
    )
    const status = screen.getByTestId('tools-step-0-status')
    expect(status).toHaveTextContent('Running…')
    expect(status).toHaveAttribute('aria-label', 'Running…')
    // No result body yet while pending.
    expect(
      screen.queryByTestId('tools-result-0-toggle'),
    ).not.toBeInTheDocument()
    expect(screen.getByTestId('tools-step-0').dataset.status).toBe('pending')
  })

  test('a successful call shows Done and a collapsed result that toggles open', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{"q":"rust"}',
            callId: 'call-1',
            status: 'success',
            isError: false,
            result: 'search_bm25: 3 match(es).',
          },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-status')).toHaveTextContent('Done')
    // Success result is collapsed by default — the body is hidden until toggled.
    expect(screen.queryByTestId('tools-result-0-body')).not.toBeInTheDocument()
    fireEvent.click(screen.getByTestId('tools-result-0-toggle'))
    expect(screen.getByTestId('tools-result-0-body')).toHaveTextContent(
      'search_bm25: 3 match(es).',
    )
    // Toggling again collapses it.
    fireEvent.click(screen.getByTestId('tools-result-0-toggle'))
    expect(screen.queryByTestId('tools-result-0-body')).not.toBeInTheDocument()
  })

  test('a failed call shows Failed and an OPEN error result (a failure is never hidden)', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'no_such_tool',
            arguments: '{}',
            callId: 'call-x',
            status: 'error',
            isError: true,
            result: 'Tool `no_such_tool` failed: unknown tool.',
          },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-status')).toHaveTextContent(
      'Failed',
    )
    expect(screen.getByTestId('tools-step-0').dataset.status).toBe('error')
    // The error body is open by default so the user sees the honest failure immediately.
    expect(screen.getByTestId('tools-result-0-body')).toHaveTextContent(
      'Tool `no_such_tool` failed: unknown tool.',
    )
  })

  test('falls back to deriving status from result/isError when no explicit status is set', () => {
    // A call with a result but no `status` (defensive path): isError true → error word + open body.
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{}',
            result: 'boom',
            isError: true,
          },
          // No status and no result → treated as pending.
          { id: 't2', name: 'query_visits', arguments: '{}' },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-status')).toHaveTextContent(
      'Failed',
    )
    expect(screen.getByTestId('tools-step-0').dataset.status).toBe('error')
    expect(screen.getByTestId('tools-step-1-status')).toHaveTextContent(
      'Running…',
    )
    expect(screen.getByTestId('tools-step-1').dataset.status).toBe('pending')
  })

  test('derives success when a result is present without an error flag or status', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          { id: 't1', name: 'search_bm25', arguments: '{}', result: 'ok rows' },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-status')).toHaveTextContent('Done')
    expect(screen.getByTestId('tools-step-0').dataset.status).toBe('success')
  })

  test('renders a result without a testId (toggle has no test id but still works)', () => {
    render(
      <ToolCallBlock
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{}',
            status: 'success',
            isError: false,
            result: 'ok',
          },
        ]}
      />,
    )
    // The toggle is reachable by its aria-label even with no test id.
    fireEvent.click(screen.getByLabelText('Toggle tool result'))
    expect(screen.getByText('ok')).toBeVisible()
  })

  // ---------------------------------------------------------------------------
  // W-AI-8 WU-5: code-mode (run_code) observability — verbatim source, host-call
  // sub-timeline (composed from STRUCTURED fields), and the limit/error chip.
  // ---------------------------------------------------------------------------

  test('a non-code (search) tool call renders unchanged: args block, no code block', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'search_bm25',
            arguments: '{"q":"rust"}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'search_bm25: 3 match(es).',
          },
        ]}
      />,
    )
    // The W-AI-7 search step is byte-for-byte unchanged: it shows the raw args block and NONE of the
    // code-mode affordances (no verbatim-source block, no host-call timeline, no limit chip).
    expect(screen.getByTestId('tools-args-0')).toHaveTextContent('{"q":"rust"}')
    expect(
      screen.queryByTestId('tools-step-0-source-toggle'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByTestId('tools-step-0-hostcalls'),
    ).not.toBeInTheDocument()
    expect(screen.queryByTestId('tools-step-0-limit')).not.toBeInTheDocument()
  })

  test('a code run renders the verbatim source (collapsed, never truncated) and replaces the args block', () => {
    const source =
      'const r = await query_history({ query: "rust", limit: 8 });\nreturn r.length;'
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{"source":"…"}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: '8 rust pages.',
            codeSource: source,
            hostCalls: [],
          },
        ]}
      />,
    )
    // The args JSON blob is suppressed for a code run (the source IS the transparent record).
    expect(screen.queryByTestId('tools-args-0')).not.toBeInTheDocument()
    // The step header is HUMANIZED: a non-technical user sees "Wrote and ran a small program", NOT
    // the raw "Ran run_code" impl token (B1). The raw token stays honest only in the source block.
    expect(screen.getByText('Wrote and ran a small program')).toBeVisible()
    expect(screen.queryByText('Ran run_code')).not.toBeInTheDocument()
    // The source label is always visible; the body is collapsed by default (lean for long scripts).
    expect(screen.getByText('Code the assistant ran')).toBeVisible()
    expect(
      screen.queryByTestId('tools-step-0-source-body'),
    ).not.toBeInTheDocument()
    // Opening reveals the EXACT source, verbatim and untruncated.
    fireEvent.click(screen.getByTestId('tools-step-0-source-toggle'))
    const body = screen.getByTestId('tools-step-0-source-body')
    expect(body).toHaveTextContent('const r = await query_history')
    expect(body.textContent).toBe(source)
  })

  test('a code run renders a host-call sub-timeline composed from the STRUCTURED fields (localized)', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: 'return 1;',
            hostCalls: [
              {
                function: 'query_history',
                query: 'rust',
                plane: 'hybrid',
                limit: 8,
                argsSummary: 'query="rust" plane=hybrid limit=8',
                rowCount: 12,
              },
              {
                function: 'fetch_visits',
                requestedIds: 3,
                argsSummary: 'ids=3 (capped at 16)',
                rowCount: 3,
              },
            ],
          },
        ]}
      />,
    )
    expect(screen.getByText('What it looked up')).toBeVisible()
    // query_history → HUMANIZED query template, composed from query/count and keeping plane/limit as
    // an honest parenthetical (NOT argsSummary, NOT leading with the `query_history` impl token).
    expect(screen.getByTestId('tools-step-0-hostcall-0')).toHaveTextContent(
      'Searched your history for “rust” — 12 matches (hybrid, limit 8)',
    )
    // fetch_visits → HUMANIZED fetch template (opened N pages, N loaded), from requestedIds + rowCount.
    expect(screen.getByTestId('tools-step-0-hostcall-1')).toHaveTextContent(
      'Opened 3 pages — 3 loaded',
    )
    // The raw impl function tokens never lead the user-facing rows (humanized verbs do).
    expect(screen.getByTestId('tools-step-0-hostcall-0')).not.toHaveTextContent(
      'query_history',
    )
    expect(screen.getByTestId('tools-step-0-hostcall-1')).not.toHaveTextContent(
      'fetch_visits',
    )
    // The non-localized argsSummary debug string is NEVER rendered.
    expect(
      screen.queryByText(/query="rust" plane=hybrid/),
    ).not.toBeInTheDocument()
    expect(screen.queryByText(/capped at 16/)).not.toBeInTheDocument()
  })

  test('a query host call with no plane/limit falls back to empty plane and 0 limit', () => {
    // A defensive query_history record carrying `query` but neither `plane` nor `limit` exercises
    // the `?? ''` / `?? 0` fallbacks (the row still composes from the query template).
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: 'return 1;',
            hostCalls: [
              {
                function: 'query_history',
                query: 'rust',
                argsSummary: 'query="rust"',
                rowCount: 4,
              },
            ],
          },
        ]}
      />,
    )
    // `{plane}` → '' (empty) and `{limit}` → '0'; the query/count still render. Assert on the raw
    // textContent (not the whitespace-normalizing matcher) so the empty `{plane}` segment is exact.
    expect(screen.getByTestId('tools-step-0-hostcall-0').textContent).toBe(
      'Searched your history for “rust” — 4 matches (, limit 0)',
    )
  })

  test('a host call with neither query nor requestedIds falls back to the generic row template', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: 'return 1;',
            hostCalls: [
              {
                function: 'mystery_call',
                argsSummary: 'opaque',
                rowCount: 5,
              },
            ],
          },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-hostcall-0')).toHaveTextContent(
      'mystery_call · 5 rows',
    )
  })

  test('a query host call renders the EXACT query verbatim — $-sequences and brace tokens never corrupt (F1)', () => {
    // The transparency contract requires the host-call row to show the query that ACTUALLY ran, byte
    // for byte. A naive `String.replace('{query}', value)` would (1) let JS interpret `$&` / `$\`` /
    // `$$` in the query as replacement-pattern specials, and (2) re-scan injected text so a query
    // containing a LATER token like `{count}` would bleed into the next substitution. This query
    // exercises BOTH hazards at once and must render literally.
    const trickyQuery = 'cheap $& deal {count} $`$$ $1'
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: 'return 1;',
            hostCalls: [
              {
                function: 'query_history',
                query: trickyQuery,
                plane: 'hybrid',
                limit: 8,
                argsSummary: 'query=…',
                rowCount: 7,
              },
            ],
          },
        ]}
      />,
    )
    // The query renders VERBATIM and `{count}` resolves to the real row count (7) exactly once — the
    // literal `{count}` inside the query did NOT get re-substituted, and no `$`-sequence was eaten.
    expect(screen.getByTestId('tools-step-0-hostcall-0').textContent).toBe(
      `Searched your history for “${trickyQuery}” — 7 matches (hybrid, limit 8)`,
    )
  })

  test('an unrecognized {token} in a row template is left intact (no value to fill)', () => {
    // Defensive: if a translation introduces a placeholder the renderer has no value for, the safe
    // single-pass fill leaves the literal `{token}` rather than throwing or emitting `undefined`.
    const copyWithStrayToken: ToolCallBlockCopy = {
      ...copy,
      code: { ...copy.code, genericRowTemplate: '{fn} · {count} · {stray}' },
    }
    render(
      <ToolCallBlock
        testId="tools"
        copy={copyWithStrayToken}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: 'return 1;',
            hostCalls: [
              { function: 'mystery_call', argsSummary: 'opaque', rowCount: 5 },
            ],
          },
        ]}
      />,
    )
    expect(screen.getByTestId('tools-step-0-hostcall-0').textContent).toBe(
      'mystery_call · 5 · {stray}',
    )
  })

  test('a code run with no host calls renders the source but no timeline', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: 'return 1;',
            // hostCalls omitted entirely (undefined → defaults to [])
          },
        ]}
      />,
    )
    expect(screen.getByText('Code the assistant ran')).toBeVisible()
    expect(
      screen.queryByTestId('tools-step-0-hostcalls'),
    ).not.toBeInTheDocument()
  })

  test('renders a localized limit chip for each of the five kinds', () => {
    const cases: Array<{
      limit: 'time' | 'memory' | 'host-calls' | 'output' | 'cancelled'
      label: string
    }> = [
      {
        limit: 'time',
        label:
          'Hit the time limit — this answer may be based on partial results',
      },
      {
        limit: 'memory',
        label:
          'Hit the memory limit — this answer may be based on partial results',
      },
      {
        limit: 'host-calls',
        label:
          'Hit the query budget — this answer may be based on fewer results',
      },
      {
        limit: 'output',
        label:
          'Output was truncated at the size limit — this answer may be incomplete',
      },
      {
        limit: 'cancelled',
        label: 'Cancelled before it finished — this answer may be incomplete',
      },
    ]
    for (const { limit, label } of cases) {
      const { unmount } = render(
        <ToolCallBlock
          testId="tools"
          copy={copy}
          calls={[
            {
              id: 't1',
              name: 'run_code',
              arguments: '{}',
              callId: 'c1',
              status: 'success',
              isError: false,
              result: 'partial',
              codeSource: 'while(true){}',
              hostCalls: [],
              limitsHit: limit,
            },
          ]}
        />,
      )
      const chip = screen.getByTestId('tools-step-0-limit')
      expect(chip).toHaveTextContent(label)
      // The chip is labeled for assistive tech (prefix + the localized reason). The aria-label carries
      // the consequence too (the reason text spells out that the answer may be partial/incomplete).
      expect(chip).toHaveAttribute(
        'aria-label',
        `Safety limit reached: ${label}`,
      )
      unmount()
    }
  })

  test('a code run that stayed within every bound shows no limit chip', () => {
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: 'return 1;',
            hostCalls: [],
          },
        ]}
      />,
    )
    expect(screen.queryByTestId('tools-step-0-limit')).not.toBeInTheDocument()
  })

  test('a code run renders without a testId: source, host calls, and limit chip reachable by text/role', () => {
    // No testId → every `testId ? \`${testId}-…\` : undefined` branch in CodeRunDetails takes the
    // undefined arm. The affordances are still reachable by their aria-label / visible text.
    render(
      <ToolCallBlock
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: 'return 1;',
            hostCalls: [
              {
                function: 'query_history',
                query: 'rust',
                plane: 'bm25',
                limit: 5,
                argsSummary: 'query="rust" plane=bm25 limit=5',
                rowCount: 2,
              },
            ],
            limitsHit: 'memory',
          },
        ]}
      />,
    )
    // The source toggle is reachable by its aria-label and reveals the verbatim source.
    fireEvent.click(screen.getByLabelText('Toggle the code the assistant ran'))
    expect(screen.getByText('return 1;')).toBeVisible()
    // The host-call row and the limit chip render from copy with no test id.
    expect(
      screen.getByText(
        'Searched your history for “rust” — 2 matches (bm25, limit 5)',
      ),
    ).toBeVisible()
    expect(
      screen.getByLabelText(
        'Safety limit reached: Hit the memory limit — this answer may be based on partial results',
      ),
    ).toBeVisible()
  })

  test('an empty-string code source still renders as a code run (presence, not truthiness)', () => {
    // codeSource === '' is a (degenerate but possible) code run: it must NOT fall through to the args
    // block. The renderer keys on `codeSource !== undefined`, so an empty source is still a code run.
    render(
      <ToolCallBlock
        testId="tools"
        copy={copy}
        calls={[
          {
            id: 't1',
            name: 'run_code',
            arguments: '{"source":"x"}',
            callId: 'c1',
            status: 'success',
            isError: false,
            result: 'done',
            codeSource: '',
            hostCalls: [],
          },
        ]}
      />,
    )
    expect(screen.queryByTestId('tools-args-0')).not.toBeInTheDocument()
    expect(screen.getByText('Code the assistant ran')).toBeVisible()
  })
})
