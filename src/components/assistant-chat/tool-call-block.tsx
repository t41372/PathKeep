/**
 * @file tool-call-block.tsx
 * @description Inline, live timeline of the tool calls a turn ran (transparency contract).
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Render each `toolCall` as a step the user can see: tool name + streamed args (mono), in request
 *   order. This is the trust/transparency surface — the user always sees what the assistant ran.
 * - Show the call's lifecycle inline (lobehub / LM Studio / Claude style): a live spinner while the
 *   result is `pending`, the executed result (collapsible, monospace) on `success`, and an HONEST
 *   error state on `isError` (the failed result text is shown truthfully, never as fake success).
 * - For a code-mode (`run_code`) step (W-AI-8 WU-5): additionally render the VERBATIM script the
 *   assistant wrote (collapsible-but-always-labeled — never truncated, since the user must see
 *   exactly what ran), a host-call sub-timeline composed from the STRUCTURED `HostCallRecord` fields
 *   (translatable, never the raw `argsSummary` debug string), and a localized limit/error chip when a
 *   hard sandbox bound fired.
 * - Stay quiet when no tool was called (most turns), so the answer stays the focus.
 *
 * ## Not responsible for
 * - Executing tools — the worker does that; this is pure presentation of the streamed steps.
 * - Citations / evidence rows — those render in the `PaperAssistantMessage` evidence panel from the
 *   terminal `citations` chunk; this block is the per-call run log only.
 *
 * ## Performance
 * - `memo`'d; one pre-wrapped text node per result (no per-token DOM). The active turn re-renders at
 *   most once per frame (the hook's rAF flush), so a streaming result stays fluid.
 */

import { memo, useState } from 'react'
import { cn } from '@/lib/cn'
import type { HostCallRecord, LimitsHit } from '@/lib/types'
import type { AssistantToolCall } from './use-ai-chat-stream'

/**
 * Code-mode (`run_code`) observability copy (W-AI-8 WU-5). Lives inside `ToolCallBlockCopy` so a code
 * step can render its verbatim source + host-call timeline + limit chip from one copy bundle. The
 * host-call row templates are composed from the STRUCTURED `HostCallRecord` fields so they localize.
 */
export interface ToolCallCodeCopy {
  /**
   * Humanized step header for a code run, e.g. "Wrote and ran a small program". Replaces the raw
   * `Ran run_code` header so a non-technical user is not shown the implementation tool name (the raw
   * `run_code` token + the verbatim JS stay honest in the expandable source block below). A search
   * tool keeps the `ranTemplate` header.
   */
  ranLabel: string
  /** Label above the verbatim source block, e.g. "Code the assistant ran". */
  sourceLabel: string
  /** aria-label for the source expand/collapse toggle. */
  sourceToggleLabel: string
  /** Label above the host-call sub-timeline, e.g. "What it looked up". */
  hostCallsLabel: string
  /**
   * `query_history` row template — leads with a plain-language verb and keeps the implementation
   * detail (search plane / limit) as an honest parenthetical. Placeholders: `{query}` (verbatim),
   * `{count}` (rows returned), `{plane}` (lowercase plane token), `{limit}`.
   */
  queryRowTemplate: string
  /**
   * `fetch_visits` row template — a plain-language verb over `{ids}` (requested page count) and
   * `{count}` (rows actually loaded).
   */
  fetchRowTemplate: string
  /**
   * Structured fallback row for an UNKNOWN host function (kept technical on purpose — there is no
   * humanized verb for an unknown call). Placeholders: `{fn}`/`{count}`.
   */
  genericRowTemplate: string
  /** aria-label prefix for the limit chip, e.g. "Safety limit reached". */
  limitLabel: string
  /** Localized label per hard-limit kind (mirrors the backend `LimitsHit` kebab tokens). */
  limits: Record<LimitsHit, string>
}

export interface ToolCallBlockCopy {
  /** Section label, e.g. "Tools used". */
  label: string
  /** Per-step "ran {name}" template with `{name}` placeholder. */
  ranTemplate: string
  /** Status word shown while a call's result has not landed yet, e.g. "Running…". */
  runningLabel: string
  /** Status word shown when a call's result came back successfully, e.g. "Done". */
  doneLabel: string
  /** Status word shown when a call FAILED (honest), e.g. "Failed". */
  failedLabel: string
  /** aria-label for the per-result expand/collapse toggle. */
  resultToggleLabel: string
  /** Code-mode (`run_code`) observability copy (W-AI-8 WU-5). */
  code: ToolCallCodeCopy
}

