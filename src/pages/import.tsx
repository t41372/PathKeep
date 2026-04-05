import { useEffect, useState } from 'react'
import { useApp } from '../lib/app-context'
import { formatDateTime } from '../lib/format'
import {
  EmptyState,
  FieldBlock,
  Glyph,
  PreviewEntryList,
  StatusTag,
  Surface,
} from '../components/ui'
import { backend } from '../lib/backend'
import type { ImportBatchDetail, TakeoutInspection } from '../lib/types'

export function ImportPage() {
  const {
    t,
    resolvedLanguage,
    snapshot,
    initialized,
    unlocked,
    runTask,
    setNotice,
    setError,
    reloadSnapshot,
  } = useApp()

  const [takeoutPath, setTakeoutPath] = useState('')
  const [takeoutInspection, setTakeoutInspection] =
    useState<TakeoutInspection | null>(null)
  const [selectedBatchId, setSelectedBatchId] = useState<number | null>(null)
  const [batchDetail, setBatchDetail] = useState<ImportBatchDetail | null>(null)

  const batches = snapshot?.recentImportBatches ?? []

  useEffect(() => {
    if (!batches.length) {
      setSelectedBatchId(null)
      setBatchDetail(null)
      return
    }
    const stillExists = batches.some((b) => b.id === selectedBatchId)
    if (!stillExists) {
      setSelectedBatchId(batches[0].id)
    }
  }, [batches, selectedBatchId])

  useEffect(() => {
    if (selectedBatchId == null) return
    let cancelled = false

    void (async () => {
      try {
        const detail = await backend.previewImportBatch(selectedBatchId)
        if (!cancelled) setBatchDetail(detail)
      } catch (taskError) {
        if (!cancelled) {
          setError(
            taskError instanceof Error ? taskError.message : String(taskError),
          )
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [selectedBatchId, setError])

  async function handleTakeout(dryRun: boolean) {
    if (!takeoutPath) {
      setError(t('enterTakeoutPath'))
      return
    }

    await runTask(dryRun ? t('dryRun') : t('importSupported'), async () => {
      const request = { sourcePath: takeoutPath, dryRun }
      const response = dryRun
        ? await backend.inspectTakeout(request)
        : await backend.importTakeout(request)
      setTakeoutInspection(response)
      if (!dryRun) {
        await reloadSnapshot()
        if (response.importBatch) {
          setSelectedBatchId(response.importBatch.id)
          const detail = await backend.previewImportBatch(
            response.importBatch.id,
          )
          setBatchDetail(detail)
        }
      }
      setNotice(
        dryRun
          ? t('takeoutDryRunNotice')
          : t('takeoutImportNotice', { count: response.importedItems }),
      )
    })
  }

  async function handleRevertBatch(batchId: number) {
    if (!window.confirm(t('revertBatchConfirm'))) return

    await runTask(t('revertBatch'), async () => {
      const detail = await backend.revertImportBatch(batchId)
      setBatchDetail(detail)
      setTakeoutInspection((current) =>
        current && current.importBatch?.id === batchId
          ? { ...current, importBatch: detail.batch }
          : current,
      )
      await reloadSnapshot()
      setSelectedBatchId(batchId)
      setNotice(t('revertBatchNotice'))
    })
  }

  return (
    <div className="pageContent">
      <section className="pageIntro">
        <p className="sectionEyebrow">{t('importNav')}</p>
        <h2>{t('importDescription')}</h2>
      </section>

      {!initialized || !unlocked ? (
        <EmptyState icon="lock" message={t('archiveLocked')} />
      ) : (
        <>
          {/* Takeout wizard */}
          <Surface
            eyebrow={t('importTakeout')}
            title={t('takeoutImportTitle')}
            icon="upload_file"
          >
            <FieldBlock label={t('takeoutPathLabel')}>
              <input
                className="textInput"
                type="text"
                placeholder={t('takeoutPathPlaceholder')}
                value={takeoutPath}
                onChange={(e) => setTakeoutPath(e.target.value)}
              />
            </FieldBlock>
            <div className="pathActions">
              <button
                className="secondaryButton"
                type="button"
                disabled={!takeoutPath}
                onClick={() => handleTakeout(true)}
              >
                <Glyph icon="preview" />
                {t('dryRun')}
              </button>
              <button
                className="primaryButton"
                type="button"
                disabled={!takeoutPath}
                onClick={() => handleTakeout(false)}
              >
                <Glyph icon="upload_file" />
                {t('importSupported')}
              </button>
            </div>
          </Surface>

          {/* Dry run results */}
          {takeoutInspection && (
            <Surface
              eyebrow={t('takeoutResults')}
              title={t('takeoutResults')}
              icon="summarize"
            >
              <div className="inspectionStats">
                <span>
                  {t('candidateItems')}: {takeoutInspection.candidateItems}
                </span>
                <span>
                  {t('importedItems')}: {takeoutInspection.importedItems}
                </span>
                <span>
                  {t('duplicateItems')}: {takeoutInspection.duplicateItems}
                </span>
              </div>

              {takeoutInspection.recognizedFiles.length > 0 && (
                <div>
                  <h4>{t('recognizedFiles')}</h4>
                  {takeoutInspection.recognizedFiles.map((file) => (
                    <div key={file.path} className="takeoutFileRow">
                      <StatusTag tone="success">{file.kind}</StatusTag>
                      <span>{file.path}</span>
                      <span className="muted">
                        {file.records} {t('records')}
                      </span>
                    </div>
                  ))}
                </div>
              )}

              {takeoutInspection.quarantinedFiles.length > 0 && (
                <div>
                  <h4>{t('quarantinedFiles')}</h4>
                  {takeoutInspection.quarantinedFiles.map((file) => (
                    <div key={file.path} className="takeoutFileRow quarantine">
                      <StatusTag tone="danger">{file.status}</StatusTag>
                      <span>{file.path}</span>
                    </div>
                  ))}
                </div>
              )}

              {takeoutInspection.previewEntries.length > 0 && (
                <PreviewEntryList
                  entries={takeoutInspection.previewEntries}
                  language={resolvedLanguage}
                />
              )}
            </Surface>
          )}

          {/* Batch history */}
          {batches.length > 0 && (
            <Surface
              eyebrow={t('importHistory')}
              title={t('importHistory')}
              icon="history"
            >
              <div className="batchList">
                {batches.map((batch) => (
                  <button
                    key={batch.id}
                    className={`batchRow ${selectedBatchId === batch.id ? 'selected' : ''}`}
                    type="button"
                    onClick={() => setSelectedBatchId(batch.id)}
                  >
                    <div className="batchHeader">
                      <StatusTag
                        tone={
                          batch.status === 'imported'
                            ? 'success'
                            : batch.status === 'reverted'
                              ? 'danger'
                              : 'info'
                        }
                      >
                        {batch.status}
                      </StatusTag>
                      <strong>{batch.sourcePath}</strong>
                    </div>
                    <span className="muted">
                      {formatDateTime(batch.createdAt, resolvedLanguage)} ·{' '}
                      {batch.importedItems} {t('records')}
                    </span>
                  </button>
                ))}
              </div>

              {batchDetail && (
                <div className="batchDetailPanel">
                  <div className="batchDetailHeader">
                    <StatusTag
                      tone={
                        batchDetail.batch.status === 'imported'
                          ? 'success'
                          : 'info'
                      }
                    >
                      {batchDetail.batch.status}
                    </StatusTag>
                    {batchDetail.batch.status === 'imported' && (
                      <button
                        className="dangerButton"
                        type="button"
                        onClick={() => handleRevertBatch(batchDetail.batch.id)}
                      >
                        <Glyph icon="undo" />
                        {t('revertBatch')}
                      </button>
                    )}
                  </div>
                  {batchDetail.previewEntries.length > 0 && (
                    <PreviewEntryList
                      entries={batchDetail.previewEntries}
                      language={resolvedLanguage}
                    />
                  )}
                </div>
              )}
            </Surface>
          )}
        </>
      )}
    </div>
  )
}
