/**
 * @file chat-history-explorer.tsx
 * @description The chat-history explorer: a collapsible conversation list (newest-first) that
 *              opens, renames, or deletes a past conversation and starts a new chat. The W-AI-3
 *              marquee companion to the streaming chat surface.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Render the conversation list (title + relative time + message count) in the paper aesthetic,
 *   newest-first, with the active conversation marked.
 * - Surface "New chat", per-row open, per-row rename (inline), and per-row delete with an inline
 *   confirm step that honors the alertdialog focus contract (focus the safe action, Escape cancels,
 *   focus restores to the row's delete trigger).
 * - Stay purely presentational: data + callbacks are injected so the route owns persistence and
 *   the component stays unit-testable without a backend.
 *
 * ## Not responsible for
 * - Loading / saving conversations (the route calls the backend-client and feeds props here).
 * - Streaming mechanics or the chat transcript itself.
 *
 * ## Fluidity
 * - The list is a bounded, recency-ordered page (the backend caps it), so it renders without
 *   virtualization; rows are cheap (no message bodies). State changes are local and small.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { cn } from '@/lib/cn'
import { PKGlyph } from '@/components/shell/pk-glyph'
import type { AgentConversationSummary } from '@/lib/types'

/** Copy bundle for the explorer; built from the `assistant` namespace by `buildChatHistoryCopy`. */
export interface ChatHistoryCopy {
  title: string
  openLabel: string
  closeLabel: string
  newChat: string
  loading: string
  errorTitle: string
  errorBody: string
  retry: string
  emptyTitle: string
  emptyBody: string
  emptyCta: string
  activeBadge: string
  deleteAction: string
  deleteConfirmTitle: string
  deleteConfirm: string
  deleteCancel: string
  renameAction: string
  /** aria-label for the inline rename input. */
  renameLabel: string
  renameSave: string
  renameCancel: string
  /** `(title) => "Open conversation: {title}"` aria-label builder. */
  openConversationLabel: (title: string) => string
  /** `(title) => "...remove the saved transcript for {title}..."` confirm body builder. */
  deleteConfirmBody: (title: string) => string
  /** `(count) => "{count} messages"` (handles the singular form). */
  messageCount: (count: number) => string
  /** `(updatedAt) => relative time string`, already locale-projected. */
  relativeTime: (updatedAt: string) => string
}

export interface ChatHistoryExplorerProps {
  /** Whether the drawer is open (route-owned so it can be responsive / collapsible). */
  open: boolean
  /**
   * When true, an EXTERNAL affordance owns opening the drawer (e.g. the assistant route's labeled
   * header doorway), so the drawer suppresses its OWN collapsed icon-only open-button to keep
   * exactly one open affordance. The in-drawer close button is unaffected. Defaults to false, so
   * standalone uses (no external doorway) keep their built-in open toggle.
   */
  externalOpenControl?: boolean
  conversations: readonly AgentConversationSummary[]
  /** The conversation currently loaded into the chat, if any. */
  activeId: string | null
  /** True while the list is loading; shows a skeleton row set. */
  loading?: boolean
  /** Set when the last list load failed; shows an inline retry. */
  error?: boolean
  copy: ChatHistoryCopy
  onToggle: () => void
  onNewChat: () => void
  onOpenConversation: (id: string) => void
  onDeleteConversation: (id: string) => void
  /** Rename a conversation. Optional: when absent, the rename affordance is hidden. */
  onRenameConversation?: (id: string, title: string) => void
  onRetry?: () => void
  testId?: string
}

/** Skeleton row count shown while the first list load is in flight. */
const SKELETON_ROWS = 4

/**
 * The conversation explorer drawer. Keeps a single "pending delete" id and a single "renaming" id
 * in local state so the confirm / rename steps are inline (no modal), matching the lightweight
 * paper aesthetic.
 */
