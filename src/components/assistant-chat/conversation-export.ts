/**
 * @file conversation-export.ts
 * @description Pure serializers that turn the in-memory chat transcript into a downloadable
 *              Markdown (human-readable) or JSON (structured/lossless) document.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - `buildConversationMarkdown`: render a clean, readable transcript — a title/header (model +
 *   export date), then each turn humanized like the in-app timeline: the user prompt, the
 *   assistant's reasoning (as a `> ` quote), the answer, tool calls (name + args + a short result,
 *   plus `run_code` source / host-call timeline when present), per-turn token usage, and a
 *   Citations section. No raw machine tokens are dumped unreadably.
 * - `buildConversationJson`: serialize the full `ChatMessage[]` (role, content, reasoning,
 *   toolCalls, toolResults, citations, usage, status) plus a small envelope (schema version,
 *   model, exportedAt) — lossless and pretty-printed so a round-trip parse recovers the shape.
 * - `defaultConversationExportName`: a sensible default filename embedding a timestamp.
 *
 * ## Not responsible for
 * - Reading the transcript from state, picking a file path, or writing to disk — the orchestration
 *   hook + the route own that. These functions are intentionally pure (string in → string out) so
 *   they are trivially testable and never touch the main-thread-heavy paths.
 * - Localizing the document body. The transcript is the user's own content; only the small set of
 *   section labels is parameterized via `ConversationExportLabels` so the caller passes localized
 *   copy. (Filenames stay locale-stable on purpose — see `defaultConversationExportName`.)
 *
 * ## Why this exists
 * The user asked to export the CURRENT conversation as Markdown or JSON, capturing everything the
 * chat surface shows. Centralizing the projection here keeps the route thin and lets the gate prove
 * the exact output shape against assertions (not a mock).
 */

import type { ChatMessage } from './use-ai-chat-stream'

/** The two supported export shapes. */
export type ConversationExportFormat = 'markdown' | 'json'

/** Bumped if the JSON envelope shape changes; lets a future importer branch safely. */
export const CONVERSATION_EXPORT_SCHEMA_VERSION = 1

/**
 * Localized section labels for the Markdown document. Only the structural headings are localized;
 * the transcript content itself is the user's own text and is never translated.
 */
export interface ConversationExportLabels {
  /** Top-level document title, e.g. "PathKeep conversation". */
  title: string
  /** "Model" field label in the header. */
  model: string
  /** "Exported" field label in the header. */
  exported: string
  /** Used when no model/provider label is known. */
  modelUnknown: string
  /** Byline for a user turn. */
  user: string
  /** Byline for an assistant turn. */
  assistant: string
  /** Heading above the reasoning quote. */
  reasoning: string
  /** Heading above the tool-call list. */
  tools: string
  /** Heading above the citations list. */
  citations: string
  /** Per-turn token usage line, with `{prompt}` / `{completion}` placeholders. */
  usage: string
  /** Shown in place of an answer when an assistant turn produced no text. */
  noAnswer: string
  /** Suffix appended to a turn whose status is `error`. */
  errorSuffix: string
  /** Suffix appended to a turn whose status is `cancelled`. */
  cancelledSuffix: string
}

/** Inputs the serializers need beyond the transcript itself. */
export interface ConversationExportContext {
  /** Active provider label (name / model), or null when none is configured. */
  modelLabel?: string | null
  /** The export instant (injected for deterministic tests). */
  exportedAt?: Date
  /** Localized Markdown section labels. */
  labels: ConversationExportLabels
}

/**
 * Fill the `{prompt}` / `{completion}` placeholders in the usage template. Both placeholders are
 * always present in the shipped template, so a plain replace is exhaustive (no fallback branch).
 */
function fillUsage(
  template: string,
  promptTokens: number,
  completionTokens: number,
): string {
  return template
    .replace('{prompt}', String(promptTokens))
    .replace('{completion}', String(completionTokens))
}

/**
 * Format a Date as a stable `YYYY-MM-DD-HHmmss` token for filenames (local time). Kept separate
 * from any locale formatting so two exports in the same session never collide and the name is the
 * same regardless of UI language.
 */
function timestampToken(now: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0')
  return (
    `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}` +
    `-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`
  )
}

/**
 * A sensible default filename for the save dialog, e.g. `pathkeep-conversation-2026-06-24-143012.md`.
 * The extension matches the chosen format so the picker filters correctly.
 */
export function defaultConversationExportName(
  format: ConversationExportFormat,
  now: Date = new Date(),
): string {
  const extension = format === 'json' ? 'json' : 'md'
  return `pathkeep-conversation-${timestampToken(now)}.${extension}`
}

