/**
 * Rename and Add folder (`agent-history.md` §5.7). Rename gives a project
 * a name only PathKeep shows; empty goes back to the main folder's name.
 * Add folder takes another project's folders into this one: only projects
 * with conversations that are not the loose folders, not added somewhere
 * already, and not holding folders of their own added from others.
 */
import { useState, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import { useI18n } from '@/lib/i18n'
import { useProjectList } from '@/lib/queries/agent-projects'
import { ProjectIcon } from './project-icon'
import { projectName, tildePath } from './project-words'
import { useProjectEdit } from './use-project-edit'

/** The name field, its help line and Save: in the dialog and the settings sheet. */
export function NameForm({
  project,
  onSaved,
  footer,
}: {
  project: ProjectCard
  onSaved?: () => void
  /** Extra buttons before Save (the dialog's Cancel). */
  footer?: ReactNode
}) {
  const { t } = useI18n()
  const edit = useProjectEdit()
  const shown = projectName(t, project)
  const [name, setName] = useState(shown)
  const [saving, setSaving] = useState(false)
  const unchanged = name.trim() === shown.trim()
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(event) => {
        event.preventDefault()
        if (unchanged || saving) return
        setSaving(true)
        void edit(
          { kind: 'rename', id: project.id, name: name.trim() },
          { title: t('agentProjects.rename.saved'), noUndo: true },
        ).then((result) => {
          setSaving(false)
          if (result) onSaved?.()
        })
      }}
    >
      <div className="flex flex-col gap-2">
        <Label htmlFor={`project-name-${project.id}`}>
          {t('agentProjects.rename.name')}
        </Label>
        <Input
          id={`project-name-${project.id}`}
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <p className="text-[12.5px] text-muted-foreground">
          {t('agentProjects.rename.help', { original: project.defaultName })}
        </p>
      </div>
      <DialogFooter>
        {footer}
        <Button type="submit" disabled={unchanged || saving}>
          {t('agentProjects.rename.save')}
        </Button>
      </DialogFooter>
    </form>
  )
}

export function RenameDialog({
  project,
  onClose,
}: {
  project: ProjectCard | null
  onClose: () => void
}) {
  const { t } = useI18n()
  return (
    <Dialog open={project !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>{t('agentProjects.rename.title')}</DialogTitle>
          <DialogDescription className="font-mono text-[12px] break-all">
            {project?.mainFolder ??
              projectName(t, project ?? { displayName: '', kind: 'scratch' })}
          </DialogDescription>
        </DialogHeader>
        {project && (
          <NameForm
            key={project.id}
            project={project}
            onSaved={onClose}
            footer={
              <Button type="button" variant="outline" onClick={onClose}>
                {t('common.cancel')}
              </Button>
            }
          />
        )}
      </DialogContent>
    </Dialog>
  )
}

export function AddFolderDialog({
  project,
  home,
  onClose,
}: {
  project: ProjectCard | null
  home: string | null
  onClose: () => void
}) {
  const { t, locale } = useI18n()
  const edit = useProjectEdit()
  const candidates = useProjectList(
    { candidateFor: project?.id ?? '' },
    project !== null,
    false,
  )
  const name = project ? projectName(t, project) : ''
  const items = (candidates.data?.items ?? []).filter(
    (card) => card.conversationCount > 0,
  )
  return (
    <Dialog open={project !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="gap-3 p-0 sm:max-w-[520px]">
        <DialogHeader className="px-5 pt-5">
          <DialogTitle>
            {t('agentProjects.addFolder.title', { name })}
          </DialogTitle>
          <DialogDescription>
            {t('agentProjects.addFolder.body', { name })}
          </DialogDescription>
        </DialogHeader>
        <Command className="border-t">
          <CommandInput placeholder={t('agentProjects.addFolder.search')} />
          <CommandList className="max-h-[360px]">
            {candidates.data && (
              <CommandEmpty>{t('agentProjects.addFolder.none')}</CommandEmpty>
            )}
            {!candidates.data && (
              <p
                role="status"
                className="px-3 py-3 text-[12.5px] text-muted-foreground"
              >
                {candidates.isError
                  ? t('agentProjects.states.failed')
                  : t('agentProjects.states.loading')}
              </p>
            )}
            <CommandGroup>
              {items.map((card) => (
                <CommandItem
                  key={card.id}
                  value={`${projectName(t, card)} ${card.mainFolder ?? ''} ${card.id}`}
                  onSelect={() => {
                    if (!project) return
                    const path = card.mainFolder ?? projectName(t, card)
                    onClose()
                    void edit(
                      { kind: 'join', from: card.id, into: project.id },
                      {
                        title: t('agentProjects.toast.added', {
                          path: tildePath(path, home),
                          name,
                        }),
                        description: t('agentProjects.toast.addedBody', {
                          count: card.conversationCount,
                          shown: card.conversationCount.toLocaleString(locale),
                        }),
                      },
                    )
                  }}
                >
                  <ProjectIcon project={card} />
                  <span className="truncate">{projectName(t, card)}</span>
                  <span className="truncate text-[12px] text-muted-foreground">
                    {card.parentFolder
                      ? tildePath(card.parentFolder, home)
                      : ''}
                  </span>
                  <span className="ml-auto shrink-0 text-[12px] text-muted-foreground tabular">
                    {t('agentProjects.row.conversations', {
                      count: card.conversationCount,
                    })}
                  </span>
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </DialogContent>
    </Dialog>
  )
}
