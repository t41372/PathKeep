/**
 * Settings › AI. The group "各類型的資料能不能給 AI 讀" is the proposed addition:
 * each type's own settings page holds its AI access switches, and this page
 * points to them.
 */
import { Link } from 'react-router-dom'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { kinds } from '~/kinds'
import { useProto } from '~/proto-state'
import { Group } from './parts'

export function AiSection() {
  const { enabledKinds } = useProto()
  return (
    <SettingsSection title="AI">
      <SettingRow title="對話模型" description="LM Studio · qwen3.8-27b（本機）" control={<Button variant="outline" size="sm">更改…</Button>} />
      <SettingRow title="Embedding 模型" description="內建的靜態模型（本機）" control={<Button variant="outline" size="sm">更改…</Button>} />
      <Group title="各類型的資料能不能給 AI 讀" intro="在各資料類型自己的設定裡分開開。">
        <div className="flex flex-col gap-1.5 text-[13px]">
          {kinds
            .filter((k) => k.real && enabledKinds.includes(k.id))
            .map((k) => (
              <Link key={k.id} to={`/settings/${k.id}`} className="flex w-fit items-center gap-1.5 text-brand hover:underline">
                <k.icon className="size-3.5" />
                {k.label}
                <span className="text-muted-foreground">{k.id === 'agent' ? '：預設都關' : '：助理與語意搜尋已開'}</span>
              </Link>
            ))}
        </div>
      </Group>
    </SettingsSection>
  )
}
