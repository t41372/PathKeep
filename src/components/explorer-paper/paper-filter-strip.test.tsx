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
})
