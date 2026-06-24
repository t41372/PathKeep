/**
 * @file export-conversation-menu.test.tsx
 * @description Covers the Export menu: disabled-when-empty, open/close, the two format actions,
 *              keyboard navigation, and the announced success / error / cancelled states.
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import {
  ExportConversationMenu,
  type ExportConversationMenuCopy,
} from './export-conversation-menu'

const COPY: ExportConversationMenuCopy = {
  triggerLabel: 'Export',
  menuLabel: 'Export this conversation',
  markdownLabel: 'Markdown (.md)',
  jsonLabel: 'JSON (.json)',
  exportingLabel: 'Exporting…',
  successLabel: 'Conversation exported',
  errorLabel: "Couldn't export the conversation",
}

function renderMenu(
  overrides: {
    hasMessages?: boolean
    onExport?: (format: 'markdown' | 'json') => Promise<boolean>
  } = {},
) {
  const onExport = overrides.onExport ?? vi.fn().mockResolvedValue(true)
  render(
    <ExportConversationMenu
      copy={COPY}
      hasMessages={overrides.hasMessages ?? true}
      onExport={onExport}
      testId="export"
    />,
  )
  return { onExport }
}

describe('ExportConversationMenu', () => {
  test('disables the trigger when there are no messages', () => {
    renderMenu({ hasMessages: false })
    expect(screen.getByTestId('export-trigger')).toBeDisabled()
  })

  test('opens the menu and exports as Markdown', async () => {
    const user = userEvent.setup()
    const onExport = vi.fn().mockResolvedValue(true)
    renderMenu({ onExport })

    await user.click(screen.getByTestId('export-trigger'))
    expect(screen.getByRole('menu')).toBeVisible()

    await user.click(screen.getByTestId('export-markdown'))
    expect(onExport).toHaveBeenCalledWith('markdown')
    // Menu closes after a pick.
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    // Success is announced.
    await waitFor(() =>
      expect(screen.getByTestId('export-status')).toHaveTextContent(
        'Conversation exported',
      ),
    )
  })

  test('exports as JSON', async () => {
    const user = userEvent.setup()
    const onExport = vi.fn().mockResolvedValue(true)
    renderMenu({ onExport })

    await user.click(screen.getByTestId('export-trigger'))
    await user.click(screen.getByTestId('export-json'))
    expect(onExport).toHaveBeenCalledWith('json')
  })

  test('stays silent when the save dialog is cancelled (resolves false)', async () => {
    const user = userEvent.setup()
    const onExport = vi.fn().mockResolvedValue(false)
    renderMenu({ onExport })

    await user.click(screen.getByTestId('export-trigger'))
    await user.click(screen.getByTestId('export-markdown'))
    await waitFor(() => expect(onExport).toHaveBeenCalled())
    // No "exported" claim when nothing was written.
    expect(screen.getByTestId('export-status')).toHaveTextContent('')
  })

  test('announces a failure when the export rejects', async () => {
    const user = userEvent.setup()
    const onExport = vi.fn().mockRejectedValue(new Error('disk full'))
    renderMenu({ onExport })

    await user.click(screen.getByTestId('export-trigger'))
    await user.click(screen.getByTestId('export-json'))
    await waitFor(() =>
      expect(screen.getByTestId('export-status')).toHaveTextContent(
        "Couldn't export the conversation",
      ),
    )
  })

  test('opens via ArrowDown and navigates items with the keyboard', async () => {
    const onExport = vi.fn().mockResolvedValue(true)
    renderMenu({ onExport })

    const trigger = screen.getByTestId('export-trigger')
    trigger.focus()
    // ArrowDown opens the menu and focuses the first item.
    fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    await waitFor(() =>
      expect(screen.getByTestId('export-markdown')).toHaveFocus(),
    )

    const markdown = screen.getByTestId('export-markdown')
    const json = screen.getByTestId('export-json')

    fireEvent.keyDown(markdown, { key: 'ArrowDown' })
    expect(json).toHaveFocus()
    fireEvent.keyDown(json, { key: 'ArrowDown' }) // wraps to first
    expect(markdown).toHaveFocus()
    fireEvent.keyDown(markdown, { key: 'ArrowUp' }) // wraps to last
    expect(json).toHaveFocus()
    fireEvent.keyDown(json, { key: 'Home' })
    expect(markdown).toHaveFocus()
    fireEvent.keyDown(markdown, { key: 'End' })
    expect(json).toHaveFocus()

    // Escape closes and returns focus to the trigger.
    fireEvent.keyDown(json, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(trigger).toHaveFocus()
  })

  test('ArrowUp on the trigger also opens the menu', async () => {
    renderMenu()
    const trigger = screen.getByTestId('export-trigger')
    fireEvent.keyDown(trigger, { key: 'ArrowUp' })
    await waitFor(() => expect(screen.getByRole('menu')).toBeVisible())
  })

  test('a non-arrow key on the trigger does not open the menu', () => {
    renderMenu()
    const trigger = screen.getByTestId('export-trigger')
    fireEvent.keyDown(trigger, { key: 'Enter' })
    expect(screen.queryByRole('menu')).toBeNull()
  })

  test('a second export clears the prior status timer before scheduling a new one', async () => {
    const user = userEvent.setup()
    const onExport = vi.fn().mockResolvedValue(true)
    renderMenu({ onExport })

    // First export: settle the promise + run the finally (schedules the auto-clear timer).
    await user.click(screen.getByTestId('export-trigger'))
    await user.click(screen.getByTestId('export-markdown'))
    await waitFor(() =>
      expect(screen.getByTestId('export-status')).toHaveTextContent(
        'Conversation exported',
      ),
    )

    // Second export BEFORE the (2.6s) auto-clear fires: exercises the "clear the existing timer"
    // branch in runExport and its finally.
    await user.click(screen.getByTestId('export-trigger'))
    await user.click(screen.getByTestId('export-markdown'))
    await waitFor(() => expect(onExport).toHaveBeenCalledTimes(2))
    expect(screen.getByTestId('export-status')).toHaveTextContent(
      'Conversation exported',
    )
  })

  test('auto-clears the announced result after the timeout window', async () => {
    const user = userEvent.setup()
    const onExport = vi.fn().mockResolvedValue(true)
    renderMenu({ onExport })

    await user.click(screen.getByTestId('export-trigger'))
    await user.click(screen.getByTestId('export-markdown'))
    await waitFor(() =>
      expect(screen.getByTestId('export-status')).toHaveTextContent(
        'Conversation exported',
      ),
    )
    // The 2.6s auto-clear timer fires and returns the announcer to empty.
    await waitFor(
      () => expect(screen.getByTestId('export-status')).toHaveTextContent(''),
      { timeout: 4000 },
    )
  })

  test('clears a pending status timer on unmount', async () => {
    const user = userEvent.setup()
    const clearSpy = vi.spyOn(globalThis, 'clearTimeout')
    const onExport = vi.fn().mockResolvedValue(true)
    const { unmount } = render(
      <ExportConversationMenu
        copy={COPY}
        hasMessages
        onExport={onExport}
        testId="export"
      />,
    )

    await user.click(screen.getByTestId('export-trigger'))
    await user.click(screen.getByTestId('export-markdown'))
    await waitFor(() =>
      expect(screen.getByTestId('export-status')).toHaveTextContent(
        'Conversation exported',
      ),
    )

    clearSpy.mockClear()
    // Unmount while the auto-clear timer is still pending → the cleanup clears it.
    unmount()
    expect(clearSpy).toHaveBeenCalled()
    clearSpy.mockRestore()
  })

  test('a non-navigation key on an item is ignored', async () => {
    const user = userEvent.setup()
    renderMenu()
    await user.click(screen.getByTestId('export-trigger'))
    const markdown = screen.getByTestId('export-markdown')
    markdown.focus()
    fireEvent.keyDown(markdown, { key: 'a' })
    // Menu stays open, focus unchanged.
    expect(screen.getByRole('menu')).toBeVisible()
    expect(markdown).toHaveFocus()
  })

  test('closes when clicking outside and on Escape via document', async () => {
    const user = userEvent.setup()
    renderMenu()
    await user.click(screen.getByTestId('export-trigger'))
    expect(screen.getByRole('menu')).toBeVisible()
    // Outside pointer-down closes it.
    fireEvent.mouseDown(document.body)
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())

    // Re-open, then a document-level Escape (not focused on an item) closes it.
    await user.click(screen.getByTestId('export-trigger'))
    fireEvent.keyDown(document, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
  })

  test('toggling the trigger a second time closes the menu', async () => {
    const user = userEvent.setup()
    renderMenu()
    const trigger = screen.getByTestId('export-trigger')
    await user.click(trigger)
    expect(screen.getByRole('menu')).toBeVisible()
    await user.click(trigger)
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
  })

  test('shows the exporting label while a slow export is in flight', async () => {
    const user = userEvent.setup()
    let resolve!: (value: boolean) => void
    const onExport = vi.fn(() => new Promise<boolean>((r) => (resolve = r)))
    renderMenu({ onExport })

    await user.click(screen.getByTestId('export-trigger'))
    await user.click(screen.getByTestId('export-markdown'))
    // Trigger reflects the in-flight state and is disabled.
    await waitFor(() =>
      expect(screen.getByTestId('export-trigger')).toHaveTextContent(
        'Exporting…',
      ),
    )
    expect(screen.getByTestId('export-trigger')).toBeDisabled()

    resolve(true)
    await waitFor(() =>
      expect(screen.getByTestId('export-trigger')).toHaveTextContent('Export'),
    )
  })
})
