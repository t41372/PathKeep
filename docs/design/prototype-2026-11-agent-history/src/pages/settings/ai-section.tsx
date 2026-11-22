/**
 * Settings › AI: today's section (src/features/settings/ai-section.tsx) with
 * fake data, plus at the end the proposed group "各類型的資料能不能給 AI 讀":
 * each type's own settings page holds its AI access switches, and this page
 * points to them. The rows live in ./ai/.
 */
import { Link } from 'react-router-dom'
import { SettingsSection } from '@/components/app/setting-row'
import { kinds } from '~/kinds'
import { useProto } from '~/proto-state'
import { McpLogRow, McpRow } from './ai/mcp'
import { ProviderRow } from './ai/provider-row'
import { AiProtoSwitches } from './ai/proto-switches'
import { RebuildRow } from './ai/rebuild'
import { SearchTuningRow } from './ai/search-tuning-row'
import { SemanticRow } from './ai/semantic-row'
import { AiProvider } from './ai/state'
import { Group } from './parts'

export function AiSection() {
  const { enabledKinds } = useProto()
  return (
    <AiProvider>
      <SettingsSection title="AI">
        <p className="-mt-2 mb-1 text-[13px] text-muted-foreground">這裡的一切都是選用的。沒有這些，PathKeep 也能備份、搜尋和分析你的歷史。</p>
        <AiProtoSwitches />
        <SemanticRow />
        <ProviderRow />
        <McpRow />
        <McpLogRow />
        <RebuildRow />
        <SearchTuningRow />
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
    </AiProvider>
  )
}
