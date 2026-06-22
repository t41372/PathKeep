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
import type { AssistantToolCall } from './use-ai-chat-stream'

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
                    {copy.ranTemplate.replace('{name}', call.name)}
                  </span>
                  <ToolStatusBadge
                    status={status}
                    copy={copy}
                    testId={stepTestId}
                  />
                </div>
                {call.arguments ? (
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
