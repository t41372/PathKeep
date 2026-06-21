/**
 * @file assistant-chat-copy.ts
 * @description Projects the `assistant` i18n namespace into the copy bundles the chat view and
 *              its blocks consume.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Read every `chat*` assistant key once and build the nested copy shapes for the view,
 *   turns, reasoning panel, tool-use block, and composer.
 * - Resolve the provider-attribution byline from the active LLM provider label, falling back
 *   to the keyword-only string when none is configured. No model id is ever embedded in copy.
 *
 * ## Not responsible for
 * - Choosing which prompts to show beyond the three built-in cards.
 * - Owning the strings themselves — the catalog is the source of truth.
 */

import type { PaperAssistantGreetingPrompt } from '@/components/explorer-paper'
import type { AssistantChatViewCopy } from './assistant-chat-view'

export type AssistantTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

export interface BuildAssistantChatCopyOptions {
  /** Display label for the active LLM provider (name / model), or null when none is set. */
  providerLabel?: string | null
}

export function buildAssistantChatCopy(
  t: AssistantTranslator,
  { providerLabel }: BuildAssistantChatCopyOptions = {},
): AssistantChatViewCopy {
  const attribution = providerLabel
    ? t('chatAttribution', { provider: providerLabel })
    : t('chatAttributionFallback')
  const assistantByline = providerLabel
    ? t('chatAssistantByline', { provider: providerLabel })
    : t('chatAttributionFallback')

  return {
    greetingTitle: t('chatGreetingTitle'),
    greetingSubtitle: t('chatGreetingSubtitle'),
    turn: {
      assistantByline,
      userByline: t('chatUserByline'),
      typingLabel: t('chatTyping'),
      evidenceLabel: t('chatEvidenceLabel'),
      errorGeneric: t('chatErrorGeneric'),
      stoppedLabel: t('chatStopped'),
      retryLabel: t('chatTryAgain'),
      noAnswerLabel: t('chatNoAnswer'),
      statusUsingTool: t('chatStatusUsingTool'),
      statusAnswering: t('chatStatusAnswering'),
      statusComplete: t('chatStatusComplete'),
      reasoning: {
        thinkingLabel: t('chatReasoningThinking'),
        thoughtLabel: t('chatReasoningThought'),
        toggleLabel: t('chatReasoningToggle'),
      },
      toolCalls: {
        label: t('chatToolsLabel'),
        ranTemplate: t('chatToolRan'),
      },
    },
    composer: {
      placeholder: t('chatComposerPlaceholder'),
      sendLabel: t('chatComposerSend'),
      cancelLabel: t('chatComposerCancel'),
      attribution,
      keyHint: t('chatComposerKeyHint'),
      connectingLabel: providerLabel
        ? t('chatConnecting', { provider: providerLabel })
        : t('chatConnecting', { provider: t('chatAttributionFallback') }),
    },
  }
}

/** The three built-in suggested-prompt cards for the empty state. */
export function buildAssistantChatPrompts(
  t: AssistantTranslator,
): PaperAssistantGreetingPrompt[] {
  return [
    { id: 'chat-prompt-1', text: t('chatPrompt1') },
    { id: 'chat-prompt-2', text: t('chatPrompt2') },
    { id: 'chat-prompt-3', text: t('chatPrompt3') },
  ]
}
