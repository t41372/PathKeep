/**
 * What an address in a space shows, by its kind's state: the space's page
 * while the kind is on or paused, and while it is off a page that says so,
 * with the ways to turn it on (`data-spaces.md` §2).
 *
 * Responsibilities: choosing between those, and what shows before the
 * backend has answered: a space that is on by default shows its page at
 * once, as it did before kinds could be turned off; any other waits.
 * Not responsible for: the page for a kind that is off
 * (`features/data-kinds/kind-off-page.tsx`).
 */
import { lazy, Suspense } from 'react'
import { Outlet } from 'react-router-dom'
import { CardError } from '@/components/app/section-card'
import { useDataKinds } from '@/lib/queries/data-kinds'
import { kindStateOf, type DataSpace } from '@/lib/spaces'
import { ScreenFallback } from './screen-fallback'

const KindOffPage = lazy(() => import('@/features/data-kinds/kind-off-page'))

export function KindGate({ space }: { space: DataSpace }) {
  const kinds = useDataKinds()
  const state = kindStateOf(kinds.data, space.kind)

  if (state === undefined) {
    if (space.onByDefault) return <Outlet />
    if (kinds.isError) {
      return (
        <div className="mx-auto flex w-full max-w-[520px] flex-col justify-center px-10">
          <CardError error={kinds.error} onRetry={() => void kinds.refetch()} />
        </div>
      )
    }
    return <ScreenFallback />
  }
  if (state === 'off') {
    return (
      <Suspense fallback={<ScreenFallback />}>
        <KindOffPage space={space} />
      </Suspense>
    )
  }
  return <Outlet />
}