export function ChatHistoryExplorer({
  open,
  externalOpenControl = false,
  conversations,
  activeId,
  loading = false,
  error = false,
  copy,
  onToggle,
  onNewChat,
  onOpenConversation,
  onDeleteConversation,
  onRenameConversation,
  onRetry,
  testId,
}: ChatHistoryExplorerProps) {
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null)
  const [renamingId, setRenamingId] = useState<string | null>(null)

  const requestDelete = useCallback((id: string) => {
    setRenamingId(null)
    setPendingDeleteId(id)
  }, [])
  const cancelDelete = useCallback(() => {
    setPendingDeleteId(null)
  }, [])
  const confirmDelete = useCallback(
    (id: string) => {
      setPendingDeleteId(null)
      onDeleteConversation(id)
    },
    [onDeleteConversation],
  )

  const requestRename = useCallback((id: string) => {
    setPendingDeleteId(null)
    setRenamingId(id)
  }, [])
  const cancelRename = useCallback(() => {
    setRenamingId(null)
  }, [])
  const confirmRename = useCallback(
    (id: string, title: string) => {
      setRenamingId(null)
      onRenameConversation?.(id, title)
    },
    [onRenameConversation],
  )

  if (!open) {
    // An external doorway (the route's labeled header button) owns opening the drawer — render no
    // collapsed open-button here so there is exactly ONE, more-accessible open affordance.
    if (externalOpenControl) return null
    return (
      <button
        type="button"
        onClick={onToggle}
        aria-label={copy.openLabel}
        title={copy.openLabel}
        data-testid={testId ? `${testId}-open` : undefined}
        className={cn(
          'rounded-paper inline-grid h-[38px] w-[38px] place-items-center',
          'border-border-default text-ink-secondary border bg-card-paper',
          'hover:border-accent hover:text-accent transition-colors duration-150',
        )}
      >
        <PKGlyph icon="history" size={18} strokeWidth={1.8} />
      </button>
    )
  }

  return (
    <aside
      data-testid={testId}
      aria-label={copy.title}
      className="border-border-light flex w-[260px] shrink-0 flex-col border-r"
    >
      <div className="flex items-center justify-between px-3 pb-2 pt-1">
        <span className="text-ink-faint font-mono text-[10px] uppercase tracking-[0.08em]">
          {copy.title}
        </span>
        <button
          type="button"
          onClick={onToggle}
          aria-label={copy.closeLabel}
          title={copy.closeLabel}
          data-testid={testId ? `${testId}-close` : undefined}
          className="text-ink-faint hover:text-accent transition-colors duration-150"
        >
          <PKGlyph icon="close" size={16} strokeWidth={1.8} />
        </button>
      </div>

      <button
        type="button"
        onClick={onNewChat}
        data-testid={testId ? `${testId}-new-chat` : undefined}
        className={cn(
          'mx-3 mb-2 flex items-center gap-2 rounded-paper px-3 py-2',
          'border-border-default text-ink-secondary border bg-card-paper',
          'hover:border-accent hover:text-accent transition-colors duration-150',
          'font-serif text-[14px]',
        )}
      >
        <PKGlyph icon="plus" size={16} strokeWidth={1.8} />
        <span>{copy.newChat}</span>
      </button>

      <div className="pk-scrollbar flex flex-1 flex-col gap-1 overflow-y-auto px-2 pb-3">
        {loading ? (
          <div data-testid={testId ? `${testId}-loading` : undefined}>
            <span className="sr-only">{copy.loading}</span>
            {Array.from({ length: SKELETON_ROWS }).map((_, index) => (
              <div
                // Skeleton placeholders have no stable identity; index keys are fine for a fixed,
                // non-reordered set rendered only during the brief loading window.
                key={`skeleton-${index}`}
                aria-hidden="true"
                // Opacity-only keyframe (not the scaling `pk-pulse`) so prefers-reduced-motion
                // leaves a clean, static bar instead of a frozen mid-scale rectangle.
                className="bg-border-light/50 mb-1 h-[44px] animate-[pk-skeleton-pulse_1.4s_ease-in-out_infinite] rounded-paper"
              />
            ))}
          </div>
        ) : error ? (
          <div
            data-testid={testId ? `${testId}-error` : undefined}
            role="alert"
            className="flex flex-col gap-2 px-2 py-4"
          >
            <p className="font-serif text-[14px] text-ink">{copy.errorTitle}</p>
            <p className="text-ink-faint text-[12px]">{copy.errorBody}</p>
            {onRetry ? (
              <button
                type="button"
                onClick={onRetry}
                className="text-accent self-start font-mono text-[11px] hover:underline"
              >
                {copy.retry}
              </button>
            ) : null}
          </div>
        ) : conversations.length === 0 ? (
          <div
            data-testid={testId ? `${testId}-empty` : undefined}
            className="flex flex-col items-center gap-2 px-2 py-6 text-center"
          >
            <p className="font-serif text-[14px] text-ink">{copy.emptyTitle}</p>
            <p className="text-ink-faint text-[12px]">{copy.emptyBody}</p>
            <button
              type="button"
              onClick={onNewChat}
              data-testid={testId ? `${testId}-empty-cta` : undefined}
              className={cn(
                'mt-1 flex items-center gap-1.5 rounded-paper px-3 py-1.5',
                'border-border-default text-ink-secondary border bg-card-paper',
                'hover:border-accent hover:text-accent transition-colors duration-150',
                'font-serif text-[13px]',
              )}
            >
              <PKGlyph icon="plus" size={14} strokeWidth={1.8} />
              <span>{copy.emptyCta}</span>
            </button>
          </div>
        ) : (
          <ul
            // Roving list semantics so assistive tech announces the conversation set; rows are list
            // items. The icon-only buttons inside carry their own aria-labels.
            role="list"
            aria-label={copy.title}
            className="m-0 flex list-none flex-col gap-1 p-0"
          >
            {conversations.map((conversation) => (
              <ConversationRow
                key={conversation.id}
                conversation={conversation}
                isActive={conversation.id === activeId}
                isPendingDelete={conversation.id === pendingDeleteId}
                isRenaming={conversation.id === renamingId}
                canRename={Boolean(onRenameConversation)}
                copy={copy}
                onOpen={onOpenConversation}
                onRequestDelete={requestDelete}
                onCancelDelete={cancelDelete}
                onConfirmDelete={confirmDelete}
                onRequestRename={requestRename}
                onCancelRename={cancelRename}
                onConfirmRename={confirmRename}
                testId={testId}
              />
            ))}
          </ul>
        )}
      </div>
    </aside>
  )
}

