/**
 * Archive health: runs the read-only checks (`doctor_report`) and, when one
 * fails, offers the repair (`repair_health`) behind a confirmation that says
 * what it touches.
 */
import { useMutation } from '@tanstack/react-query'
import { CircleAlert, CircleCheck } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { auditClient } from '@/lib/backend-client/audit'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import type { HealthCheck } from '@/lib/types'

const knownChecks = [
  'config',
  'archive-db',
  'archive-unlock',
  'schema-version',
  'manifest-chain',
  'browser-sources',
  'snapshot-artifacts',
  'import-audit-artifacts',
  'broken-visibility-references',
  'derived-state-freshness',
] as const

type CheckCode = (typeof knownChecks)[number]

function isKnown(code: string | undefined): code is CheckCode {
  return (knownChecks as readonly (string | undefined)[]).includes(code)
}

export function HealthRow() {
  const { t } = useI18n()
  const [confirm, setConfirm] = useState(false)
  const check = useMutation({ mutationFn: auditClient.getHealthReport })
  const repair = useMutation({
    mutationFn: auditClient.repairHealth,
    onSuccess: () => {
      toast.success(t('settingsAbout.health.repaired'))
      check.mutate()
    },
    onError: (error) =>
      toast.error(t('settingsAbout.health.repairFailed'), {
        description: describeError(error, 'repair_health'),
      }),
  })

  const report = check.data
  const failing = report?.checks.filter((item) => !item.ok) ?? []
  const busy = check.isPending || repair.isPending

  const name = (item: HealthCheck) =>
    isKnown(item.code)
      ? t(`settingsAbout.health.check.${item.code}`)
      : item.name

  return (
    <SettingRow
      title={t('settingsAbout.health.title')}
      description={
        check.isError ? (
          <span className="text-destructive">
            {t('settingsAbout.health.failed', {
              message: describeError(check.error, 'doctor_report'),
            })}
          </span>
        ) : report ? (
          failing.length === 0 ? (
            t('settingsAbout.health.allGood', { count: report.checks.length })
          ) : (
            t('settingsAbout.health.problems', { count: failing.length })
          )
        ) : (
          t('settingsAbout.health.description')
        )
      }
      control={
        <>
          {failing.length > 0 && (
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => setConfirm(true)}
            >
              {repair.isPending && <Spinner />}
              {t('settingsAbout.health.repair')}
            </Button>
          )}
          <Button
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => check.mutate()}
          >
            {check.isPending && <Spinner />}
            {report
              ? t('settingsAbout.health.again')
              : t('settingsAbout.health.run')}
          </Button>
        </>
      }
    >
      {report && (
        <ul className="flex flex-col gap-1.5 border-t pt-3 text-[13px]">
          {report.checks.map((item) => (
            <li key={item.code ?? item.name} className="flex gap-2">
              {item.ok ? (
                <CircleCheck
                  className="mt-0.5 size-4 shrink-0 text-green"
                  aria-label={t('settingsAbout.health.ok')}
                />
              ) : (
                <CircleAlert
                  className="mt-0.5 size-4 shrink-0 text-destructive"
                  aria-label={t('settingsAbout.health.notOk')}
                />
              )}
              <span className="flex min-w-0 flex-col">
                <span>{name(item)}</span>
                {!item.ok && item.detail && (
                  <span className="text-xs break-words text-muted-foreground">
                    {item.detail}
                  </span>
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settingsAbout.health.repairTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('settingsAbout.health.repairBody')}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => repair.mutate()}>
              {t('settingsAbout.health.repair')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}
