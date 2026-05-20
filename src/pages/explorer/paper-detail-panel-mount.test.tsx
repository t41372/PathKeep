import { describe, expect, test, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import type { HistoryEntry } from '@/lib/types/archive'
import { PaperDetailPanelMount } from './paper-detail-panel-mount'
import type { LocalAnnotations } from './use-local-annotations'

function explorerT(key: string, vars?: Record<string, string | number>) {
  return vars ? `${key}:${JSON.stringify(vars)}` : key
}

function makeEntry(over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: 7,
    profileId: 'chrome:Default',
    url: 'https://example.com/x',
    title: 'Example X',
    domain: 'example.com',
    favicon: null,
    visitedAt: '2026-05-17T10:30:00',
    visitTime: 1747488600,
    durationMs: null,
    transition: 0,
    sourceVisitId: 0,
    appId: null,
    ...over,
  }
}

function emptyAnnotations(
  over: Partial<LocalAnnotations> = {},
): LocalAnnotations {
  return {
    notesFor: () => '',
    tagsFor: () => [],
    updateNotes: () => {},
    updateTags: () => {},
    ...over,
  }
}

describe('PaperDetailPanelMount', () => {
  test('renders nothing when selectedEntry is null', () => {
    const { container } = render(
      <PaperDetailPanelMount
        selectedEntry={null}
        annotations={emptyAnnotations()}
        explorerT={explorerT}
        onClose={() => {}}
        onOpen={() => {}}
      />,
    )
    expect(container.firstChild).toBeNull()
  })

  test('renders the panel and shows the entry title when selectedEntry is set', () => {
    render(
      <PaperDetailPanelMount
        selectedEntry={makeEntry({ title: 'Reading list' })}
        annotations={emptyAnnotations()}
        explorerT={explorerT}
        onClose={() => {}}
        onOpen={() => {}}
      />,
    )
    expect(
      screen.getByTestId('explorer-paper-detail-panel'),
    ).toBeInTheDocument()
    expect(screen.getByText('Reading list')).toBeInTheDocument()
  })

  test('falls back to the URL when the title is null', () => {
    render(
      <PaperDetailPanelMount
        selectedEntry={makeEntry({ title: null })}
        annotations={emptyAnnotations()}
        explorerT={explorerT}
        onClose={() => {}}
        onOpen={() => {}}
      />,
    )
    // The URL appears both as the title-fallback heading and as the mono
    // URL line — sanitizeExplorerDisplayText strips the protocol so the
    // rendered text is 'example.com/x'.
    expect(
      screen.getAllByText('example.com/x').length,
    ).toBeGreaterThanOrEqual(1)
  })

  test('clicking the close button forwards onClose', () => {
    const onClose = vi.fn()
    render(
      <PaperDetailPanelMount
        selectedEntry={makeEntry()}
        annotations={emptyAnnotations()}
        explorerT={explorerT}
        onClose={onClose}
        onOpen={() => {}}
      />,
    )
    const closeButtons = screen.getAllByRole('button', {
      name: 'paperBrowse.detailClose',
    })
    fireEvent.click(closeButtons[0])
    expect(onClose).toHaveBeenCalled()
  })

  test('Open action forwards the page URL (not the entry id)', () => {
    const onOpen = vi.fn()
    render(
      <PaperDetailPanelMount
        selectedEntry={makeEntry({ id: 42, url: 'https://example.com/x' })}
        annotations={emptyAnnotations()}
        explorerT={explorerT}
        onClose={() => {}}
        onOpen={onOpen}
      />,
    )
    fireEvent.click(
      screen.getByRole('button', { name: 'paperBrowse.detailActionOpen' }),
    )
    // The route hands this argument to handleVisit → openExternalUrl,
    // so it must be the URL string. Passing the numeric id would try to
    // open "42" as a URL.
    expect(onOpen).toHaveBeenCalledWith('https://example.com/x')
  })

  test('Copy URL action forwards through onCopyUrl when provided', () => {
    const onCopyUrl = vi.fn()
    render(
      <PaperDetailPanelMount
        selectedEntry={makeEntry()}
        annotations={emptyAnnotations()}
        explorerT={explorerT}
        onClose={() => {}}
        onOpen={() => {}}
        onCopyUrl={onCopyUrl}
      />,
    )
    fireEvent.click(
      screen.getByRole('button', { name: 'paperBrowse.detailActionCopy' }),
    )
    expect(onCopyUrl).toHaveBeenCalledWith('https://example.com/x')
  })

  test('Copy URL action falls back to navigator.clipboard.writeText when no handler is supplied', () => {
    const writeText = vi.fn()
    const originalClipboard = globalThis.navigator?.clipboard
    Object.defineProperty(globalThis.navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    })
    try {
      render(
        <PaperDetailPanelMount
          selectedEntry={makeEntry()}
          annotations={emptyAnnotations()}
          explorerT={explorerT}
          onClose={() => {}}
          onOpen={() => {}}
        />,
      )
      fireEvent.click(
        screen.getByRole('button', { name: 'paperBrowse.detailActionCopy' }),
      )
      expect(writeText).toHaveBeenCalledWith('https://example.com/x')
    } finally {
      Object.defineProperty(globalThis.navigator, 'clipboard', {
        value: originalClipboard,
        configurable: true,
      })
    }
  })

  test('reads notes through the annotation store', () => {
    const notesFor = vi.fn(() => 'remembered')
    render(
      <PaperDetailPanelMount
        selectedEntry={makeEntry()}
        annotations={emptyAnnotations({ notesFor })}
        explorerT={explorerT}
        onClose={() => {}}
        onOpen={() => {}}
      />,
    )
    expect(notesFor).toHaveBeenCalledWith('https://example.com/x')
  })

  test('writes notes back to the annotation store after the debounce fires', async () => {
    vi.useFakeTimers()
    const updateNotes = vi.fn()
    try {
      render(
        <PaperDetailPanelMount
          selectedEntry={makeEntry()}
          annotations={emptyAnnotations({ updateNotes })}
          explorerT={explorerT}
          onClose={() => {}}
          onOpen={() => {}}
        />,
      )
      const textarea = screen.getByPlaceholderText(
        'paperBrowse.detailNotesPlaceholder',
      )
      fireEvent.change(textarea, { target: { value: 'new note' } })
      // PaperDetailPanel debounces note writes at 400 ms — advance past it.
      await vi.advanceTimersByTimeAsync(450)
      expect(updateNotes).toHaveBeenCalledWith(
        'https://example.com/x',
        'new note',
      )
    } finally {
      vi.useRealTimers()
    }
  })

  test('reads existing tags through the annotation store', () => {
    const tagsFor = vi.fn(() => ['rust', 'sqlite'])
    render(
      <PaperDetailPanelMount
        selectedEntry={makeEntry()}
        annotations={emptyAnnotations({ tagsFor })}
        explorerT={explorerT}
        onClose={() => {}}
        onOpen={() => {}}
      />,
    )
    expect(tagsFor).toHaveBeenCalledWith('https://example.com/x')
    expect(screen.getByText('rust')).toBeInTheDocument()
    expect(screen.getByText('sqlite')).toBeInTheDocument()
  })

  test('adding a tag in the input writes the merged list back via updateTags', () => {
    const updateTags = vi.fn()
    render(
      <PaperDetailPanelMount
        selectedEntry={makeEntry()}
        annotations={emptyAnnotations({
          tagsFor: () => ['rust'],
          updateTags,
        })}
        explorerT={explorerT}
        onClose={() => {}}
        onOpen={() => {}}
      />,
    )
    const tagInput = screen.getByPlaceholderText(
      'paperBrowse.detailTagInputPlaceholder',
    )
    fireEvent.change(tagInput, { target: { value: 'sqlite' } })
    fireEvent.keyDown(tagInput, { key: 'Enter' })
    expect(updateTags).toHaveBeenCalledWith('https://example.com/x', [
      'rust',
      'sqlite',
    ])
  })
})
