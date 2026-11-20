/**
 * "Continue" (`agent-history.md` §4.2): how to pick the conversation up
 * again in the tool. PathKeep copies a command only for a tool version
 * whose resume command is confirmed; otherwise it copies the session ID.
 * When only PathKeep has the conversation, the tool cannot continue it,
 * and the popover says so. PathKeep never opens a terminal itself.
 */
import { ChevronDown, Copy, TerminalSquare } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import type { ChatHeader } from '@/lib/backend-client/agent-chats'
import { useI18n } from '@/lib/i18n'
import { copyText, toolLabel } from './reader-words'

export function ResumeMenu({ header }: { header: ChatHeader }) {
  const { t } = useI18n()
  const tool = toolLabel(t, header.summary.harness)
  const gone = header.summary.onlyInPathKeep
  const folderGone =
    header.folderState === 'gone' || header.folderState === 'worktreeGone'
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm">
          <TerminalSquare aria-hidden />
          {t('agentReader.resume.button')}
          <ChevronDown className="opacity-60" aria-hidden />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="flex w-[380px] flex-col gap-2 p-3">
        {gone ? (
          <p className="text-[12.5px] leading-relaxed">
            {t('agentReader.resume.gone', { tool })}
          </p>
        ) : (
          <>
            {folderGone && (
              <p className="text-[12.5px] leading-relaxed text-foreground">
                {t('agentReader.resume.folderGone')}
              </p>
            )}
            {header.resumeCommand ? (
              <>
                <p className="text-[12.5px] text-muted-foreground">
                  {t('agentReader.resume.command', { tool })}
                </p>
                <pre className="overflow-x-auto rounded-md bg-muted p-2 font-mono text-[11.5px] whitespace-pre-wrap [overflow-wrap:anywhere]">
                  {header.resumeCommand}
                </pre>
                <Button
                  size="sm"
                  variant="secondary"
                  className="self-start"
                  onClick={() =>
                    copyText(
                      t,
                      header.resumeCommand!,
                      t('agentReader.resume.commandCopied'),
                    )
                  }
                >
                  <Copy aria-hidden />
                  {t('agentReader.resume.copyCommand')}
                </Button>
              </>
            ) : header.nativeSessionId ? (
              <>
                <p className="text-[12.5px] leading-relaxed text-muted-foreground">
                  {t('agentReader.resume.unconfirmed', { tool })}
                </p>
                <Button
                  size="sm"
                  variant="secondary"
                  className="self-start"
                  onClick={() =>
                    copyText(
                      t,
                      header.nativeSessionId!,
                      t('agentReader.resume.idCopied'),
                    )
                  }
                >
                  <Copy aria-hidden />
                  {t('agentReader.resume.copyId')}
                </Button>
              </>
            ) : (
              <p className="text-[12.5px] text-muted-foreground">
                {t('agentReader.resume.noId', { tool })}
              </p>
            )}
          </>
        )}
        <p className="pt-1 text-[11.5px] text-muted-foreground">
          {t('agentReader.resume.footnote', { tool })}
        </p>
      </PopoverContent>
    </Popover>
  )
}
