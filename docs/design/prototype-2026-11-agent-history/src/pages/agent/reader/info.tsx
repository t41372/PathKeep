/**
 * Everything known about one conversation, grouped the way people ask:
 * how long, how much, what changed, where, with what, and where it is kept.
 * A field the harness didn't record says so instead of showing zero.
 */
import { ChevronDown, ExternalLink, FolderX } from 'lucide-react'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import type { ReactNode } from 'react'
import { Kv, TokenBar } from '~/components/bits'
import { harnesses, projectOf, sourcePath, subagentsOf } from '~/data/archive'
import { totalTokens } from '~/data/fixture'
import { big, bytes, dateTime, duration, num, pct, usd } from '~/data/format'
import type { Chat } from '~/data/types'

const entrypointLabel = { cli: '終端機（CLI）', vscode: 'VS Code 擴充', desktop: '桌面 App', sdk: 'SDK / 腳本', app: 'Codex App' } as const

function Section({ title, open, children }: { title: string; open?: boolean; children: ReactNode }) {
  return (
    <Collapsible defaultOpen={open} className="group/section border-b px-4 py-2.5 last:border-b-0">
      <CollapsibleTrigger className="flex w-full items-center justify-between py-0.5 text-[12px] font-semibold text-muted-foreground hover:text-foreground">
        {title}
        <ChevronDown className="size-3.5 transition-transform group-data-[state=open]/section:rotate-180" />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <dl className="pt-1">{children}</dl>
      </CollapsibleContent>
    </Collapsible>
  )
}

export function InfoPanel({ chat }: { chat: Chat }) {
  const project = projectOf(chat)
  const location = project.locations.find((l) => l.path === chat.cwd)
  const subs = subagentsOf(chat)
  const subTokens = subs.reduce((s, x) => s + totalTokens(x.tokens), 0)
  const h = harnesses[chat.harness]
  return (
    <div className="flex flex-col">
      <Section title="時間" open>
        <Kv k="Agent 工作">
          {duration(chat.agentMs)}
          <span className="block text-[12px] text-muted-foreground">
            {chat.agentMsSource === 'recorded' ? `${h.name} 自己記錄的每輪時間加總` : '從訊息時間推算（這個版本沒記每輪時間）'}
          </span>
        </Kv>
        <Kv k="開始">{dateTime(chat.started)}</Kv>
        <Kv k="最後活動">{dateTime(chat.ended)}</Kv>
        <Kv k="來回">
          {num(chat.turns)} 輪 · 工具 {num(chat.toolCalls)} 次
        </Kv>
      </Section>
      <Section title="Token" open>
        {chat.tokens ? (
          <>
            <div className="mb-2 text-[20px] font-semibold tracking-tight tabular">{big(totalTokens(chat.tokens))}</div>
            <TokenBar tokens={chat.tokens} />
            {chat.tokens.reasoning > 0 && <p className="mt-2 text-[12px] text-muted-foreground">輸出裡有 {big(chat.tokens.reasoning)} 是推理用的 token。</p>}
            {subs.length > 0 && <p className="mt-1 text-[12px] text-muted-foreground">其中 subagent 用了 {big(subTokens)}（{pct(subTokens / totalTokens(chat.tokens))}）。</p>}
            {chat.recordedCostUsd !== undefined && (
              <Kv k="費用">
                {usd(chat.recordedCostUsd)}
                <span className="block text-[12px] text-muted-foreground">{h.name} 自己記下的數字，按 API 牌價算；訂閱方案實際不是這樣收費</span>
              </Kv>
            )}
          </>
        ) : (
          <p className="text-[13px] text-muted-foreground">這段對話沒有用量紀錄：{h.name} 這個版本沒把 token 寫進檔案。不是 0。</p>
        )}
      </Section>
      <Section title="在哪裡做的" open>
        <Kv k="專案">{project.name}</Kv>
        <Kv k="資料夾">
          <span className="font-mono text-[12px]">{chat.cwd}</span>
          {location && !location.exists && (
            <span className="mt-0.5 flex items-center gap-1 text-[12px] text-amber-700 dark:text-amber-300">
              <FolderX className="size-3.5" />
              {location.role === 'worktree' ? 'worktree 已刪除' : '資料夾已不在磁碟上'}
            </span>
          )}
        </Kv>
        {project.remote && <Kv k="遠端">{project.remote}</Kv>}
        {chat.branch && <Kv k="分支"><span className="font-mono text-[12px]">{chat.branch}</span></Kv>}
        {chat.commit && <Kv k="開始時的 commit"><span className="font-mono text-[12px]">{chat.commit}</span></Kv>}
      </Section>
      <Section title="改動">
        {chat.linesAdded + chat.linesRemoved > 0 ? (
          <Kv k="程式碼">
            <span className="text-green">+{num(chat.linesAdded)}</span> <span className="text-red">−{num(chat.linesRemoved)}</span> 行 · {num(chat.filesTouched)} 個檔案
          </Kv>
        ) : (
          <p className="text-[13px] text-muted-foreground">沒有改任何檔案。</p>
        )}
        {chat.pr && (
          <Kv k="PR">
            <a href="#" onClick={(e) => e.preventDefault()} className="inline-flex items-center gap-1 text-brand hover:underline">
              {chat.pr.repo}#{chat.pr.number}
              <ExternalLink className="size-3" />
            </a>
          </Kv>
        )}
      </Section>
      <Section title="用什麼做的">
        <Kv k="工具">
          {h.name} {chat.version}
        </Kv>
        <Kv k="從哪裡開">{entrypointLabel[chat.entrypoint]}</Kv>
        <Kv k="模型">{chat.models.join('、')}</Kv>
        {chat.effort && <Kv k="推理強度">{chat.effort}</Kv>}
        {chat.permissionMode && <Kv k="權限模式">{chat.permissionMode}</Kv>}
        {chat.compactions > 0 && <Kv k="壓縮">{chat.compactions} 次</Kv>}
        {chat.apiErrors > 0 && <Kv k="API 錯誤">{chat.apiErrors} 次（都重試成功）</Kv>}
      </Section>
      <Section title="保存">
        <Kv k="狀態">{chat.source === 'gone' ? '只剩 PathKeep 有：工具資料夾裡的檔案已經不在' : '工具資料夾裡也還在'}</Kv>
        <Kv k="原始檔">
          <span className="font-mono text-[11.5px]">{sourcePath(chat)}</span>
        </Kv>
        <Kv k="大小">
          {bytes(chat.sizeBytes)} · {num(chat.events)} 筆紀錄
          {chat.subagentCount > 0 && ` · 另有 ${chat.subagentCount} 個 subagent 檔`}
        </Kv>
        <Kv k="上次備份">今天 16:05</Kv>
      </Section>
    </div>
  )
}