/** Render one tool call as a readable Markdown list item (and nested code/host-call detail). */
function toolCallToMarkdown(
  call: NonNullable<ChatMessage['toolCalls']>[number],
): string[] {
  const lines: string[] = []
  // `run_code` steps lead with the verbatim source; search tools lead with name + args.
  const isCode = typeof call.codeSource === 'string'
  const header = isCode ? `\`run_code\`` : `\`${call.name}\``
  lines.push(`- **${header}**`)

  if (!isCode && call.arguments.trim().length > 0) {
    lines.push(`  - args: \`${call.arguments.trim()}\``)
  }

  if (isCode && call.codeSource && call.codeSource.trim().length > 0) {
    lines.push('  - source:')
    lines.push('')
    lines.push('    ```')
    for (const sourceLine of call.codeSource.replace(/\n+$/, '').split('\n')) {
      lines.push(`    ${sourceLine}`)
    }
    lines.push('    ```')
  }

  if (call.hostCalls && call.hostCalls.length > 0) {
    for (const hostCall of call.hostCalls) {
      const detail =
        hostCall.query !== undefined
          ? `“${hostCall.query}” (${hostCall.plane ?? '?'}, ${hostCall.rowCount} rows)`
          : `${hostCall.function} (${hostCall.rowCount} rows)`
      lines.push(`  - call: ${detail}`)
    }
  }

  if (call.limitsHit) {
    lines.push(`  - limit: \`${call.limitsHit}\``)
  }

  const result = call.result?.trim()
  if (result && result.length > 0) {
    // A short, single-line preview keeps the doc readable; the JSON export carries the full text.
    const oneLine = result.replace(/\s+/g, ' ')
    const preview = oneLine.length > 280 ? `${oneLine.slice(0, 280)}…` : oneLine
    lines.push(`  - result${call.isError ? ' (error)' : ''}: ${preview}`)
  }

  return lines
}

/** Render the citations list for a turn as Markdown list items. */
function citationsToMarkdown(
  citations: NonNullable<ChatMessage['citations']>,
): string[] {
  return citations.map((citation) => {
    const label = citation.title?.trim() || citation.url
    const date = citation.visitedAt.slice(0, 10)
    return `- [${label}](${citation.url}) — ${date}`
  })
}

/**
 * Build a clean, human-readable Markdown transcript of the conversation. Empty transcripts still
 * produce a valid (header-only) document, so the caller never has to special-case length here (the
 * UI gates the affordance on `messages.length > 0` separately).
 */
export function buildConversationMarkdown(
  messages: readonly ChatMessage[],
  { modelLabel, exportedAt = new Date(), labels }: ConversationExportContext,
): string {
  const lines: string[] = []
  lines.push(`# ${labels.title}`)
  lines.push('')
  lines.push(`- **${labels.model}:** ${modelLabel || labels.modelUnknown}`)
  lines.push(`- **${labels.exported}:** ${exportedAt.toISOString()}`)
  lines.push('')

  for (const message of messages) {
    lines.push('---')
    lines.push('')

    if (message.role === 'user') {
      lines.push(`## ${labels.user}`)
      lines.push('')
      lines.push(message.content.trim() || labels.noAnswer)
      lines.push('')
      continue
    }

    // Assistant turn: byline (+ terminal-status suffix), reasoning, tools, answer, usage, citations.
    let byline = `## ${labels.assistant}`
    if (message.status === 'error') byline += ` ${labels.errorSuffix}`
    else if (message.status === 'cancelled')
      byline += ` ${labels.cancelledSuffix}`
    lines.push(byline)
    lines.push('')

    const reasoning = message.reasoning?.trim()
    if (reasoning) {
      lines.push(`### ${labels.reasoning}`)
      lines.push('')
      for (const reasoningLine of reasoning.split('\n')) {
        lines.push(`> ${reasoningLine}`)
      }
      lines.push('')
    }

    if (message.toolCalls && message.toolCalls.length > 0) {
      lines.push(`### ${labels.tools}`)
      lines.push('')
      for (const call of message.toolCalls) {
        lines.push(...toolCallToMarkdown(call))
      }
      lines.push('')
    }

    lines.push(message.content.trim() || labels.noAnswer)
    lines.push('')

    if (message.usage) {
      lines.push(
        fillUsage(
          labels.usage,
          message.usage.promptTokens,
          message.usage.completionTokens,
        ),
      )
      lines.push('')
    }

    if (message.citations && message.citations.length > 0) {
      lines.push(`### ${labels.citations}`)
      lines.push('')
      lines.push(...citationsToMarkdown(message.citations))
      lines.push('')
    }
  }

  // Collapse the trailing blank line to a single newline-terminated document.
  return `${lines.join('\n').replace(/\n+$/, '')}\n`
}

/** The structured envelope written by `buildConversationJson` (and recovered by a parse). */
export interface ConversationExportDocument {
  schemaVersion: number
  exportedAt: string
  modelLabel: string | null
  messages: readonly ChatMessage[]
}

/**
 * Build the lossless, pretty-printed JSON document. The full `ChatMessage` objects are embedded
 * verbatim (toolCalls + results + host calls + citations + usage + status), so a parse round-trips
 * the transcript exactly.
 */
export function buildConversationJson(
  messages: readonly ChatMessage[],
  { modelLabel, exportedAt = new Date() }: ConversationExportContext,
): string {
  const doc: ConversationExportDocument = {
    schemaVersion: CONVERSATION_EXPORT_SCHEMA_VERSION,
    exportedAt: exportedAt.toISOString(),
    modelLabel: modelLabel ?? null,
    messages,
  }
  return `${JSON.stringify(doc, null, 2)}\n`
}