/** One conversation row, with an inline delete-confirm or rename overlay when armed. */
function ConversationRow({
  conversation,
  isActive,
  isPendingDelete,
  isRenaming,
  canRename,
  copy,
  onOpen,
  onRequestDelete,
  onCancelDelete,
  onConfirmDelete,
  onRequestRename,
  onCancelRename,
  onConfirmRename,
  testId,
}: {
  conversation: AgentConversationSummary
  isActive: boolean
  isPendingDelete: boolean
  isRenaming: boolean
  canRename: boolean
  copy: ChatHistoryCopy
  onOpen: (id: string) => void
  onRequestDelete: (id: string) => void
  onCancelDelete: () => void
  onConfirmDelete: (id: string) => void
  onRequestRename: (id: string) => void
  onCancelRename: () => void
  onConfirmRename: (id: string, title: string) => void
  testId?: string
}) {
  const rowTestId = testId ? `${testId}-row-${conversation.id}` : undefined
  // The delete trigger so focus can be restored to it after the confirm closes.
  const deleteTriggerRef = useRef<HTMLButtonElement | null>(null)
  // Remembers the prior confirm state so we restore focus when the row returns to normal (the
  // alertdialog contract: cancel/confirm hands focus back to the trigger that opened it). The
  // trigger button only exists in the normal-row branch, so this restore must run after that branch
  // has re-rendered — hence an effect keyed on `isPendingDelete`, not an inline call.
  const wasPendingDelete = useRef(false)
  useEffect(() => {
    if (wasPendingDelete.current && !isPendingDelete) {
      const frame = window.requestAnimationFrame(() =>
        deleteTriggerRef.current?.focus(),
      )
      wasPendingDelete.current = isPendingDelete
      return () => window.cancelAnimationFrame(frame)
    }
    wasPendingDelete.current = isPendingDelete
  }, [isPendingDelete])

  if (isRenaming) {
    return (
      <li role="listitem" className="list-none">
        <RenameRow
          conversation={conversation}
          copy={copy}
          onCancel={onCancelRename}
          onConfirm={onConfirmRename}
          rowTestId={rowTestId}
        />
      </li>
    )
  }

  if (isPendingDelete) {
    return (
      <li role="listitem" className="list-none">
        <DeleteConfirm
          conversation={conversation}
          copy={copy}
          onCancel={onCancelDelete}
          onConfirm={onConfirmDelete}
          rowTestId={rowTestId}
        />
      </li>
    )
  }

  return (
    <li role="listitem" className="list-none">
      <div
        data-testid={rowTestId}
        className={cn(
          'group relative rounded-paper transition-colors duration-150',
          isActive
            ? 'bg-accent-soft border-l-[2px] border-accent'
            : 'hover:bg-border-light/40',
        )}
      >
        <button
          type="button"
          onClick={() => onOpen(conversation.id)}
          aria-label={copy.openConversationLabel(conversation.title)}
          aria-current={isActive ? 'true' : undefined}
          className={cn(
            'flex w-full flex-col gap-[2px] py-2 pr-14 text-left',
            isActive ? 'pl-[10px]' : 'px-3',
          )}
        >
          <span className="truncate font-serif text-[14px] leading-[1.3] text-ink">
            {conversation.title}
          </span>
          <span className="text-ink-faint flex items-center gap-2 font-mono text-[10px]">
            <span>{copy.relativeTime(conversation.updatedAt)}</span>
            <span aria-hidden="true">·</span>
            <span>{copy.messageCount(conversation.messageCount)}</span>
            {isActive ? (
              <span className="text-accent ml-auto">{copy.activeBadge}</span>
            ) : null}
          </span>
        </button>
        <div className="absolute right-2 top-2 flex items-center gap-1">
          {canRename ? (
            <button
              type="button"
              onClick={() => onRequestRename(conversation.id)}
              aria-label={copy.renameAction}
              title={copy.renameAction}
              data-testid={rowTestId ? `${rowTestId}-rename` : undefined}
              className={cn(
                'text-ink-faint hover:text-accent transition-opacity duration-150',
                'opacity-0 focus:opacity-100 group-hover:opacity-100',
              )}
            >
              <PKGlyph icon="edit" size={15} strokeWidth={1.8} />
            </button>
          ) : null}
          <button
            type="button"
            ref={deleteTriggerRef}
            onClick={() => onRequestDelete(conversation.id)}
            aria-label={copy.deleteAction}
            title={copy.deleteAction}
            data-testid={rowTestId ? `${rowTestId}-delete` : undefined}
            className={cn(
              'text-ink-faint hover:text-error transition-opacity duration-150',
              'opacity-0 focus:opacity-100 group-hover:opacity-100',
            )}
          >
            <PKGlyph icon="delete_sweep" size={15} strokeWidth={1.8} />
          </button>
        </div>
      </div>
    </li>
  )
}

