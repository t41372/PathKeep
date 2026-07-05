import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import {
  PaperAdvancedSearchHelp,
  type PaperAdvancedSearchHelpCopy,
} from './paper-advanced-search-help'

const COPY: PaperAdvancedSearchHelpCopy = {
  ariaLabel: 'Show advanced keyword syntax',
  title: 'Advanced keyword syntax',
  intro: 'Use operators to narrow the local archive.',
  siteExclude: 'Limit to a site and exclude a term.',
  exactPhrase: 'Match an exact phrase.',
  or: 'Match either term.',
  field: 'Search title and URL fields.',
  fileDate: 'Filter by file type and date.',
  tag: 'Filter by tags.',
  note: 'Filter by notes.',
  starred: 'Show only starred pages.',
  regexNote: 'Regex mode is separate.',
}

describe('PaperAdvancedSearchHelp', () => {
  test('uses the default test id and hides the popover until opened', () => {
    render(<PaperAdvancedSearchHelp copy={COPY} />)

    const wrapper = screen.getByTestId('paper-advanced-search-help')
    expect(wrapper).toBeVisible()
    expect(
      screen.queryByTestId('paper-advanced-search-help-panel'),
    ).not.toBeInTheDocument()

    fireEvent.mouseEnter(wrapper)
    expect(
      screen.getByTestId('paper-advanced-search-help-panel'),
    ).toHaveTextContent('Advanced keyword syntax')
  })
})
