/**
 * How the user moves between sites: three different sites visited one after
 * another in a session, most frequent first. Round trips ("search engine →
 * site → search engine") are not paths and are left out by the backend.
 */
import { ChevronRight } from 'lucide-react'
import { Fragment } from 'react'
import { Link } from 'react-router-dom'
import { Favicon } from '@/components/app/favicon'
import { SectionCard } from '@/components/app/section-card'
import { useFormat, useI18n } from '@/lib/i18n'
import { sitePath } from './links'
import { ListSkeleton, QueryBody, ScrollList, SectionStatus } from './parts'
import { usePaths } from './queries'
import type { Scope } from './range'

export function PathsCard({ scope }: { scope: Scope }) {
  const { t } = useI18n()
  const format = useFormat()
  const paths = usePaths(scope)
  return (
    <SectionCard
      title={t('insights.paths.title')}
      subtitle={t('insights.paths.subtitle')}
      action={
        <SectionStatus
          meta={paths.data?.meta}
          explain={t('insights.paths.explain')}
        />
      }
    >
      <QueryBody
        query={paths}
        skeleton={<ListSkeleton rows={5} />}
        isEmpty={(result) => result.data.items.length === 0}
        empty={t('insights.paths.empty')}
      >
        {(result) => (
          <ScrollList maxHeight={6 * 40}>
            <ul className="flex flex-col gap-1">
              {result.data.items.map((path) => (
                <li
                  key={path.flowId}
                  className="flex items-center justify-between gap-3 py-1 text-[13px]"
                >
                  <span className="flex min-w-0 flex-1 items-center gap-1">
                    {path.steps.map((step, index) => (
                      <Fragment key={step.index}>
                        {index > 0 && (
                          <ChevronRight
                            className="size-3.5 shrink-0 text-muted-foreground"
                            aria-label={t('insights.paths.then')}
                          />
                        )}
                        {step.registrableDomain ? (
                          <Link
                            to={sitePath(step.registrableDomain, scope)}
                            className="flex min-w-0 items-center gap-1.5 rounded-md px-1 hover:bg-muted hover:no-underline"
                          >
                            <Favicon domain={step.registrableDomain} />
                            <span className="truncate">{step.label}</span>
                          </Link>
                        ) : (
                          <span className="truncate px-1">{step.label}</span>
                        )}
                      </Fragment>
                    ))}
                  </span>
                  <span className="shrink-0 text-xs text-muted-foreground tabular">
                    {t('insights.paths.times', {
                      count: path.occurrenceCount,
                      times: format.number(path.occurrenceCount),
                    })}
                  </span>
                </li>
              ))}
            </ul>
          </ScrollList>
        )}
      </QueryBody>
    </SectionCard>
  )
}