export interface ToolCallBlockProps {
  calls: readonly AssistantToolCall[]
  copy: ToolCallBlockCopy
  testId?: string
}

/** Resolve a call's lifecycle: a result present means it landed; otherwise it is still pending. */
function statusOf(call: AssistantToolCall): 'pending' | 'success' | 'error' {
  if (call.status) return call.status
  // Defensive fallback for any call missing an explicit status (plain path never streams a result).
  return call.result !== undefined
    ? call.isError
      ? 'error'
      : 'success'
    : 'pending'
}

/**
 * Single-pass, `$`-safe template fill (W-AI-8 WU-5, F1). A naive chain of
 * `String.prototype.replace('{token}', value)` is WRONG for transparency copy two ways: (1) a string
 * `value` lets JS interpret `$`-sequences (`$&`, `` $` ``, `$'`, `$$`, `$n`) in the replacement, so a
 * verbatim query like `cheap $& deal` would render corrupted; (2) chained replaces re-scan text
 * ALREADY substituted, so a query that itself contains a later token like `{count}` would bleed into
 * the next replace. This helper walks every `{token}` occurrence in ONE pass over the template and
 * substitutes from `values` literally — injected content is copied verbatim and never re-scanned, so
 * the host-call row always shows the EXACT query/values that ran.
 */
function fillTemplate(
  template: string,
  values: Readonly<Record<string, string>>,
): string {
  return template.replace(/\{(\w+)\}/g, (match, token: string) =>
    Object.prototype.hasOwnProperty.call(values, token) ? values[token] : match,
  )
}

/**
 * Compose one host-call row from the STRUCTURED `HostCallRecord` fields (W-AI-8 WU-5), so the row is
 * fully localized, leads with a plain-language verb, and never leaks the non-localized `argsSummary`
 * debug string. A `query_history` call (it carries `query`) uses the humanized query template
 * (searched … — N matches, plane/limit kept as an honest parenthetical); a `fetch_visits` call (it
 * carries `requestedIds`) uses the humanized fetch template (opened N pages); any other call falls
 * back to the structured generic template (fn · rows) — there is no humanized verb for an unknown
 * function, so the technical row stays honest. All substitution is `$`-safe + single-pass (F1).
 */
function formatHostCall(
  record: HostCallRecord,
  copy: ToolCallCodeCopy,
): string {
  if (record.query !== undefined) {
    return fillTemplate(copy.queryRowTemplate, {
      query: record.query,
      plane: record.plane ?? '',
      limit: String(record.limit ?? 0),
      count: String(record.rowCount),
    })
  }
  if (record.requestedIds !== undefined) {
    return fillTemplate(copy.fetchRowTemplate, {
      ids: String(record.requestedIds),
      count: String(record.rowCount),
    })
  }
  return fillTemplate(copy.genericRowTemplate, {
    fn: record.function,
    count: String(record.rowCount),
  })
}

/** One step's collapsible result body (monospace). Defaults open on error so a failure is visible. */
const ToolResult = memo(function ToolResult({
  result,
  isError,
  toggleLabel,
  testId,
}: {
  result: string
  isError: boolean
  toggleLabel: string
  testId?: string
}) {
  const [open, setOpen] = useState(isError)
  return (
    <div className="mt-[4px]">
      <button
        type="button"
        aria-expanded={open}
        aria-label={toggleLabel}
        onClick={() => setOpen((value) => !value)}
        data-testid={testId ? `${testId}-toggle` : undefined}
        className={cn(
          'text-ink-faint enabled:hover:text-ink-secondary flex items-center gap-[6px]',
          'font-mono text-[10px] uppercase tracking-[0.08em] transition-colors duration-150',
        )}
      >
        <span aria-hidden="true" className="text-[11px]">
          {open ? '−' : '+'}
        </span>
        <span>{toggleLabel}</span>
      </button>
      {open ? (
        <pre
          data-testid={testId ? `${testId}-body` : undefined}
          className={cn(
            'mt-[4px] overflow-x-auto whitespace-pre-wrap rounded-[2px] px-[8px] py-[6px]',
            'font-mono text-[10.5px] leading-[1.5]',
            isError
              ? cn(
                  'text-ink-secondary border-l-[2px] border-[color:var(--error)]',
                  'bg-[color-mix(in_srgb,var(--error)_8%,var(--bg-paper))]',
                )
              : 'text-ink-muted bg-card-paper',
          )}
        >
          {result}
        </pre>
      ) : null}
    </div>
  )
})