/**
 * The inline delete-confirm overlay. Implements the alertdialog focus contract: on mount it focuses
 * the safe "Keep"/cancel button (so a stray Enter never deletes), and Escape cancels. Focus is
 * restored to the row's delete trigger by the parent `ConversationRow` once this overlay closes
 * (the trigger only exists in the normal-row branch). Mirrors the Escape + focus-restore pattern in
 * `profile-switcher.tsx`.
 */
function DeleteConfirm({
  conversation,
  copy,
  onCancel,
  onConfirm,
  rowTestId,
}: {
  conversation: AgentConversationSummary
  copy: ChatHistoryCopy
  onCancel: () => void
  onConfirm: (id: string) => void
  rowTestId?: string
}) {
  const cancelRef = useRef<HTMLButtonElement | null>(null)

  // On arming the confirm, move focus to the safe action so a stray Enter never deletes.
  useEffect(() => {
    const frame = window.requestAnimationFrame(() => cancelRef.current?.focus())
    return () => window.cancelAnimationFrame(frame)
  }, [])

  return (
    <div
      data-testid={rowTestId ? `${rowTestId}-confirm` : undefined}
      role="alertdialog"
      aria-label={copy.deleteConfirmTitle}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault()
          event.stopPropagation()
          onCancel()
        }
      }}
      className="border-border-default rounded-paper border bg-card-paper px-3 py-2"
    >
      <p className="font-serif text-[13px] text-ink">
        {copy.deleteConfirmTitle}
      </p>
      <p className="text-ink-faint mt-1 text-[11px] leading-[1.4]">
        {copy.deleteConfirmBody(conversation.title)}
      </p>
      <div className="mt-2 flex justify-end gap-2">
        <button
          type="button"
          ref={cancelRef}
          onClick={onCancel}
          className="text-ink-secondary font-mono text-[11px] hover:underline"
        >
          {copy.deleteCancel}
        </button>
        <button
          type="button"
          onClick={() => onConfirm(conversation.id)}
          data-testid={rowTestId ? `${rowTestId}-confirm-delete` : undefined}
          className="text-error font-mono text-[11px] hover:underline"
        >
          {copy.deleteConfirm}
        </button>
      </div>
    </div>
  )
}

