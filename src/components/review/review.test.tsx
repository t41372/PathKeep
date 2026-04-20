import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import {
  copyReviewValue,
  GeneratedArtifactViewer,
  PmeTabBar,
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
    render(
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

    expect(screen.getByText('alpha')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'b.txt' }))
    expect(screen.getByText('beta')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Open path' }))
    expect(onOpenPath).toHaveBeenCalledWith('/tmp/b.txt')
    await user.click(screen.getByRole('button', { name: 'Copy path' }))
    expect(onCopy).toHaveBeenCalledWith('path:b.txt', '/tmp/b.txt')
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
