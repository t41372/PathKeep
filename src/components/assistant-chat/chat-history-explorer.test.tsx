/**
 * @file chat-history-explorer.test.tsx
 * @description Behavior coverage for the chat-history explorer drawer.
 *
 * Proves: collapsed toggle, open header + new-chat, loading skeleton, error + retry, empty state,
 * the conversation list (active marker + relative time + message count), open / delete-with-confirm
 * (and cancel), all driven through injected copy + callbacks (no backend, no i18n runtime).
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  ChatHistoryExplorer,
  type ChatHistoryCopy,
} from './chat-history-explorer'
import type { AgentConversationSummary } from '../../lib/types'

const copy: ChatHistoryCopy = {
  title: 'Conversations',
  openLabel: 'Show conversations',
  closeLabel: 'Hide conversations',
  newChat: 'New chat',
  loading: 'Loading conversations…',
  errorTitle: "Couldn't load conversations",
  errorBody: 'Your saved chats are still on disk. Try again.',
  retry: 'Try again',
  emptyTitle: 'No saved chats yet',
  emptyBody: 'Your conversations are saved here as you chat.',
  emptyCta: 'Start a chat',
  activeBadge: 'Current',
  deleteAction: 'Delete conversation',
  deleteConfirmTitle: 'Delete this conversation?',
  deleteConfirm: 'Delete',
  deleteCancel: 'Keep',
  renameAction: 'Rename conversation',
  renameLabel: 'New conversation title',
  renameSave: 'Save',
  renameCancel: 'Cancel',
  openConversationLabel: (title) => `Open conversation: ${title}`,
  deleteConfirmBody: (title) => `This removes "${title}".`,
  messageCount: (count) => (count === 1 ? '1 message' : `${count} messages`),
  relativeTime: () => '5 min ago',
}

function conversation(
  overrides: Partial<AgentConversationSummary> = {},
): AgentConversationSummary {
  return {
    id: 'conv-1',
    title: 'First conversation',
    providerId: 'llm-local',
    createdAt: '2026-06-20T10:00:00Z',
    updatedAt: '2026-06-20T10:05:00Z',
    messageCount: 4,
    ...overrides,
  }
}

function renderExplorer(
  props: Partial<React.ComponentProps<typeof ChatHistoryExplorer>> = {},
) {
  const handlers = {
    onToggle: vi.fn(),
    onNewChat: vi.fn(),
    onOpenConversation: vi.fn(),
    onDeleteConversation: vi.fn(),
    onRenameConversation: vi.fn(),
    onRetry: vi.fn(),
  }
  render(
    <ChatHistoryExplorer
      open
      conversations={[conversation()]}
      activeId={null}
      copy={copy}
      testId="explorer"
      {...handlers}
      {...props}
    />,
  )
  return handlers
}

describe('ChatHistoryExplorer', () => {
  test('collapsed: renders only a toggle button', () => {
    const onToggle = vi.fn()
    render(
      <ChatHistoryExplorer
        open={false}
        conversations={[]}
        activeId={null}
        copy={copy}
        onToggle={onToggle}
        onNewChat={vi.fn()}
        onOpenConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
        testId="explorer"
      />,
    )
    const toggle = screen.getByTestId('explorer-open')
    expect(toggle).toBeVisible()
    expect(screen.queryByTestId('explorer')).not.toBeInTheDocument()
    fireEvent.click(toggle)
    expect(onToggle).toHaveBeenCalledTimes(1)
  })

  test('open: header shows title, close, and new-chat actions', () => {
    const handlers = renderExplorer()
    expect(screen.getByText('Conversations')).toBeVisible()
    fireEvent.click(screen.getByTestId('explorer-close'))
    expect(handlers.onToggle).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByTestId('explorer-new-chat'))
    expect(handlers.onNewChat).toHaveBeenCalledTimes(1)
  })

  test('loading: shows the skeleton placeholders and the SR loading label', () => {
    renderExplorer({ loading: true })
    expect(screen.getByTestId('explorer-loading')).toBeInTheDocument()
    expect(screen.getByText('Loading conversations…')).toBeInTheDocument()
    // The conversation rows are not rendered while loading.
    expect(screen.queryByTestId('explorer-row-conv-1')).not.toBeInTheDocument()
  })

  test('error: shows the error copy and a retry that calls onRetry', () => {
    const handlers = renderExplorer({ error: true })
    expect(screen.getByTestId('explorer-error')).toBeInTheDocument()
    fireEvent.click(screen.getByText('Try again'))
    expect(handlers.onRetry).toHaveBeenCalledTimes(1)
  })

  test('error without onRetry omits the retry button', () => {
    render(
      <ChatHistoryExplorer
        open
        conversations={[]}
        activeId={null}
        error
        copy={copy}
        onToggle={vi.fn()}
        onNewChat={vi.fn()}
        onOpenConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
        testId="explorer"
      />,
    )
    expect(screen.getByTestId('explorer-error')).toBeInTheDocument()
    expect(screen.queryByText('Try again')).not.toBeInTheDocument()
  })

  test('empty: shows the empty state when there are no conversations', () => {
    renderExplorer({ conversations: [] })
    expect(screen.getByTestId('explorer-empty')).toBeInTheDocument()
    expect(screen.getByText('No saved chats yet')).toBeVisible()
  })

  test('lists conversations with relative time and message count, marking the active one', () => {
    renderExplorer({
      conversations: [
        conversation({ id: 'conv-1', title: 'Active one', messageCount: 1 }),
        conversation({ id: 'conv-2', title: 'Other one', messageCount: 3 }),
      ],
      activeId: 'conv-1',
    })
    expect(screen.getByText('Active one')).toBeVisible()
    expect(screen.getByText('1 message')).toBeVisible()
    expect(screen.getByText('3 messages')).toBeVisible()
    expect(screen.getAllByText('5 min ago')).toHaveLength(2)
    // The active conversation shows the "Current" badge.
    expect(screen.getByText('Current')).toBeVisible()
  })

  test('clicking a row opens that conversation', () => {
    const handlers = renderExplorer()
    fireEvent.click(
      screen.getByRole('button', {
        name: 'Open conversation: First conversation',
      }),
    )
    expect(handlers.onOpenConversation).toHaveBeenCalledWith('conv-1')
  })

  test('delete asks for confirmation, then confirms', () => {
    const handlers = renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-delete'))
    // The inline confirm replaces the row.
    expect(
      screen.getByTestId('explorer-row-conv-1-confirm'),
    ).toBeInTheDocument()
    expect(screen.getByText('This removes "First conversation".')).toBeVisible()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-confirm-delete'))
    expect(handlers.onDeleteConversation).toHaveBeenCalledWith('conv-1')
  })

  test('delete confirmation can be cancelled', () => {
    const handlers = renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-delete'))
    fireEvent.click(screen.getByText('Keep'))
    // Back to the normal row; no delete fired.
    expect(screen.getByTestId('explorer-row-conv-1')).toBeInTheDocument()
    expect(
      screen.queryByTestId('explorer-row-conv-1-confirm'),
    ).not.toBeInTheDocument()
    expect(handlers.onDeleteConversation).not.toHaveBeenCalled()
  })

  test('renders and operates without a testId (open + delete via aria/text)', () => {
    const onOpenConversation = vi.fn()
    const onDeleteConversation = vi.fn()
    render(
      <ChatHistoryExplorer
        open
        conversations={[conversation()]}
        activeId="conv-1"
        copy={copy}
        onToggle={vi.fn()}
        onNewChat={vi.fn()}
        onOpenConversation={onOpenConversation}
        onDeleteConversation={onDeleteConversation}
      />,
    )
    // The aria-labelled aside is present even without a testId.
    expect(
      screen.getByRole('complementary', { name: 'Conversations' }),
    ).toBeInTheDocument()

    // Open via the aria-label (no testId branch).
    fireEvent.click(
      screen.getByRole('button', {
        name: 'Open conversation: First conversation',
      }),
    )
    expect(onOpenConversation).toHaveBeenCalledWith('conv-1')

    // Delete + confirm via aria-labels / text (no testId branch in the confirm overlay).
    fireEvent.click(screen.getByRole('button', { name: 'Delete conversation' }))
    expect(
      screen.getByRole('alertdialog', { name: 'Delete this conversation?' }),
    ).toBeInTheDocument()
    fireEvent.click(screen.getByText('Delete'))
    expect(onDeleteConversation).toHaveBeenCalledWith('conv-1')
  })

  test('loading and error states render without a testId', () => {
    const { rerender } = render(
      <ChatHistoryExplorer
        open
        conversations={[]}
        activeId={null}
        loading
        copy={copy}
        onToggle={vi.fn()}
        onNewChat={vi.fn()}
        onOpenConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
      />,
    )
    expect(screen.getByText('Loading conversations…')).toBeInTheDocument()

    rerender(
      <ChatHistoryExplorer
        open
        conversations={[]}
        activeId={null}
        error
        copy={copy}
        onToggle={vi.fn()}
        onNewChat={vi.fn()}
        onOpenConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
      />,
    )
    expect(screen.getByRole('alert')).toBeInTheDocument()

    rerender(
      <ChatHistoryExplorer
        open
        conversations={[]}
        activeId={null}
        copy={copy}
        onToggle={vi.fn()}
        onNewChat={vi.fn()}
        onOpenConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
      />,
    )
    expect(screen.getByText('No saved chats yet')).toBeInTheDocument()
  })

  test('collapsed toggle renders without a testId', () => {
    const onToggle = vi.fn()
    render(
      <ChatHistoryExplorer
        open={false}
        conversations={[]}
        activeId={null}
        copy={copy}
        onToggle={onToggle}
        onNewChat={vi.fn()}
        onOpenConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Show conversations' }))
    expect(onToggle).toHaveBeenCalledTimes(1)
  })

  test('empty state offers a "Start a chat" CTA wired to onNewChat', () => {
    const handlers = renderExplorer({ conversations: [] })
    const cta = screen.getByTestId('explorer-empty-cta')
    expect(cta).toHaveTextContent('Start a chat')
    fireEvent.click(cta)
    expect(handlers.onNewChat).toHaveBeenCalledTimes(1)
  })

  test('conversation rows use list / listitem semantics', () => {
    renderExplorer({
      conversations: [
        conversation({ id: 'conv-1', title: 'A' }),
        conversation({ id: 'conv-2', title: 'B' }),
      ],
    })
    const list = screen.getByRole('list', { name: 'Conversations' })
    expect(list).toBeInTheDocument()
    expect(screen.getAllByRole('listitem')).toHaveLength(2)
  })

  test('the active row uses the accent-active convention, not the faint border', () => {
    renderExplorer({ activeId: 'conv-1' })
    const row = screen.getByTestId('explorer-row-conv-1')
    expect(row.className).toContain('bg-accent-soft')
    expect(row.className).toContain('border-accent')
    expect(row.className).not.toContain('bg-border-light/60')
  })

  test('arming the delete confirm focuses the safe "Keep" action', async () => {
    renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-delete'))
    const keep = screen.getByText('Keep')
    await vi.waitFor(() => expect(keep).toHaveFocus())
  })

  test('a non-Escape key on the delete confirm does not cancel it', () => {
    const handlers = renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-delete'))
    const dialog = screen.getByRole('alertdialog')
    fireEvent.keyDown(dialog, { key: 'Enter' })
    // The confirm stays armed (only Escape cancels here) and no delete fired.
    expect(
      screen.getByTestId('explorer-row-conv-1-confirm'),
    ).toBeInTheDocument()
    expect(handlers.onDeleteConversation).not.toHaveBeenCalled()
  })

  test('Escape cancels the delete confirm and restores focus to the delete trigger', async () => {
    const handlers = renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-delete'))
    const dialog = screen.getByRole('alertdialog')
    fireEvent.keyDown(dialog, { key: 'Escape' })
    // Back to the normal row, no delete fired, focus restored to the (re-rendered) trigger.
    expect(
      screen.queryByTestId('explorer-row-conv-1-confirm'),
    ).not.toBeInTheDocument()
    expect(handlers.onDeleteConversation).not.toHaveBeenCalled()
    await vi.waitFor(() =>
      expect(screen.getByTestId('explorer-row-conv-1-delete')).toHaveFocus(),
    )
  })

  test('cancelling the delete confirm restores focus to the delete trigger', async () => {
    renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-delete'))
    fireEvent.click(screen.getByText('Keep'))
    await vi.waitFor(() =>
      expect(screen.getByTestId('explorer-row-conv-1-delete')).toHaveFocus(),
    )
  })

  test('rename: opens an inline editor seeded with the title and saves the trimmed value', async () => {
    const handlers = renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-rename'))
    const input = screen.getByTestId('explorer-row-conv-1-rename-input')
    // The editor is seeded with the current title and auto-focused on open.
    expect(input).toHaveValue('First conversation')
    await vi.waitFor(() => expect(input).toHaveFocus())
    fireEvent.change(input, { target: { value: '  Renamed chat  ' } })
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-rename-save'))
    expect(handlers.onRenameConversation).toHaveBeenCalledWith(
      'conv-1',
      'Renamed chat',
    )
  })

  test('rename: Enter submits, Escape cancels without firing rename', () => {
    const handlers = renderExplorer()
    // Enter submits.
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-rename'))
    const input = screen.getByTestId('explorer-row-conv-1-rename-input')
    fireEvent.change(input, { target: { value: 'Via enter' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(handlers.onRenameConversation).toHaveBeenCalledWith(
      'conv-1',
      'Via enter',
    )

    // Re-open and Escape: no second rename.
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-rename'))
    const reopened = screen.getByTestId('explorer-row-conv-1-rename-input')
    fireEvent.keyDown(reopened, { key: 'Escape' })
    expect(handlers.onRenameConversation).toHaveBeenCalledTimes(1)
    expect(screen.getByTestId('explorer-row-conv-1')).toBeInTheDocument()
  })

  test('rename: a blank title cancels instead of submitting an empty rename', () => {
    const handlers = renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-rename'))
    const input = screen.getByTestId('explorer-row-conv-1-rename-input')
    fireEvent.change(input, { target: { value: '   ' } })
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-rename-save'))
    expect(handlers.onRenameConversation).not.toHaveBeenCalled()
    expect(screen.getByTestId('explorer-row-conv-1')).toBeInTheDocument()
  })

  test('rename: a non-submit key (e.g. typing a letter) neither submits nor cancels', () => {
    const handlers = renderExplorer()
    fireEvent.click(screen.getByTestId('explorer-row-conv-1-rename'))
    const input = screen.getByTestId('explorer-row-conv-1-rename-input')
    fireEvent.keyDown(input, { key: 'a' })
    expect(handlers.onRenameConversation).not.toHaveBeenCalled()
    // Still in the rename editor (not cancelled back to the normal row).
    expect(input).toBeInTheDocument()
  })

  test('rename operates without a testId (open + save via aria/text)', () => {
    const onRenameConversation = vi.fn()
    render(
      <ChatHistoryExplorer
        open
        conversations={[conversation()]}
        activeId={null}
        copy={copy}
        onToggle={vi.fn()}
        onNewChat={vi.fn()}
        onOpenConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
        onRenameConversation={onRenameConversation}
      />,
    )
    // Open the rename editor via its aria-label (no testId branch).
    fireEvent.click(screen.getByRole('button', { name: 'Rename conversation' }))
    const input = screen.getByRole('textbox', {
      name: 'New conversation title',
    })
    fireEvent.change(input, { target: { value: 'No-testId rename' } })
    fireEvent.click(screen.getByText('Save'))
    expect(onRenameConversation).toHaveBeenCalledWith(
      'conv-1',
      'No-testId rename',
    )
  })

  test('the rename affordance is hidden when no onRenameConversation is provided', () => {
    render(
      <ChatHistoryExplorer
        open
        conversations={[conversation()]}
        activeId={null}
        copy={copy}
        onToggle={vi.fn()}
        onNewChat={vi.fn()}
        onOpenConversation={vi.fn()}
        onDeleteConversation={vi.fn()}
        testId="explorer"
      />,
    )
    expect(
      screen.queryByTestId('explorer-row-conv-1-rename'),
    ).not.toBeInTheDocument()
  })
})
