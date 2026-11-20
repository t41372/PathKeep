/**
 * Link previews' two options: when to fetch (as pages are shown, or after
 * each backup) and how long to keep the images. They are settings, so they
 * live under the Link previews switch in Settings › Browsing history
 * (`data-spaces.md` §8) and show only while that switch is on.
 *
 * Responsibilities: reading the saved values from the snapshot and saving a
 * change at once; a saved cleanup rule the menu doesn't offer stays shown as
 * "custom". Not responsible for: the switch, the coverage and "Clean up now"
 * (the background tasks page), or fetching.
 */
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { RowSelect } from '../row-select'
import { useSaveSetting } from '../use-save-setting'
import { keepChoiceOf, keepChoices, keepRules } from './link-preview-keep'

const FETCH_MODES = ['on_demand', 'background'] as const

export function LinkPreviewOptions() {
  const { t } = useI18n()
  const og = useSnapshot().config.ogImage
  const { save, saving } = useSaveSetting()
  if (!og) return null
  const keep = keepChoiceOf(og.cleanup)

  const setMode = (mode: string) =>
    void save((config) => {
      if (config.ogImage)
        config.ogImage.fetchMode = mode as (typeof FETCH_MODES)[number]
      return config
    })
  const setKeep = (choice: string) =>
    void save((config) => {
      if (config.ogImage && choice in keepRules)
        config.ogImage.cleanup = keepRules[choice as keyof typeof keepRules]
      return config
    })

  return (
    <>
      <div className="flex items-center justify-between gap-4">
        <label htmlFor="settings-previews-mode">
          {t('settingsBackground.previews.mode.label')}
        </label>
        <RowSelect
          id="settings-previews-mode"
          value={og.fetchMode}
          disabled={saving}
          onChange={setMode}
          options={FETCH_MODES.map((mode) => ({
            value: mode,
            label: t(`settingsBackground.previews.mode.${mode}`),
          }))}
        />
      </div>
      <div className="flex items-center justify-between gap-4">
        <label htmlFor="settings-previews-keep">
          {t('settingsBackground.previews.keep.label')}
        </label>
        <RowSelect
          id="settings-previews-keep"
          value={keep}
          disabled={saving}
          onChange={setKeep}
          options={[
            ...(keep === 'custom' ? (['custom'] as const) : []),
            ...keepChoices,
          ].map((choice) => ({
            value: choice,
            label: t(`settingsBackground.previews.keep.${choice}`),
          }))}
        />
      </div>
    </>
  )
}
