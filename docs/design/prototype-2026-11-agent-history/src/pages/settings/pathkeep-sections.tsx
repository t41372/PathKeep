/**
 * The settings entry of a data type that is only planned. The shared
 * sections (一般, 安全, 儲存, 關於) each have their own file beside this one.
 */
import { SettingsSection } from '@/components/app/setting-row'
import type { Kind } from '~/kinds'
import { Note } from './parts'

/** A planned type that the six-type preset puts in the nav. */
export function PlannedKindSection({ kind }: { kind: Kind }) {
  return (
    <SettingsSection title={kind.label}>
      <Note>
        這個類型還在規劃，沒有設定。之後會讀：{kind.sources.join('、')}。原型裡放它，只是為了看設定的左欄在類型變多時排不排得下。
      </Note>
    </SettingsSection>
  )
}
