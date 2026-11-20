/**
 * What a screen says when the backend refuses AI work on Agent
 * conversations, by its typed code: a title, one sentence of what to do,
 * the settings pages where it is done, and whether trying again can help.
 * The opt-in named for `agent-conversations-access-disabled` depends on
 * what was refused: the semantic index (Records, Settings) or the
 * assistant (Ask).
 *
 * Responsible for: those words and places. Not responsible for: reading
 * the code (`lib/backend-client/ai-refusals.ts`) or drawing.
 */
import type { AiRefusalCode } from '@/lib/backend-client/ai-refusals'
import type { MessageKey, Translator } from '@/lib/i18n'

export type RefusalPlace = 'ai' | 'agent' | 'kinds'

export const refusalPlacePath: Record<RefusalPlace, string> = {
  ai: '/settings/ai',
  agent: '/settings/agent',
  kinds: '/settings/kinds',
}

export const refusalPlaceName: Record<RefusalPlace, MessageKey> = {
  ai: 'settings.nav.ai',
  agent: 'settings.nav.agent',
  kinds: 'settings.nav.kinds',
}

export interface RefusalWords {
  title: string
  body: string
  places: RefusalPlace[]
  /** The cause can pass by itself (a service that was down, a model that changed). */
  retry: boolean
}

type Kind =
  | 'aiDisabled'
  | 'semanticDisabled'
  | 'assistantDisabled'
  | 'semanticAccess'
  | 'assistantAccess'
  | 'kindOff'
  | 'providerMissing'
  | 'providerChanged'
  | 'serviceUnavailable'
  | 'modelUnavailable'

const keys = {
  aiDisabled: ['aiRefusals.aiDisabled.title', 'aiRefusals.aiDisabled.body'],
  semanticDisabled: [
    'aiRefusals.semanticDisabled.title',
    'aiRefusals.semanticDisabled.body',
  ],
  assistantDisabled: [
    'aiRefusals.assistantDisabled.title',
    'aiRefusals.assistantDisabled.body',
  ],
  semanticAccess: [
    'aiRefusals.semanticAccess.title',
    'aiRefusals.semanticAccess.body',
  ],
  assistantAccess: [
    'aiRefusals.assistantAccess.title',
    'aiRefusals.assistantAccess.body',
  ],
  kindOff: ['aiRefusals.kindOff.title', 'aiRefusals.kindOff.body'],
  providerMissing: [
    'aiRefusals.providerMissing.title',
    'aiRefusals.providerMissing.body',
  ],
  providerChanged: [
    'aiRefusals.providerChanged.title',
    'aiRefusals.providerChanged.body',
  ],
  serviceUnavailable: [
    'aiRefusals.serviceUnavailable.title',
    'aiRefusals.serviceUnavailable.body',
  ],
  modelUnavailable: [
    'aiRefusals.modelUnavailable.title',
    'aiRefusals.modelUnavailable.body',
  ],
} satisfies Record<Kind, [MessageKey, MessageKey]>

export function refusalWords(
  t: Translator,
  code: AiRefusalCode,
  refused: 'semantic' | 'assistant',
): RefusalWords {
  const words = (
    kind: Kind,
    places: RefusalPlace[],
    retry = false,
    params?: Record<string, string>,
  ): RefusalWords => ({
    title: t(keys[kind][0]),
    body: t(keys[kind][1], params),
    places,
    retry,
  })
  switch (code) {
    case 'ai-disabled':
      return words('aiDisabled', ['ai'])
    case 'semantic-index-disabled':
      return words('semanticDisabled', ['ai'], false, {
        semantic: t('settingsAi.semantic.title'),
      })
    case 'assistant-disabled':
      return words('assistantDisabled', ['ai'])
    case 'agent-conversations-access-disabled':
      return refused === 'semantic'
        ? words('semanticAccess', ['agent'], false, {
            switch: t('settingsAgent.ai.semantic'),
          })
        : words('assistantAccess', ['agent'], false, {
            switch: t('settingsAgent.ai.assistant'),
          })
    case 'kind-off':
      return words('kindOff', ['kinds'])
    case 'embedding-provider-unavailable':
      return words('providerMissing', ['ai'])
    case 'embedding-provider-changed':
      return words('providerChanged', ['agent'], true)
    case 'embedding-service-unavailable':
      return words('serviceUnavailable', ['ai'], true)
    case 'embedding-model-unavailable':
      return words('modelUnavailable', ['ai'], true)
  }
}
