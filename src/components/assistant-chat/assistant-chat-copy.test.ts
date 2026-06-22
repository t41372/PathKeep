/**
 * @file assistant-chat-copy.test.ts
 * @description Coverage for the copy/prompt builders, run against the live `assistant` catalog
 *              so renames surface here, and asserting the provider-vs-fallback byline branches.
 */

import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '../../lib/i18n'
import {
  buildAssistantChatCopy,
  buildAssistantChatPrompts,
} from './assistant-chat-copy'

const t = createNamespaceTranslator('en', 'assistant')

describe('buildAssistantChatCopy', () => {
  test('uses the provider label in the attribution and assistant byline', () => {
    const copy = buildAssistantChatCopy(t, { providerLabel: 'Local LLM / g4' })
    expect(copy.composer.attribution).toBe('Local · Local LLM / g4')
    expect(copy.turn.assistantByline).toBe('Local · Local LLM / g4')
    expect(copy.greetingTitle).toBe(t('chatGreetingTitle'))
    expect(copy.composer.sendLabel).toBe(t('chatComposerSend'))
    expect(copy.composer.cancelLabel).toBe(t('chatComposerCancel'))
    expect(copy.turn.typingLabel).toBe(t('chatTyping'))
    expect(copy.turn.reasoning.thinkingLabel).toBe(t('chatReasoningThinking'))
    expect(copy.turn.toolCalls.label).toBe(t('chatToolsLabel'))
    expect(copy.turn.evidenceLabel).toBe(t('chatEvidenceLabel'))
    // New W-AI-2 review keys are wired through the turn + composer copy.
    expect(copy.turn.errorGeneric).toBe(t('chatErrorGeneric'))
    expect(copy.turn.stoppedLabel).toBe(t('chatStopped'))
    expect(copy.turn.retryLabel).toBe(t('chatTryAgain'))
    expect(copy.turn.noAnswerLabel).toBe(t('chatNoAnswer'))
    expect(copy.turn.statusUsingTool).toBe(t('chatStatusUsingTool'))
    expect(copy.turn.statusAnswering).toBe(t('chatStatusAnswering'))
    expect(copy.turn.statusComplete).toBe(t('chatStatusComplete'))
    expect(copy.composer.connectingLabel).toBe(
      t('chatConnecting', { provider: 'Local LLM / g4' }),
    )
    // ASSIST-2 per-message action copy is wired through the turn copy.
    expect(copy.turn.copyLabel).toBe(t('chatCopyAnswer'))
    expect(copy.turn.copiedLabel).toBe(t('chatCopiedAnswer'))
    expect(copy.turn.regenerateLabel).toBe(t('chatRegenerateAnswer'))
    // ASSIST-3 / C1-3 honesty: the whole-archive scope fact now rides the PERSISTENT composer
    // footer (so it survives the whole conversation, not just the empty greeting), and the greeting
    // subtitle is just the focused greeting — no longer carries the scope sentence.
    expect(copy.composer.scopeNote).toBe(t('chatScopeNote'))
    expect(copy.greetingSubtitle).toBe(t('chatGreetingSubtitle'))
  })

  test('falls back to the keyword-only string when no provider is set', () => {
    const copy = buildAssistantChatCopy(t)
    expect(copy.composer.attribution).toBe(t('chatAttributionFallback'))
    expect(copy.turn.assistantByline).toBe(t('chatAttributionFallback'))
    // The connecting affordance falls back to the keyword-only label as the provider name.
    expect(copy.composer.connectingLabel).toBe(
      t('chatConnecting', { provider: t('chatAttributionFallback') }),
    )
  })

  test('treats a null provider label as no provider', () => {
    const copy = buildAssistantChatCopy(t, { providerLabel: null })
    expect(copy.composer.attribution).toBe(t('chatAttributionFallback'))
  })
})

describe('buildAssistantChatPrompts', () => {
  test('builds the three suggested-prompt cards from the catalog', () => {
    const prompts = buildAssistantChatPrompts(t)
    expect(prompts.map((prompt) => prompt.id)).toEqual([
      'chat-prompt-1',
      'chat-prompt-2',
      'chat-prompt-3',
    ])
    expect(prompts[0].text).toBe(t('chatPrompt1'))
    expect(prompts[2].text).toBe(t('chatPrompt3'))
  })
})
