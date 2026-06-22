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
    // The full-archive scope fact now lives in the composer footer (`scopeNote`) so it stays
    // ambient through the WHOLE conversation, not only on this empty greeting (ASSIST-3). The
    // greeting subtitle stays focused; the persistent footer carries scope honesty.
    greetingSubtitle: t('chatGreetingSubtitle'),
    turn: {
      assistantByline,
      userByline: t('chatUserByline'),
      typingLabel: t('chatTyping'),
      evidenceLabel: t('chatEvidenceLabel'),
      errorGeneric: t('chatErrorGeneric'),
      stoppedLabel: t('chatStopped'),
      retryLabel: t('chatTryAgain'),
      copyLabel: t('chatCopyAnswer'),
      copiedLabel: t('chatCopiedAnswer'),
      regenerateLabel: t('chatRegenerateAnswer'),
      noAnswerLabel: t('chatNoAnswer'),
      statusUsingTool: t('chatStatusUsingTool'),
      statusAnswering: t('chatStatusAnswering'),
      statusComplete: t('chatStatusComplete'),
      usageLabel: t('chatUsageLabel'),
      evidenceStar: {
        starLabel: t('chatEvidenceStar'),
        unstarLabel: t('chatEvidenceUnstar'),
        status: {
          starred: t('chatEvidenceStarred'),
          unstarred: t('chatEvidenceUnstarred'),
        },
      },
      reasoning: {
        thinkingLabel: t('chatReasoningThinking'),
        thoughtLabel: t('chatReasoningThought'),
        toggleLabel: t('chatReasoningToggle'),
      },
      toolCalls: {
        label: t('chatToolsLabel'),
        ranTemplate: t('chatToolRan'),
        runningLabel: t('chatToolRunning'),
        doneLabel: t('chatToolDone'),
        failedLabel: t('chatToolFailed'),
        resultToggleLabel: t('chatToolResultToggle'),
        // W-AI-8 WU-5 code-mode observability copy. These ride the same tool-call copy bundle so the
        // tool-call block can render a code run's verbatim source + host-call timeline + limit chip
        // without a separate copy plumbing. The host-call row templates are composed from the
        // STRUCTURED HostCallRecord fields (never the non-localized argsSummary) so they translate.
        code: {
          ranLabel: t('chatCodeRanLabel'),
          sourceLabel: t('chatCodeSourceLabel'),
          sourceToggleLabel: t('chatCodeSourceToggle'),
          hostCallsLabel: t('chatCodeHostCallsLabel'),
          queryRowTemplate: t('chatCodeHostCallQuery'),
          fetchRowTemplate: t('chatCodeHostCallFetch'),
          genericRowTemplate: t('chatCodeHostCallGeneric'),
          limitLabel: t('chatCodeLimitLabel'),
          limits: {
            time: t('chatCodeLimitTime'),
            memory: t('chatCodeLimitMemory'),
            'host-calls': t('chatCodeLimitHostCalls'),
            output: t('chatCodeLimitOutput'),
            cancelled: t('chatCodeLimitCancelled'),
          },
        },
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
      // Short, ambient scope micro-line for the footer (the full sentence lived in the empty-state
      // greeting before; scope must persist, so a concise mono note rides the always-visible
      // attribution row instead).
      scopeNote: t('chatScopeNote'),
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
