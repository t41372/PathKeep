/**
 * What any address of a kind that is off shows (a bookmark, an old link):
 * that it is off, that what it saved is still in the archive, and the ways
 * to turn it on (`data-spaces.md` §2). Agent conversations also offer the
 * preview that turns them on.
 *
 * Not responsible for: deciding that the kind is off (`app/shell/kind-gate.tsx`).
 */
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import type { DataSpace } from '@/lib/spaces'
import { useOpenEnableAgent } from '@/features/enable-agent/links'

export default function KindOffPage({ space }: { space: DataSpace }) {
  const { t } = useI18n()
  const openEnable = useOpenEnableAgent()
  const Icon = space.icon
  const name = t(space.name)
  return (
    <div className="flex flex-1 animate-rise items-center justify-center overflow-y-auto p-10">
      <div className="flex max-w-[440px] flex-col items-center gap-3 text-center">
        <Icon
          aria-hidden
          className="size-8 text-muted-foreground"
          strokeWidth={1.5}
        />
        <h1 className="text-lg font-semibold">
          {t('dataKinds.off.title', { name })}
        </h1>
        <p className="text-[13px] leading-relaxed text-muted-foreground">
          {t('dataKinds.off.body')}
        </p>
        <div className="mt-1 flex flex-wrap justify-center gap-2">
          <Button variant="outline" asChild>
            <Link to="/settings/kinds">{t('dataKinds.off.manage')}</Link>
          </Button>
          {space.kind === 'agent-history' && (
            <Button onClick={openEnable}>
              {t('dataKinds.off.enable', { name })}
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
