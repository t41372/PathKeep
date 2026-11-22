/**
 * The screen PathKeep shows instead of the app when it won't open the
 * archive in its data folder (`get_archive_access`): why, in words, and the
 * action that fixes it. Every state the backend can answer has its own
 * words; see `classify.ts`.
 *
 * Nothing on this screen deletes data. The actions are: updating PathKeep
 * (an archive a newer version changed), rebuilding a lost or damaged
 * settings file, making a store a PathKeep 0.5 archive lacks, restoring
 * browsing history from a safety copy (under the archive's own password, or
 * under the copy's on explicit choice when that key is lost), and choosing
 * what happens to the Agent conversations set aside for PathKeep 0.5.0.
 * PathKeep 0.5's unfinished "Delete all data" is described, not finished.
 *
 * The desktop answers only the recovery commands while this screen shows
 * (unless only the opening failed), so the screen uses nothing else: paths
 * are copied rather than shown in the file manager, and progress is a
 * sliding bar with what is happening.
 */
import { useState } from 'react'
import { backdropDragRegion } from '@/components/app/drag-region'
import { BrandMark } from '@/components/app/brand-mark'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { UpdateRow } from '@/features/settings/update-row'
import { RollbackOffer } from '@/features/settings/rollback-dialog'
import { useSession } from '../../session'
import { classify, offersRestore, type AccessKind } from './classify'
import { CopyRestore } from './copy-restore'
import { HandoffChoice } from './handoff-choice'
import { AfterRepair, Details, PathLine } from './parts'
import { RepairAction } from './repair-action'

const storeWords: Record<string, MessageKey> = {
  ask: 'archiveAccess.store.ask',
  intelligence: 'archiveAccess.store.intelligence',
  evidence: 'archiveAccess.store.evidence',
  other: 'archiveAccess.store.other',
}

function words(kind: AccessKind, prepared: boolean) {
  if (kind.kind === 'handoff' && prepared)
    return {
      title: 'archiveAccess.title.prepared',
      body: 'archiveAccess.body.prepared',
    } as const
  if (kind.kind === 'storeMissing')
    return {
      title: 'archiveAccess.title.storeMissing',
      body: storeWords[kind.store],
    } as const
  return {
    title: `archiveAccess.title.${kind.kind}`,
    body: `archiveAccess.body.${kind.kind}`,
  } as const
}

export function AccessScreen() {
  const { t } = useI18n()
  const session = useSession()
  const access = session.access
  const [checking, setChecking] = useState(false)
  if (!access) return null
  const kind = classify(access)
  const { title, body } = words(kind, session.prepared !== null)
  const detail = [access.code, access.detail].filter(Boolean).join(': ')

  const checkAgain = async () => {
    setChecking(true)
    try {
      await session.refresh()
    } finally {
      setChecking(false)
    }
  }

  return (
    <div
      {...backdropDragRegion()}
      className="fixed inset-0 overflow-y-auto"
      data-archive-access={access.code ?? access.state}
    >
      <main className="mx-auto flex w-full max-w-xl animate-rise flex-col items-stretch gap-4 px-8 py-12">
        <BrandMark className="mb-1 size-12 self-center" />
        <h1 className="text-center text-lg font-semibold">{t(title)}</h1>
        <p className="text-center text-sm text-muted-foreground">{t(body)}</p>
        {(kind.kind === 'newer' || kind.kind === 'writer') &&
          kind.needed !== null && (
            <p className="text-center text-[13px] text-muted-foreground">
              {t('archiveAccess.versions', {
                build: access.buildLevel,
                needed: kind.needed,
              })}
            </p>
          )}

        <PathLine label={t('archiveAccess.folder')} path={access.root} />

        <Primary kind={kind} />

        {offersRestore(kind.kind) && (
          <>
            {hasPrimary(kind) && (
              <h2 className="pt-2 text-[13px] font-medium text-muted-foreground">
                {t('archiveAccess.otherWays')}
              </h2>
            )}
            <CopyRestore />
          </>
        )}
        {(kind.kind === 'recoveryMode' || kind.kind === 'unknown') && (
          <RollbackOffer />
        )}

        {detail && <Details text={detail} />}

        {kind.kind !== 'handoff' && (
          <Button
            variant="ghost"
            size="sm"
            className="self-center"
            disabled={checking}
            onClick={() => void checkAgain()}
          >
            {checking && <Spinner />}
            {checking
              ? t('archiveAccess.checking')
              : t('archiveAccess.checkAgain')}
          </Button>
        )}
      </main>
    </div>
  )
}

/** Whether `Primary` shows an action for this kind. */
function hasPrimary(kind: AccessKind) {
  switch (kind.kind) {
    case 'storeMissing':
      return kind.store === 'ask' || kind.store === 'intelligence'
    case 'unreadable':
    case 'installRecord':
    case 'handoffInvalid':
    case 'legacyWipe':
    case 'unknown':
      return false
    default:
      return true
  }
}

/** The action that fixes this kind, first on the screen. */
function Primary({ kind }: { kind: AccessKind }) {
  const session = useSession()
  switch (kind.kind) {
    case 'newer':
    case 'writer':
      return <UpdateRow version={session.buildInfo?.version ?? ''} />
    case 'controlMissing':
    case 'controlInvalid':
      return <RepairAction repair="settings" />
    case 'storeMissing':
      return kind.store === 'ask' || kind.store === 'intelligence' ? (
        <RepairAction repair={kind.store} />
      ) : null
    case 'handoff':
      return <HandoffChoice />
    case 'interrupted':
    case 'recoveryMode':
      // Starting again runs the ordinary startup, which finishes or undoes
      // an interrupted change itself.
      return <AfterRepair />
    default:
      return null
  }
}
