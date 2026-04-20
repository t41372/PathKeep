/**
 * Shared generated-artifact viewer for preview/manual surfaces that expose
 * file tabs, code previews, and open/copy-path actions.
 */

import type { ReactNode } from 'react'
import { useState } from 'react'
import {
  ReviewCodePreview,
  ReviewCopyStatus,
  type ReviewCopyFeedback,
} from './review-surface'

export interface GeneratedArtifactFile {
  absolutePath?: string | null
  contents: string
  purpose: ReactNode
  relativePath: string
}

export function GeneratedArtifactViewer({
  copyFeedback,
  copyLabel,
  copyPathLabel,
  errorMessage,
  files,
  onCopy,
  onOpenPath,
  openPathLabel,
  successMessage,
}: {
  copyFeedback: ReviewCopyFeedback | null
  copyLabel: string
  copyPathLabel: string
  errorMessage: string
  files: GeneratedArtifactFile[]
  onCopy: (key: string, payload: string) => void
  onOpenPath?: (path: string) => void
  openPathLabel: string
  successMessage: string
}) {
  const [selectedRelativePath, setSelectedRelativePath] = useState<
    string | null
  >(files[0]?.relativePath ?? null)
  const selectedFile =
    files.find((file) => file.relativePath === selectedRelativePath) ??
    files[0] ??
    null

  if (!selectedFile) {
    return null
  }

  const pathKey = `path:${selectedFile.relativePath}`
  const contentsKey = `contents:${selectedFile.relativePath}`

  return (
    <>
      <div className="generated-file-tabs">
        {files.map((file) => (
          <button
            key={file.relativePath}
            className={`chip-button ${
              selectedFile?.relativePath === file.relativePath
                ? 'chip-button--active'
                : ''
            }`}
            type="button"
            onClick={() => setSelectedRelativePath(file.relativePath)}
          >
            {file.relativePath}
          </button>
        ))}
      </div>
      <ReviewCodePreview
        actions={
          selectedFile.absolutePath ? (
            <>
              {onOpenPath ? (
                <button
                  className="btn-tiny"
                  type="button"
                  onClick={() => {
                    void onOpenPath(selectedFile.absolutePath ?? '')
                  }}
                >
                  {openPathLabel}
                </button>
              ) : null}
              <button
                className="btn-tiny"
                type="button"
                onClick={() => {
                  void onCopy(
                    pathKey,
                    selectedFile.absolutePath ?? selectedFile.relativePath,
                  )
                }}
              >
                {copyPathLabel}
              </button>
            </>
          ) : null
        }
        code={selectedFile.contents}
        copyFeedback={copyFeedback}
        copyKey={contentsKey}
        copyLabel={copyLabel}
        errorMessage={errorMessage}
        onCopy={onCopy}
        successMessage={successMessage}
        title={selectedFile.purpose}
        titleMeta={
          <span className="mono dim">{selectedFile.relativePath}</span>
        }
      />
      {selectedFile.absolutePath ? (
        <ReviewCopyStatus
          copyFeedback={copyFeedback}
          copyKey={pathKey}
          errorMessage={errorMessage}
          successMessage={successMessage}
        />
      ) : null}
    </>
  )
}
