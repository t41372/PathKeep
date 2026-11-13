/**
 * Agent conversations: sources, how folders become projects, the reader,
 * usage and cost, AI access (three separate opt-ins, all off by default),
 * and what "continue" does. Writing back into the tools' folders is not
 * offered and the page says so.
 */
import { FolderPlus, Plus, X } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { useProto } from '~/proto-state'
import { Switch } from '@/components/ui/switch'
import { HarnessBadge } from '~/components/bits'
import { archive } from '~/data/archive'
import { num } from '~/data/format'
import { harnessSources } from '../backup/sources-data'
import { Group, Mono, Note } from './parts'

function Toggle({ initial, disabled, label }: { initial: boolean; disabled?: boolean; label: string }) {
  const [on, setOn] = useState(initial)
  return <Switch checked={on} onCheckedChange={setOn} disabled={disabled} aria-label={label} />
}

const scratchRules = ['/tmp', '/private/var/folders', '~（家目錄本身）']

export function AgentSection() {
  const { estimateCost, set } = useProto()
  const [rules, setRules] = useState(scratchRules)
  const suggestions = archive.suggestions.length
  return (
    <SettingsSection title="Agent 對話">
      <Group title="來源" intro="PathKeep 只讀這些資料夾，不改、不刪任何檔案。工具沒裝的會自動略過。">
        {harnessSources.map((h) => (
          <SettingRow
            key={h.id}
            title={<HarnessBadge id={h.id} withName className="text-sm" />}
            description={
              <>
                <span className="font-mono text-[12px]">{h.root}</span>
                {h.alsoReads && (
                  <>
                    {' '}和 <span className="font-mono text-[12px]">{h.alsoReads}</span>
                  </>
                )}
                <br />
                找到 {num(h.chats)} 段對話{h.subagents > 0 && `，另有 ${num(h.subagents)} 個 subagent`}
              </>
            }
            control={<Toggle initial label={`備份 ${h.name}`} />}
          />
        ))}
        <SettingRow
          title="加入其他資料夾"
          description="例如用 CLAUDE_CONFIG_DIR 把 Claude Code 的資料放在別處，或從另一台電腦拷過來的資料夾。"
          control={
            <Button variant="outline" size="sm">
              <FolderPlus />
              選擇資料夾…
            </Button>
          }
        />
      </Group>

      <Group
        title="專案歸類"
        intro="Agent 把每個資料夾都當成一個專案。PathKeep 會把同一個 repo 的主資料夾、worktree、子資料夾和搬家前的位置併成一個專案，只改 PathKeep 這邊的顯示。"
      >
        <SettingRow
          title="讀取專案資料夾的 git 資訊"
          description={
            <>
              備份時讀每個專案資料夾的 <Mono>.git</Mono>：repo 的根目錄在哪、worktree 屬於哪個 repo、遠端網址。資料夾刪掉之後就查不到了，所以要在備份當下記下來。只讀這三項，不讀程式碼，也不改任何檔案。
            </>
          }
          control={<Toggle initial label="讀取 git 資訊" />}
        />
        <SettingRow
          title="暫存資料夾歸到「零散資料夾」"
          description="在這些位置裡的對話不各自算一個專案，全部放進「零散資料夾」。"
          control={
            <Button variant="outline" size="sm" onClick={() => setRules((r) => [...r, '~/Downloads'])}>
              <Plus />
              加入位置
            </Button>
          }
        >
          <div className="flex flex-wrap gap-1.5">
            {rules.map((r) => (
              <span key={r} className="flex items-center gap-1 rounded-md border bg-background/60 py-0.5 pr-1 pl-2 font-mono text-[12px]">
                {r}
                <button type="button" aria-label={`移除 ${r}`} onClick={() => setRules((all) => all.filter((x) => x !== r))} className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground">
                  <X className="size-3" />
                </button>
              </span>
            ))}
          </div>
        </SettingRow>
        <SettingRow
          title="可能屬於同一個專案的資料夾"
          description={`有 ${suggestions} 個資料夾看起來屬於別的專案，例如搬過家的資料夾、刪掉時還沒讀到 .git 的 worktree。要你確認才會加進去。`}
          control={
            <Button variant="outline" size="sm" asChild>
              <Link to="/agent/projects?review=1">去看看</Link>
            </Button>
          }
        />
      </Group>

      <Group title="閱讀器">
        <SettingRow title="預設收起工具呼叫" description="每一輪的工具呼叫收成一行，例如「讀了 3 個檔案、跑了 2 個指令」，點開才看細節。" control={<Toggle initial label="預設收起工具呼叫" />} />
        <SettingRow title="顯示思考過程" description="有紀錄 thinking 的對話，在回覆上方顯示一行「思考了 6 秒」，點開才看全文。" control={<Toggle initial label="顯示思考過程" />} />
      </Group>

      <Group title="用量與費用" intro="Token 用量照工具自己記的數字算；沒有紀錄的對話會標出來，不當成 0。">
        <SettingRow
          title="顯示工具自己記錄的費用"
          description="Claude Code 與 Pi 會在紀錄裡寫下它們自己算的費用，PathKeep 照抄，不重算。"
          control={<Toggle initial label="顯示工具記錄的費用" />}
        />
        <SettingRow
          title="按 API 牌價估算其他對話的費用"
          description="沒有記費用的對話，按 PathKeep 附的牌價表（2026-11 版，跟著 app 更新）換算成等值金額。訂閱方案不按 token 收費，這只是看用量的分量。本機模型沒有牌價，不估。"
          control={<Switch checked={estimateCost} onCheckedChange={(v) => set({ estimateCost: v })} aria-label="按牌價估算費用" />}
        />
      </Group>

      <Group title="AI 存取" intro="對話裡常有程式碼、金鑰和私人路徑。下面三項分開開，預設都關。">
        <SettingRow
          title="助理可以讀 Agent 對話"
          description="助理回答時可以引用你的 Agent 對話。用的是你在「AI」裡設定的模型；如果那是遠端服務，引用到的段落會送過去。"
          control={<Toggle initial={false} label="助理可以讀 Agent 對話" />}
        />
        <SettingRow
          title="建立語意搜尋索引"
          description="把對話內容轉成向量，讓搜尋也能找到意思相近、字面不同的段落。向量存在本機存檔裡；用遠端 embedding 服務時，內容會送過去。"
          control={<Toggle initial={false} label="建立語意搜尋索引" />}
        />
        <SettingRow
          title="MCP 可以讀 Agent 對話"
          description="接上 PathKeep MCP 的工具（包括 Agent 自己）可以搜尋和讀取這些對話。存檔鎖定時一律讀不到。"
          control={<Toggle initial={false} label="MCP 可以讀 Agent 對話" />}
        />
      </Group>

      <Group title="繼續對話">
        <SettingRow
          title="「繼續」會複製一行指令"
          description={
            <>
              閱讀器裡按「繼續」，會複製像 <Mono>cd ~/code/orbit-notes &amp;&amp; claude --resume 1f3e…</Mono> 這樣的指令，貼到終端機就能接著做。PathKeep 不會替你開終端機或啟動工具。
            </>
          }
        />
        <Note>
          還沒提供：把已刪除的對話放回工具的資料夾、把 worktree 裡的對話搬回主專案。這兩件事都要寫進工具自己的資料夾，會先在隔離環境裡試過，確定不會弄壞工具的資料再決定要不要做。
        </Note>
      </Group>
    </SettingsSection>
  )
}
