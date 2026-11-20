/**
 * The colours of the Agent Insights charts (`data-spaces-tradeoff.md`
 * §4.3, §4.4). A chart whose one series is the page's main amount uses
 * the space's accent; a chart that splits one thing into parts (by tool,
 * by kind of token, by model) uses the fixed categorical slots, which
 * look the same in every space and stay apart for colour-blind eyes
 * (`src/lib/chart-palette.test.ts`).
 *
 * Responsibilities: which slot each tool, token kind and model rank takes.
 * Not responsible for: the colours themselves (`src/index.css`).
 */
import type { TokenKinds } from '@/lib/backend-client/agent-insights'
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import type { MessageKey } from '@/lib/i18n'

/** Each tool keeps its slot; a tool added later gets the next free one. */
export const TOOL_COLOR: Record<AgentToolId, string> = {
  'claude-code': 'var(--chart-2)',
  codex: 'var(--chart-1)',
  pi: 'var(--chart-3)',
  deepseek: 'var(--chart-5)',
}

export const OTHER_COLOR = 'var(--chart-other)'

/** The four kinds, coloured as the reader's info panel colours them. */
export const TOKEN_KINDS: {
  key: keyof TokenKinds
  label: MessageKey
  color: string
}[] = [
  { key: 'output', label: 'agentInfo.tokens.output', color: 'var(--chart-1)' },
  { key: 'input', label: 'agentInfo.tokens.input', color: 'var(--chart-2)' },
  {
    key: 'cacheWrite',
    label: 'agentInfo.tokens.cacheWrite',
    color: 'var(--chart-3)',
  },
  {
    key: 'cacheRead',
    label: 'agentInfo.tokens.cacheRead',
    color: 'var(--chart-5)',
  },
]

/** The five largest models in rank order; the rest share `OTHER_COLOR`. */
export const MODEL_COLORS = [
  'var(--chart-1)',
  'var(--chart-2)',
  'var(--chart-3)',
  'var(--chart-4)',
  'var(--chart-5)',
]

/** The space's accent for the main amount, and a quiet grey beside it. */
export const MAIN_COLOR = 'var(--brand)'
export const BESIDE_COLOR =
  'color-mix(in oklab, var(--muted-foreground) 45%, transparent)'
