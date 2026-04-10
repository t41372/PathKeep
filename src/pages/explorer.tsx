import { useDeferredValue, useEffect, useState } from 'react'
import { useApp } from '../lib/app-context'
import { formatDateTime } from '../lib/format'
import { EmptyState, FieldBlock, Surface } from '../components/ui'
import { backend } from '../lib/backend'
import type { ExportFormat, HistoryQueryResponse } from '../lib/types'

export function ExplorerPage() {
  const { t, resolvedLanguage, unlocked, snapshot, runTask, setNotice } =
    useApp()

  const [history, setHistory] = useState<HistoryQueryResponse | null>(null)
  const [searchInput, setSearchInput] = useState('')
  const [domain, setDomain] = useState('')
  const [profileFilter, setProfileFilter] = useState('')
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const deferredSearch = useDeferredValue(searchInput)

  useEffect(() => {
    if (!unlocked) return

    void (async () => {
      const response = await backend.queryHistory({
        q: deferredSearch || null,
        domain: domain || null,
        profileId: profileFilter || null,
        limit: 160,
      })
      setHistory(response)
    })()
  }, [deferredSearch, domain, profileFilter, unlocked])

  const activeSelectedId = history?.items.some((item) => item.id === selectedId)
    ? selectedId
    : (history?.items[0]?.id ?? null)

  const selectedEntry =
    history?.items.find((item) => item.id === activeSelectedId) ??
    history?.items[0] ??
    null

  async function handleExport(format: ExportFormat) {
    await runTask(`${t('exportLabel')} ${format.toUpperCase()}`, async () => {
      const result = await backend.exportHistory({
        format,
        query: {
          q: deferredSearch || null,
          domain: domain || null,
          profileId: profileFilter || null,
          limit: 200,
        },
      })
      setNotice(`${t('exportLabel')} ${result.count} -> ${result.path}`)
    })
  }

  const profiles = snapshot?.browserProfiles ?? []

  return (
    <div className="pageContent">
      <section className="pageIntro">
        <p className="sectionEyebrow">{t('explorerNav')}</p>
        <h2>{t('explorerDescription')}</h2>
      </section>

      {!unlocked ? (
        <EmptyState icon="lock" message={t('archiveLocked')} />
      ) : (
        <>
          {/* Search & filters */}
          <Surface
            eyebrow={t('searchLabel')}
            title={t('searchLabel')}
            icon="search"
          >
            <div className="explorerFilters">
              <FieldBlock label={t('searchLabel')}>
                <input
                  className="textInput"
                  type="search"
                  placeholder={t('searchPlaceholder')}
                  value={searchInput}
                  onChange={(e) => setSearchInput(e.target.value)}
                />
              </FieldBlock>
              <FieldBlock label={t('domainFilter')}>
                <input
                  className="textInput"
                  type="text"
                  placeholder={t('domainFilterPlaceholder')}
                  value={domain}
                  onChange={(e) => setDomain(e.target.value)}
                />
              </FieldBlock>
              <FieldBlock label={t('profileFilter')}>
                <select
                  className="selectInput"
                  value={profileFilter}
                  onChange={(e) => setProfileFilter(e.target.value)}
                >
                  <option value="">{t('allProfiles')}</option>
                  {profiles.map((p) => (
                    <option key={p.profileId} value={p.profileId}>
                      {p.profileName} ({p.browserName})
                    </option>
                  ))}
                </select>
              </FieldBlock>
            </div>

            <div className="explorerActions">
              <span className="muted">
                {history ? t('resultsCount', { count: history.total }) : '…'}
              </span>
              <div className="exportButtons">
                {(['jsonl', 'html', 'markdown', 'text'] as ExportFormat[]).map(
                  (fmt) => (
                    <button
                      key={fmt}
                      className="secondaryButton"
                      type="button"
                      onClick={() => handleExport(fmt)}
                    >
                      {fmt.toUpperCase()}
                    </button>
                  ),
                )}
              </div>
            </div>
          </Surface>

          {/* Results */}
          <div className="explorerGrid">
            <div className="explorerList">
              {history?.items.map((item) => (
                <button
                  key={item.id}
                  className={`explorerRow ${activeSelectedId === item.id ? 'selected' : ''}`}
                  type="button"
                  onClick={() => setSelectedId(item.id)}
                >
                  <div className="explorerRowMain">
                    <strong>{item.title ?? item.url}</strong>
                    <span className="explorerDomain">{item.domain}</span>
                  </div>
                  <span className="explorerTime">
                    {formatDateTime(item.visitedAt, resolvedLanguage)}
                  </span>
                </button>
              ))}
              {history && history.items.length === 0 && (
                <EmptyState icon="search_off" message={t('noResultsFound')} />
              )}
            </div>

            {/* Detail panel */}
            {selectedEntry && (
              <Surface
                eyebrow={t('detailPanel')}
                title={selectedEntry.title ?? selectedEntry.url}
                icon="info"
              >
                <div className="detailGrid">
                  <div className="detailField">
                    <span className="fieldLabel">{t('urlLabel')}</span>
                    <a
                      href={selectedEntry.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="detailLink"
                    >
                      {selectedEntry.url}
                    </a>
                  </div>
                  <div className="detailField">
                    <span className="fieldLabel">{t('domainLabel')}</span>
                    <span>{selectedEntry.domain}</span>
                  </div>
                  <div className="detailField">
                    <span className="fieldLabel">{t('profileLabel')}</span>
                    <span>{selectedEntry.profileId}</span>
                  </div>
                  <div className="detailField">
                    <span className="fieldLabel">{t('visitTimeLabel')}</span>
                    <span>
                      {formatDateTime(
                        selectedEntry.visitedAt,
                        resolvedLanguage,
                      )}
                    </span>
                  </div>
                  <div className="detailField">
                    <span className="fieldLabel">{t('transitionLabel')}</span>
                    <span>
                      {selectedEntry.transition == null
                        ? t('notAvailable')
                        : String(selectedEntry.transition)}
                    </span>
                  </div>
                </div>
              </Surface>
            )}
          </div>
        </>
      )}
    </div>
  )
}
