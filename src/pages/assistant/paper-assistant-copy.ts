/**
 * Translator → PaperAssistantView copy bundle.
 *
 * The Assistant route hands the assistant-namespace translator into this
 * builder, which projects the flat `paper*` keys into the nested copy
 * shape PaperAssistantView expects. Keeping the projection here (instead
 * of in the route) means catalog renames surface in one diff and the
 * builder can be tested against the live catalog.
 *
 * ## Responsibilities
 * - Read every `paper*` assistant key once.
 * - Build the composer attribution from the active provider label,
 *   falling back to the lexical-only string when no provider is set.
 *
 * ## Not responsible for
 * - Constructing greeting prompts — the route picks which prompt ids to
 *   surface (e.g. from `suggestedQuestions`).
 */

import type {
  PaperAssistantGreetingPrompt,
  PaperAssistantViewCopy,
} from '@/components/explorer-paper'

export type AssistantTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

export interface BuildPaperAssistantCopyOptions {
  /** Display label for the active LLM provider (name / model). Undefined →
   *  composer attribution falls back to the "keyword only" string. */
  providerLabel?: string | null
}

export function buildPaperAssistantViewCopy(
  t: AssistantTranslator,
  { providerLabel }: BuildPaperAssistantCopyOptions = {},
): PaperAssistantViewCopy {
  const attribution = providerLabel
    ? t('paperComposerAttribution', { provider: providerLabel })
    : t('paperComposerAttributionFallback')
  return {
    greetingTitle: t('paperGreetingTitle'),
    greetingSubtitle: t('paperGreetingSubtitle'),
    composer: {
      placeholder: t('paperComposerPlaceholder'),
      sendLabel: t('paperComposerSendLabel'),
      attribution,
      keyHint: t('paperComposerKeyHint'),
    },
    evidenceLabel: t('paperEvidenceLabel'),
  }
}

/**
 * Build the three suggested-prompt cards rendered by PaperAssistantGreeting.
 * Each prompt's `text` doubles as the value seeded into the composer when
 * the card is picked.
 */
export function buildPaperAssistantPrompts(
  t: AssistantTranslator,
): PaperAssistantGreetingPrompt[] {
  return [
    { id: 'prompt-1', text: t('paperPrompt1') },
    { id: 'prompt-2', text: t('paperPrompt2') },
    { id: 'prompt-3', text: t('paperPrompt3') },
  ]
}