/** The inline rename overlay: a text input seeded with the current title plus save / cancel. */
function RenameRow({
  conversation,
  copy,
  onCancel,
  onConfirm,
  rowTestId,
}: {
  conversation: AgentConversationSummary
  copy: ChatHistoryCopy
  onCancel: () => void
  onConfirm: (id: string, title: string) => void
  rowTestId?: string
}) {
  const [value, setValue] = useState(conversation.title)
  const inputRef = useRef<HTMLInputElement | null>(null)

  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      inputRef.current?.focus()
      inputRef.current?.select()
    })
    return () => window.cancelAnimationFrame(frame)
  }, [])

  const submit = useCallback(() => {
    const trimmed = value.trim()
    // A blank title is rejected by the backend; never submit one — just cancel out.
    if (!trimmed) {
      onCancel()
      return
    }
    onConfirm(conversation.id, trimmed)
  }, [conversation.id, onCancel, onConfirm, value])

  return (
    <div
      data-testid={rowTestId ? `${rowTestId}-rename-row` : undefined}
      className="border-border-default rounded-paper border bg-card-paper px-3 py-2"
    >
      <input
        ref={inputRef}
        type="text"
        value={value}
        aria-label={copy.renameLabel}
        data-testid={rowTestId ? `${rowTestId}-rename-input` : undefined}
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            event.preventDefault()
            submit()
          } else if (event.key === 'Escape') {
            event.preventDefault()
            event.stopPropagation()
            onCancel()
          }
        }}
        className={cn(
          'border-border-light w-full rounded-paper border bg-transparent px-2 py-1',
          'font-serif text-[14px] text-ink outline-none focus:border-accent',
        )}
      />
      <div className="mt-2 flex justify-end gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="text-ink-secondary font-mono text-[11px] hover:underline"
        >
          {copy.renameCancel}
        </button>
        <button
          type="button"
          onClick={submit}
          data-testid={rowTestId ? `${rowTestId}-rename-save` : undefined}
          className="text-accent font-mono text-[11px] hover:underline"
        >
          {copy.renameSave}
        </button>
      </div>
    </div>
  )
}
