/**
 * Tests for PaperFilterStrip — the paper Browse chip strip + add-filter
 * popover. Pins the rendered chips, the remove / clear-all paths, and the
 * popover's apply form (the only path that re-emits URL state).
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperFilterStrip,
  type PaperFilterStripCopy,
  type PaperFilterStripFormState,
} from './paper-filter-strip'

const COPY: PaperFilterStripCopy = {
  addFilter: '+ Filter',
  clearAll: 'Clear all',
  emptyHint: 'No filters · viewing everything',
  removeFilterAria: 'Remove filter {label}: {value}',
  popoverTitle: 'Refine the view',
  fieldDomain: 'Domain contains',
  fieldBrowser: 'Browser',
  fieldProfile: 'Profile',
  fieldStart: 'From',
  fieldEnd: 'To',
  fieldRegex: 'Regex search',
  selectAllBrowsers: 'Any browser',
  selectAllProfiles: 'Any profile',
  applyLabel: 'Apply',
  closeLabel: 'Close filter form',
}

const EMPTY_FORM: PaperFilterStripFormState = {
  domain: '',
  browserKind: '',
  profileId: '',
  start: '',
  end: '',
  regexMode: false,
}

describe('PaperFilterStrip', () => {
  test('shows the empty hint when no chips are active', () => {
    render(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={() => {}}
      />,
    )
    expect(screen.getByText('No filters · viewing everything')).toBeVisible()
    expect(screen.queryByText('Clear all')).toBeNull()
  })

  test('renders one chip per active filter and fires onRemove with the chip id', () => {
    const onRemove = vi.fn()
    render(
      <PaperFilterStrip
        chips={[
          { id: 'domain', label: 'Domain', value: 'github.com' },
          { id: 'profileId', label: 'Profile', value: 'Chrome · Default' },
        ]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[]}
        profileOptions={[]}
        onRemove={onRemove}
        onClearAll={() => {}}
        onApply={() => {}}
      />,
    )
    fireEvent.click(screen.getByLabelText('Remove filter Domain: github.com'))
    expect(onRemove).toHaveBeenCalledWith('domain')
  })

  test('shows Clear all only when at least one chip is active and fires onClearAll', () => {
    const onClearAll = vi.fn()
    render(
      <PaperFilterStrip
        chips={[{ id: 'domain', label: 'Domain', value: 'github.com' }]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={onClearAll}
        onApply={() => {}}
      />,
    )
    fireEvent.click(screen.getByText('Clear all'))
    expect(onClearAll).toHaveBeenCalledTimes(1)
  })

  test('opens the popover, edits the domain, and fires onApply with the diff', () => {
    const onApply = vi.fn()
    render(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[
          { value: 'chrome', label: 'Chrome' },
          { value: 'firefox', label: 'Firefox' },
        ]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={onApply}
      />,
    )
    fireEvent.click(screen.getByText('+ Filter'))
    expect(screen.getByText('Refine the view')).toBeVisible()
    fireEvent.change(screen.getByTestId('paper-filter-input-domain'), {
      target: { value: 'github.com' },
    })
    fireEvent.click(screen.getByTestId('paper-filter-apply'))
    expect(onApply).toHaveBeenCalledTimes(1)
    expect(onApply).toHaveBeenCalledWith(
      expect.objectContaining({ domain: 'github.com' }),
    )
    // Popover closes after apply.
    expect(screen.queryByText('Refine the view')).toBeNull()
  })

  test('omits browser / profile selects when only zero or one option is available', () => {
    render(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[{ value: 'chrome', label: 'Chrome' }]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={() => {}}
      />,
    )
    fireEvent.click(screen.getByText('+ Filter'))
    expect(screen.queryByTestId('paper-filter-input-browser')).toBeNull()
    expect(screen.queryByTestId('paper-filter-input-profile')).toBeNull()
  })

  test('edits every popover field and ships the diff to onApply', () => {
    const onApply = vi.fn()
    render(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[
          { value: 'chrome', label: 'Chrome' },
          { value: 'firefox', label: 'Firefox' },
        ]}
        profileOptions={[
          { value: 'chrome:Default', label: 'Chrome · Default' },
          { value: 'firefox:dev', label: 'Firefox · dev' },
        ]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={onApply}
      />,
    )
    fireEvent.click(screen.getByText('+ Filter'))
    fireEvent.change(screen.getByTestId('paper-filter-input-browser'), {
      target: { value: 'firefox' },
    })
    fireEvent.change(screen.getByTestId('paper-filter-input-profile'), {
      target: { value: 'firefox:dev' },
    })
    fireEvent.change(screen.getByTestId('paper-filter-input-start'), {
      target: { value: '2026-05-01' },
    })
    fireEvent.change(screen.getByTestId('paper-filter-input-end'), {
      target: { value: '2026-05-22' },
    })
    fireEvent.click(screen.getByTestId('paper-filter-input-regex'))
    fireEvent.click(screen.getByTestId('paper-filter-apply'))
    expect(onApply).toHaveBeenCalledWith({
      domain: '',
      browserKind: 'firefox',
      profileId: 'firefox:dev',
      start: '2026-05-01',
      end: '2026-05-22',
      regexMode: true,
    })
  })

  test('closes the popover via the close button, Escape, and outside click', () => {
    render(
      <>
        <div data-testid="outside" />
        <PaperFilterStrip
          chips={[]}
          copy={COPY}
          formState={EMPTY_FORM}
          browserOptions={[]}
          profileOptions={[]}
          onRemove={() => {}}
          onClearAll={() => {}}
          onApply={() => {}}
        />
      </>,
    )
    // 1. Close button.
    fireEvent.click(screen.getByText('+ Filter'))
    fireEvent.click(screen.getByLabelText('Close filter form'))
    expect(screen.queryByText('Refine the view')).toBeNull()
    // 2. Escape key.
    fireEvent.click(screen.getByText('+ Filter'))
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByText('Refine the view')).toBeNull()
    // 3. Outside click.
    fireEvent.click(screen.getByText('+ Filter'))
    fireEvent.mouseDown(screen.getByTestId('outside'))
    expect(screen.queryByText('Refine the view')).toBeNull()
  })

  test('ignores non-Escape keys while the popover is open', () => {
    render(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={() => {}}
      />,
    )
    fireEvent.click(screen.getByText('+ Filter'))
    fireEvent.keyDown(document, { key: 'Tab' })
    fireEvent.keyDown(document, { key: 'a' })
    expect(screen.getByText('Refine the view')).toBeVisible()
  })

  test('keeps the popover open when the user clicks inside the form', () => {
    render(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={() => {}}
      />,
    )
    fireEvent.click(screen.getByText('+ Filter'))
    fireEvent.mouseDown(screen.getByTestId('paper-filter-input-domain'))
    expect(screen.getByText('Refine the view')).toBeVisible()
  })

  test('honours an explicit testId across chip, trigger, popover, and form inputs', () => {
    render(
      <PaperFilterStrip
        chips={[{ id: 'domain', label: 'Domain', value: 'github.com' }]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[
          { value: 'chrome', label: 'Chrome' },
          { value: 'firefox', label: 'Firefox' },
        ]}
        profileOptions={[
          { value: 'chrome:Default', label: 'Chrome · Default' },
          { value: 'firefox:dev', label: 'Firefox · dev' },
        ]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={() => {}}
        testId="my-strip"
      />,
    )
    expect(screen.getByTestId('my-strip')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-chip-domain')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-add')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-clear-all')).toBeInTheDocument()
    fireEvent.click(screen.getByTestId('my-strip-add'))
    expect(screen.getByTestId('my-strip-popover')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-input-domain')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-input-browser')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-input-profile')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-input-start')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-input-end')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-input-regex')).toBeInTheDocument()
    expect(screen.getByTestId('my-strip-apply')).toBeInTheDocument()
  })

  test('syncs the draft from formState while the popover is closed but protects the user from a mid-edit URL clobber while open', () => {
    // Regression coverage for the popover-clobber bug: a chip removal (or
    // any other parent-driven URL update) fires while the user is still
    // typing inside the popover form. Re-syncing the draft on every
    // formState change would silently throw away the in-flight edit; the
    // contract is to only sync while the popover is closed, then reseed
    // on the closed→open transition.
    const { rerender } = render(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={EMPTY_FORM}
        browserOptions={[]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={() => {}}
      />,
    )

    // While closed: formState change must sync into the draft so opening
    // the popover surfaces the latest URL-derived values.
    rerender(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={{ ...EMPTY_FORM, domain: 'github.com' }}
        browserOptions={[]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={() => {}}
      />,
    )
    fireEvent.click(screen.getByText('+ Filter'))
    expect(
      screen.getByTestId<HTMLInputElement>('paper-filter-input-domain').value,
    ).toBe('github.com')

    // User starts typing inside the popover…
    fireEvent.change(screen.getByTestId('paper-filter-input-domain'), {
      target: { value: 'github.com/issues' },
    })
    // …then the parent re-renders with a new formState (chip removal, etc).
    // The in-flight draft must NOT be clobbered.
    rerender(
      <PaperFilterStrip
        chips={[]}
        copy={COPY}
        formState={{ ...EMPTY_FORM, domain: '' }}
        browserOptions={[]}
        profileOptions={[]}
        onRemove={() => {}}
        onClearAll={() => {}}
        onApply={() => {}}
      />,
    )
    expect(
      screen.getByTestId<HTMLInputElement>('paper-filter-input-domain').value,
    ).toBe('github.com/issues')
  })
})
