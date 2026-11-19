/**
 * The built-in embedding model's download as the backend reports it, and the
 * index build its setup asks for afterwards. Mirrors the Rust
 * `ModelDownloadProgressEvent`, `ModelDownloadWait`, `ModelDownloadTransfer`
 * and `BuildAiIndexRequest`; `docs/architecture/desktop-command-surface.md`
 * describes the protocol.
 */
import type { AiIndexRequest } from './intelligence'

/**
 * One in-app embedding model download progress event, delivered on the
 * `pathkeep://model-download-progress` Tauri channel. Mirrors the Rust
 * `ModelDownloadProgressEvent` (`#[serde(tag = "kind", rename_all = "camelCase")]`).
 *
 * The stream is per file (no upfront file count; `totalBytes` is 0 when the server sent no
 * content-length), so the UI shows honest per-file or indeterminate progress, never a made-up
 * percent. `fileProgress` comes about every 1 MB plus a final 100%. Each transfer ends with exactly
 * one of `done`, `error` or `cancelled`.
 */
export type ModelDownloadProgressEvent = {
  /** The transfer this event belongs to; see `ModelDownloadTransfer`. */
  transferId: string
} & (
  | { kind: 'waiting'; reason: ModelDownloadWait }
  | { kind: 'fileStarted'; file: string; totalBytes: number }
  | {
      kind: 'fileProgress'
      file: string
      downloadedBytes: number
      totalBytes: number
    }
  | { kind: 'fileFinished'; file: string }
  | { kind: 'done' }
  /** Something the user asked for stopped it (Cancel, or a saved setting that no longer wants it). */
  | { kind: 'cancelled' }
  | { kind: 'error'; message: string }
)

/**
 * What a `waiting` download waits for: `archive-busy`, another archive task
 * (an export, a restore, a move import) holding the archive;
 * `previous-transfer`, a cancelled download that still holds its network
 * response or files. Sent at once and then every 20 s while it waits: a sign
 * of life, not progress.
 */
export type ModelDownloadWait = 'archive-busy' | 'previous-transfer'

/**
 * The answer to starting the model download. A start while one is running
 * attaches to it and gets the same ID, so two screens never run two downloads.
 * Cancel names this ID, and every progress event carries it.
 */
export interface ModelDownloadTransfer {
  transferId: string
}

/** What `build_ai_index` takes: an index request, and after the built-in model's download its transfer. */
export interface BuildAiIndexRequest extends AiIndexRequest {
  /**
   * The download whose `done` this build follows. The backend refuses it when the saved settings no
   * longer want the built-in model, and answers a repeat with the first job. Ordinary builds leave it out.
   */
  transferId?: string
}
