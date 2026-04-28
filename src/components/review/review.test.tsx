import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import {
  copyReviewValue,
  GeneratedArtifactViewer,
  PmeTabBar,
  ReviewRuntimeBoundaryCard,
  ReviewCodePreview,
  ReviewPathActionRow,
  ReviewTargetLinksRow,
  VerifyCheckList,
} from '.'

describe('shared review primitives', () => {
  test('renders shared code preview copy chrome', async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()

    render(
      <ReviewCodePreview
        code="echo pathkeep"
        copyFeedback={null}
        copyKey="command"
        copyLabel="Copy"
        errorMessage="Copy failed"
        onCopy={onCopy}
        successMessage="Copied"
        title="Command preview"
      />,
    )

    await user.click(screen.getByRole('button', { name: 'Copy' }))
    expect(onCopy).toHaveBeenCalledWith('command', 'echo pathkeep')
  })

  test('renders shared target links with primary and secondary destinations', () => {
    const { container, rerender } = render(
      <MemoryRouter>
        <ReviewTargetLinksRow
          label="Open"
          primaryHref="/intelligence/day/2026-04-18?profileId=chrome%3ADefault"
          primaryLabel="Open insights"
          secondaryLinks={[
            {
              href: '/intelligence/domain/sqlite.org?range=custom&start=2026-04-18&end=2026-04-18&profileId=chrome%3ADefault',
              key: 'domain',
              label: 'sqlite.org',
            },
          ]}
        />
      </MemoryRouter>,
    )

    expect(screen.getByRole('link', { name: 'Open insights' })).toHaveAttribute(
      'href',
      '/intelligence/day/2026-04-18?profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: 'sqlite.org' })).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-18&end=2026-04-18&profileId=chrome%3ADefault',
    )

    rerender(
      <MemoryRouter>
        <ReviewTargetLinksRow label="Open" fallback="No target available" />
      </MemoryRouter>,
    )
    expect(screen.getByText('No target available')).toBeVisible()

    rerender(
      <MemoryRouter>
        <ReviewTargetLinksRow label="Open" fallback={null} />
      </MemoryRouter>,
    )
    expect(container).toBeEmptyDOMElement()

    rerender(
      <MemoryRouter>
        <ReviewTargetLinksRow label="Open" primaryHref="/jobs" />
      </MemoryRouter>,
    )
    expect(screen.getByText('Open')).toBeVisible()
    expect(screen.queryByRole('link')).toBeNull()
  })

  test('switches generated artifact tabs and delegates copy/open actions', async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()
    const onOpenPath = vi.fn()

    render(
      <GeneratedArtifactViewer
        copyFeedback={null}
        copyLabel="Copy"
        copyPathLabel="Copy path"
        errorMessage="Copy failed"
        files={[
          {
            absolutePath: '/tmp/a.txt',
            contents: 'alpha',
            purpose: 'Alpha file',
            relativePath: 'a.txt',
          },
          {
            absolutePath: '/tmp/b.txt',
            contents: 'beta',
            purpose: 'Beta file',
            relativePath: 'b.txt',
          },
        ]}
        onCopy={onCopy}
        onOpenPath={onOpenPath}
        openPathLabel="Open path"
        successMessage="Copied"
      />,
    )

    expect(screen.getByText('alpha')).not.toBeVisible()
    const alphaSummary = screen
      .getAllByText('Alpha file')
      .find((node) => node.tagName.toLowerCase() === 'summary')
    expect(alphaSummary?.closest('details')).not.toHaveAttribute('open')
    expect(screen.getByText('alpha').closest('.code-panel')).toBeInstanceOf(
      HTMLDetailsElement,
    )
    if (!(alphaSummary instanceof HTMLElement)) {
      throw new Error('Expected alpha summary to be present')
    }
    await user.click(alphaSummary)
    expect(screen.getByText('alpha')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'b.txt' }))
    expect(screen.getByText('beta')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Open path' }))
    expect(onOpenPath).toHaveBeenCalledWith('/tmp/b.txt')
    await user.click(screen.getByRole('button', { name: 'Copy path' }))
    expect(onCopy).toHaveBeenCalledWith('path:b.txt', '/tmp/b.txt')
  })

  test('hides the generated artifact viewer when no files are available', () => {
    const { container } = render(
      <GeneratedArtifactViewer
        copyFeedback={null}
        copyLabel="Copy"
        copyPathLabel="Copy path"
        errorMessage="Copy failed"
        files={[]}
        onCopy={vi.fn()}
        onOpenPath={vi.fn()}
        openPathLabel="Open path"
        successMessage="Copied"
      />,
    )

    expect(container).toBeEmptyDOMElement()
  })

  test('renders generated artifacts without absolute-path actions', async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()

    render(
      <GeneratedArtifactViewer
        copyFeedback={{ key: 'contents:notes.md', tone: 'error' }}
        copyLabel="Copy"
        copyPathLabel="Copy path"
        errorMessage="Copy failed"
        files={[
          {
            absolutePath: null,
            contents: 'inline notes',
            purpose: 'Notes',
            relativePath: 'notes.md',
          },
        ]}
        onCopy={onCopy}
        openPathLabel="Open path"
        successMessage="Copied"
      />,
    )

    await user.click(screen.getByRole('button', { name: 'Copy' }))
    expect(onCopy).toHaveBeenCalledWith('contents:notes.md', 'inline notes')
    expect(screen.queryByRole('button', { name: 'Copy path' })).toBeNull()
    expect(screen.queryByText('Open path')).toBeNull()
    expect(screen.getByText('Copy failed')).toBeVisible()
  })

  test('copies review values through the shared clipboard helper', async () => {
    const writeText = vi.fn(() => Promise.resolve(undefined))
    const originalClipboard = navigator.clipboard
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })

    const onFeedback = vi.fn()

    try {
      await expect(
        copyReviewValue('/tmp/pathkeep', {
          key: 'settings:app-root',
          onFeedback,
        }),
      ).resolves.toBe('success')
    } finally {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: originalClipboard,
      })
    }

    expect(writeText).toHaveBeenCalledWith('/tmp/pathkeep')
    expect(onFeedback).toHaveBeenCalledWith({
      key: 'settings:app-root',
      tone: 'success',
    })

    await expect(
      copyReviewValue('/tmp/default-key', {
        onFeedback,
      }),
    ).resolves.toBe('success')
    expect(onFeedback).toHaveBeenLastCalledWith({
      key: '/tmp/default-key',
      tone: 'success',
    })

    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: undefined,
    })

    try {
      await expect(copyReviewValue('/tmp/missing-clipboard')).resolves.toBe(
        'error',
      )
    } finally {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: originalClipboard,
      })
    }
  })

  test('renders shared path-action rows with open/copy chrome and feedback', async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()
    const onOpenPath = vi.fn()

    render(
      <ReviewPathActionRow
        copyFeedback={{ key: 'settings:app-root', tone: 'success' }}
        copyKey="settings:app-root"
        copyLabel="Copy"
        errorMessage="Copy failed"
        label="App root"
        onCopy={onCopy}
        onOpenPath={onOpenPath}
        openPathLabel="Open path"
        secondaryAction={<button type="button">Open audit</button>}
        status="Shared support actions stay reviewable."
        successMessage="Copied"
        value="/tmp/pathkeep"
      />,
    )

    await user.click(screen.getByRole('button', { name: 'Open path' }))
    expect(onOpenPath).toHaveBeenCalledWith('/tmp/pathkeep')
    await user.click(screen.getByRole('button', { name: 'Copy' }))
    expect(onCopy).toHaveBeenCalledWith('settings:app-root', '/tmp/pathkeep')
    expect(screen.getByText('Open audit')).toBeVisible()
    expect(
      screen.getByText('Shared support actions stay reviewable.'),
    ).toBeVisible()
    expect(screen.getByText('Copied')).toBeVisible()
  })

  test('renders shared path-action rows with copy-only fallback keys', async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()
    render(
      <ReviewPathActionRow
        copyFeedback={null}
        copyLabel="Copy"
        errorMessage="Copy failed"
        label="Manifest"
        onCopy={onCopy}
        openPathLabel="Open path"
        successMessage="Copied"
        value="/tmp/pathkeep/manifest.json"
      />,
    )

    expect(screen.getByText('/tmp/pathkeep/manifest.json')).toBeVisible()
    expect(screen.queryByRole('button', { name: 'Open path' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Copy' }))
    expect(onCopy).toHaveBeenCalledWith(
      'path:/tmp/pathkeep/manifest.json',
      '/tmp/pathkeep/manifest.json',
    )
  })

  test('renders shared path-action rows without optional actions', () => {
    render(
      <ReviewPathActionRow
        copyFeedback={null}
        copyLabel="Copy"
        errorMessage="Copy failed"
        label="Manifest"
        openPathLabel="Open path"
        successMessage="Copied"
        value="/tmp/pathkeep/manifest.json"
      />,
    )

    expect(screen.getByText('/tmp/pathkeep/manifest.json')).toBeVisible()
    expect(screen.queryByRole('button', { name: 'Open path' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Copy' })).toBeNull()
  })

  test('renders shared path-action rows with open-only actions', async () => {
    const user = userEvent.setup()
    const onOpenPath = vi.fn()

    render(
      <ReviewPathActionRow
        copyFeedback={null}
        copyLabel="Copy"
        errorMessage="Copy failed"
        label="Manifest"
        onOpenPath={onOpenPath}
        openPathLabel="Open path"
        successMessage="Copied"
        value="/tmp/pathkeep/manifest.json"
      />,
    )

    await user.click(screen.getByRole('button', { name: 'Open path' }))
    expect(onOpenPath).toHaveBeenCalledWith('/tmp/pathkeep/manifest.json')
    expect(screen.queryByRole('button', { name: 'Copy' })).toBeNull()
  })

  test('renders generated artifact copy-path actions without open handlers', async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()

    render(
      <GeneratedArtifactViewer
        copyFeedback={null}
        copyLabel="Copy"
        copyPathLabel="Copy path"
        errorMessage="Copy failed"
        files={[
          {
            absolutePath: '/tmp/manifest.json',
            contents: '{}',
            purpose: 'Manifest',
            relativePath: 'manifest.json',
          },
        ]}
        onCopy={onCopy}
        openPathLabel="Open path"
        successMessage="Copied"
      />,
    )

    expect(screen.queryByRole('button', { name: 'Open path' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Copy path' }))
    expect(onCopy).toHaveBeenCalledWith(
      'path:manifest.json',
      '/tmp/manifest.json',
    )
  })

  test('renders shared runtime-boundary cards with metrics, notes, and actions', async () => {
    const user = userEvent.setup()
    const onToggle = vi.fn()

    render(
      <ReviewRuntimeBoundaryCard
        active
        actions={
          <button type="button" onClick={onToggle}>
            Retry
          </button>
        }
        description="Shared runtime cards keep Jobs and Settings on the same review grammar."
        headerMeta={<span className="mono">Stale</span>}
        metrics={[
          {
            label: 'Derived tables',
            value: 'search_trails, query_families',
            valueClassName: 'mono',
          },
          {
            label: 'Last built',
            value: '2026-04-21 10:30',
            valueClassName: 'mono-support',
          },
        ]}
        notes={<p className="mono-support">Needs rebuild after new imports.</p>}
        title="Search trails"
      />,
    )

    expect(screen.getByText('Search trails')).toBeVisible()
    expect(screen.getByText('Derived tables')).toBeVisible()
    expect(screen.getByText('search_trails, query_families')).toBeVisible()
    expect(screen.getByText('Needs rebuild after new imports.')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Retry' }))
    expect(onToggle).toHaveBeenCalledTimes(1)
  })

  test('renders runtime-boundary cards with plain metric values only', () => {
    render(
      <ReviewRuntimeBoundaryCard
        metrics={[
          {
            label: 'Queue depth',
            value: 0,
          },
        ]}
        title="Queue"
      />,
    )

    expect(screen.getByText('Queue depth')).toBeVisible()
    expect(screen.getByText('0')).toHaveClass('config-value')
  })

  test('renders verify-result rows as shared review sections', () => {
    render(
      <VerifyCheckList
        items={[
          {
            body: 'Manifest checksums and required entries match.',
            key: 'manifest',
            label: 'Manifest integrity',
            status: 'success',
          },
          {
            body: 'Restore path opened cleanly.',
            key: 'restore',
            label: 'Restore readiness',
            status: 'ready',
          },
        ]}
      />,
    )

    expect(screen.getByText('Manifest integrity')).toBeVisible()
    expect(screen.getByText('Restore readiness')).toBeVisible()
    expect(
      screen.getByText('Manifest checksums and required entries match.'),
    ).toBeVisible()
  })

  test('renders no verify-result wrapper when there are no rows', () => {
    const { container } = render(<VerifyCheckList items={[]} />)

    expect(container).toBeEmptyDOMElement()
  })

  test('keeps PME tabs keyboard reachable and updates selection', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn()

    render(
      <PmeTabBar
        activeTab="preview"
        onChange={onChange}
        tabs={[
          { key: 'preview', label: 'Preview' },
          { key: 'manual', label: 'Manual' },
          { key: 'execute', label: 'Execute' },
          { key: 'verify', label: 'Verify' },
        ]}
      />,
    )

    await user.tab()
    expect(screen.getByRole('button', { name: 'Preview' })).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Verify' }))
    expect(onChange).toHaveBeenCalledWith('verify')
  })
})
