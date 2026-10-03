/**
 * Star state for the rows on screen. Status is asked for in batches as rows
 * load (the backend canonicalizes URLs, so it cannot be read off the star
 * list), and toggles are optimistic.
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { starsClient } from '@/lib/backend-client/stars'
import { useI18n } from '@/lib/i18n'
import type { DetailTarget } from './history-types'

const BATCH = 200

export function useStars(urls: string[]) {
  const { t } = useI18n()
  const client = useQueryClient()
  const [status, setStatus] = useState<ReadonlyMap<string, boolean>>(
    () => new Map(),
  )
  const requested = useRef(new Set<string>())

  useEffect(() => {
    const missing = [...new Set(urls)].filter(
      (url) => !requested.current.has(url),
    )
    for (let index = 0; index < missing.length; index += BATCH) {
      const chunk = missing.slice(index, index + BATCH)
      for (const url of chunk) requested.current.add(url)
      starsClient
        .getStarStatus({ entityKind: 'url', entityKeys: chunk })
        .then((result) =>
          setStatus(
            (previous) => new Map([...previous, ...Object.entries(result)]),
          ),
        )
        .catch(() => {
          for (const url of chunk) requested.current.delete(url)
        })
    }
  }, [urls])

  const apply = useCallback((url: string, value: boolean) => {
    requested.current.add(url)
    setStatus((previous) => new Map(previous).set(url, value))
  }, [])

  const toggle = useCallback(
    async (target: DetailTarget, value: boolean) => {
      apply(target.url, value)
      const request = {
        entityKind: 'url' as const,
        entityKey: target.url,
        sourceProfile: target.profileId ?? null,
      }
      try {
        await (value
          ? starsClient.setStar(request)
          : starsClient.unsetStar(request))
        toast(
          t(value ? 'history.detail.starAdded' : 'history.detail.starRemoved'),
        )
      } catch (error) {
        apply(target.url, !value)
        toast.error(t('history.detail.starFailed'), {
          description: error instanceof Error ? error.message : undefined,
        })
      } finally {
        void client.invalidateQueries({ queryKey: ['stars'] })
      }
    },
    [apply, client, t],
  )

  const isStarred = useCallback(
    (url: string) => status.get(url) === true,
    [status],
  )
  return { isStarred, toggle }
}