/**
 * Code-mode transparency block (W-AI-8 WU-5): the verbatim source the assistant ran, its host-call
 * sub-timeline, and a limit/error chip. Rendered only for a `run_code` step (a call carrying
 * `codeSource`). The source is NEVER truncated — collapsible-but-labeled, so a long script stays
 * scrollable; it defaults COLLAPSED (long scripts shouldn't dominate the answer) with the label
 * always visible so the user can always open the exact code that ran.
 */
const CodeRunDetails = memo(function CodeRunDetails({
  source,
  hostCalls,
  limitsHit,
  copy,
  testId,
}: {
  source: string
  hostCalls: readonly HostCallRecord[]
  limitsHit?: LimitsHit
  copy: ToolCallCodeCopy
  testId?: string
}) {
  const [open, setOpen] = useState(false)
  return (
    <div className="mt-[6px] flex flex-col gap-[6px]">
      <div>
        <button
          type="button"
          aria-expanded={open}
          aria-label={copy.sourceToggleLabel}
          onClick={() => setOpen((value) => !value)}
          data-testid={testId ? `${testId}-source-toggle` : undefined}
          className={cn(
            'text-ink-faint enabled:hover:text-ink-secondary flex items-center gap-[6px]',
            'font-mono text-[10px] uppercase tracking-[0.08em] transition-colors duration-150',
          )}
        >
          <span aria-hidden="true" className="text-[11px]">
            {open ? '−' : '+'}
          </span>
          <span>{copy.sourceLabel}</span>
        </button>
        {open ? (
          <pre
            data-testid={testId ? `${testId}-source-body` : undefined}
            className={cn(
              // Scrollable in BOTH axes + capped height so a large script never blows out the
              // timeline; the source itself is verbatim and never truncated (transparency contract).
              'text-ink-muted bg-card-paper mt-[4px] max-h-[260px] overflow-auto rounded-[2px]',
              'px-[8px] py-[6px] font-mono text-[10.5px] leading-[1.5] whitespace-pre',
            )}
          >
            {source}
          </pre>
        ) : null}
      </div>
      {hostCalls.length > 0 ? (
        <div>
          <div className="text-ink-faint mb-[4px] font-mono text-[10px] uppercase tracking-[0.08em]">
            {copy.hostCallsLabel}
          </div>
          <ol
            data-testid={testId ? `${testId}-hostcalls` : undefined}
            className="flex flex-col gap-[3px]"
          >
            {hostCalls.map((record, index) => (
              <li
                key={index}
                data-testid={testId ? `${testId}-hostcall-${index}` : undefined}
                className="text-ink-muted font-mono text-[10.5px] leading-[1.5]"
              >
                {formatHostCall(record, copy)}
              </li>
            ))}
          </ol>
        </div>
      ) : null}
      {limitsHit ? (
        <span
          data-testid={testId ? `${testId}-limit` : undefined}
          aria-label={`${copy.limitLabel}: ${copy.limits[limitsHit]}`}
          className={cn(
            'inline-flex w-fit items-center gap-[5px] rounded-[2px] px-[7px] py-[2px]',
            'font-mono text-[10px] uppercase tracking-[0.06em]',
            'text-ink-secondary border-l-[2px] border-[color:var(--warning)]',
            'bg-[color-mix(in_srgb,var(--warning)_10%,var(--bg-paper))]',
          )}
        >
          <span aria-hidden="true" className="text-[11px] leading-none">
            ⚠
          </span>
          <span>{copy.limits[limitsHit]}</span>
        </span>
      ) : null}
    </div>
  )
})

