/**
 * @file ai-providers-section.tsx
 * @description Renders the optional AI provider management surface from route-owned Settings state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 AI master toggle、provider editor lists、index health 與 integration review artifacts。
 * - 把 provider draft、API key、save/reset、copy/open-path 行為交回 route-owned handlers。
 * - 保持 optional intelligence honesty，不把 external integration preview 誤包裝成自動執行。
 *
 * ## 不負責
 * - 不保存 AI config 或 API key。
 * - 不管理 semantic index queue。
 * - 不決定 MCP / skill / embedding backend contract。
 *
 * ## 依賴關係
 * - 依賴 `AiProviderEditorList` 與 review components 呈現 provider 與 integration review surface。
 * - 依賴 route hook 提供 current draft、integration preview 與 mutation handlers。
 *
 * ## 性能備注
 * - provider editor 只編輯已載入的 draft；generated files 與 integration preview 來自既有 backend preview，不在 section 內額外 fan-out。
 */

import type { ReviewCopyFeedback } from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { useI18n } from '../../lib/i18n'
import type { aiStatusMeta } from '../../lib/intelligence-ai-presentation'
import type {
  AiIndexStatus,
  AiIntegrationPreview,
  AiProviderConfig,
  AiRequestFormat,
  AiSettings,
} from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'

type AiProviderTranslations = {
  providerName: string
  providerId: string
  requestFormat: string
  baseUrl: string
  baseUrlPlaceholder: string
  defaultModel: string
  modelCatalog: string
  modelCatalogHint: string
  enabled: string
  temperature: string
  maxTokens: string
  dimensions: string
  notes: string
  apiKey: string
  apiKeyPlaceholder: string
  keySaved: string
  keyNotSaved: string
  saveKey: string
  clearKey: string
  remove: string
  requestFormatLabels: Record<AiRequestFormat, string>
}

/**
 * Defines the route-owned AI settings state consumed by this section.
 */
export interface AiProvidersSectionState {
  aiApiKeys: Record<string, string>
  aiStatus: AiIndexStatus | null
  configDirty: boolean
  copyFeedback: ReviewCopyFeedback | null
  currentSettings: AiSettings | null
  indexMeta: ReturnType<typeof aiStatusMeta> | null
  integrationError: string | null
  integrationPreview: AiIntegrationPreview | null
  noProviders: boolean
  persistedProviderIds: Set<string>
  providerTranslations: AiProviderTranslations
  saving: boolean
  onAddProvider: (purpose: 'llm' | 'embedding') => void
  onApiKeyChange: (providerId: string, value: string) => void
  onClearAiApiKey: (providerId: string) => Promise<void>
  onCopyIntegrationValue: (key: string, value: string) => Promise<void>
  onOpenPath: (path: string) => void
  onRemoveProvider: (purpose: 'llm' | 'embedding', providerId: string) => void
  onResetAiConfig: () => void
  onSaveAiApiKey: (providerId: string) => Promise<void>
  onSaveAiConfig: () => Promise<void>
  onSelectProvider: (purpose: 'llm' | 'embedding', providerId: string) => void
  onToggleAi: () => void
  onUpdateProvider: (
    purpose: 'llm' | 'embedding',
    providerId: string,
    patch: Partial<AiProviderConfig>,
  ) => void
}

/**
 * Groups the stable section anchor descriptor with the AI settings view-model.
 */
export interface AiProvidersSectionProps {
  navItem: SettingsSectionNavItem
  state: AiProvidersSectionState
}

/**
 * Renders the optional AI provider management surface from route-owned state.
 *
 * The section exits early when there is no current AI draft, so it never
 * invents placeholder providers outside the real Settings route owner.
 */
export function AiProvidersSection({
  navItem,
  state,
}: AiProvidersSectionProps) {
  const { t } = useI18n()
  const { currentSettings } = state

  if (!currentSettings) {
    return null
  }

  return (
    <div className="panel panel--optional" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
        <span className="panel-badge">{t('settings.aiDeferredBadge')}</span>
      </div>
      <div className="panel-body">
        <StatusCallout
          tone="info"
          title={t('settings.aiDeferredTitle')}
          body={t('settings.aiDeferredBody')}
        />

        <div className="settings-field-grid" aria-disabled="true">
          {[
            t('settings.aiMasterToggle'),
            t('settings.aiLlmProviders'),
            t('settings.aiEmbeddingProviders'),
            t('settings.aiIntegrationArtifactsTitle'),
          ].map((label) => (
            <button
              className="btn-secondary"
              disabled
              key={label}
              title={t('settings.aiDeferredTooltip')}
              type="button"
            >
              {label}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