/** Renders the tool-use timeline; renders nothing when there are no calls. */
export const ToolCallBlock = memo(function ToolCallBlock({
  calls,
  copy,
  testId,
}: ToolCallBlockProps) {
  if (calls.length === 0) return null

  return (
    <div
      data-testid={testId}
      className={cn(
        'rounded-paper border-border-light bg-paper border',
        'animate-[pk-fade-in_200ms_ease]',
        'px-[12px] py-[10px]',
      )}
    >
      <div className="text-ink-faint mb-[8px] font-mono text-[10px] uppercase tracking-[0.08em]">
        {copy.label}
      </div>
      <ol className="flex flex-col gap-[8px]">
        {calls.map((call, index) => {
          const status = statusOf(call)
          const stepTestId = testId ? `${testId}-step-${index}` : undefined
          return (
            <li
              key={call.id}
              data-testid={stepTestId}
              data-status={status}
              className="grid grid-cols-[16px_1fr] gap-[8px]"
            >
              <span
                aria-hidden="true"
                className="text-accent mt-[1px] font-mono text-[11px]"
              >
                {index + 1}.
              </span>
              <div className="min-w-0">
                <div className="flex items-center gap-[8px]">
                  <span className="text-ink-secondary font-mono text-[11.5px]">
                    {/*
                      A code run gets a HUMANIZED header ("Wrote and ran a small program") instead of
                      the raw "Ran run_code": a non-technical user should not be shown the impl tool
                      name. The raw `run_code` token + the verbatim JS stay honest in the source block
                      below. A search tool keeps the existing "Ran {name}" header (W-AI-7 unchanged).
                    */}
                    {call.codeSource !== undefined
                      ? copy.code.ranLabel
                      : copy.ranTemplate.replace('{name}', () => call.name)}
                  </span>
                  <ToolStatusBadge
                    status={status}
                    copy={copy}
                    testId={stepTestId}
                  />
                </div>
                {/*
                  A code-mode (`run_code`) step renders its VERBATIM source + host-call timeline +
                  limit chip instead of the raw tool args JSON: the source IS the transparent record
                  of what ran, so the JSON-arguments blob would be redundant noise. A non-code tool
                  (search) keeps the existing args block, byte-for-byte unchanged (W-AI-7).
                */}
                {call.codeSource !== undefined ? (
                  <CodeRunDetails
                    source={call.codeSource}
                    hostCalls={call.hostCalls ?? []}
                    limitsHit={call.limitsHit}
                    copy={copy.code}
                    testId={stepTestId}
                  />
                ) : call.arguments ? (
                  <pre
                    data-testid={testId ? `${testId}-args-${index}` : undefined}
                    className={cn(
                      'text-ink-muted mt-[4px] overflow-x-auto whitespace-pre-wrap',
                      'bg-card-paper rounded-[2px] px-[8px] py-[6px]',
                      'font-mono text-[10.5px] leading-[1.5]',
                    )}
                  >
                    {call.arguments}
                  </pre>
                ) : null}
                {call.result !== undefined ? (
                  <ToolResult
                    result={call.result}
                    isError={status === 'error'}
                    toggleLabel={copy.resultToggleLabel}
                    testId={testId ? `${testId}-result-${index}` : undefined}
                  />
                ) : null}
              </div>
            </li>
          )
        })}
      </ol>
    </div>
  )
})

/** A small inline status chip: a live spinner while pending, an honest done/failed word otherwise. */
function ToolStatusBadge({
  status,
  copy,
  testId,
}: {
  status: 'pending' | 'success' | 'error'
  copy: ToolCallBlockCopy
  testId?: string
}) {
  if (status === 'pending') {
    return (
      <span
        data-testid={testId ? `${testId}-status` : undefined}
        role="status"
        aria-label={copy.runningLabel}
        className="text-ink-faint flex items-center gap-[5px] font-mono text-[10px]"
      >
        <span
          aria-hidden="true"
          className="bg-accent inline-block h-[5px] w-[5px] rounded-full animate-[pk-pulse_1.2s_ease-in-out_infinite]"
        />
        <span>{copy.runningLabel}</span>
      </span>
    )
  }
  const isError = status === 'error'
  return (
    <span
      data-testid={testId ? `${testId}-status` : undefined}
      className={cn(
        'font-mono text-[10px] uppercase tracking-[0.08em]',
        isError ? 'text-[color:var(--error)]' : 'text-ink-faint',
      )}
    >
      {isError ? copy.failedLabel : copy.doneLabel}
    </span>
  )
}
