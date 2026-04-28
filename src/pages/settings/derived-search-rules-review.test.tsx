/**
 * @file derived-search-rules-review.test.tsx
 * @description Render-level coverage for Settings search-rule review actions.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify custom rule edit/delete and editor patch callbacks.
 * - Protect disabled/action states without mounting the full Settings route.
 *
 * ## Not responsible for
 * - Re-testing persistence in the route-owned derived-state hook.
 * - Re-testing built-in search rule generation.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider and typed Core Intelligence search-rule contracts.
 *
 * ## Performance notes
 * - Renders one bounded panel with fixed data, so strict coverage remains quick.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type {
  SearchEngineRule,
  SearchEngineRuleInput,
} from '../../lib/core-intelligence/types'
import { DerivedSearchRulesReview } from './derived-search-rules-review'

const builtInRule: SearchEngineRule = {
  ruleId: 'builtin-google',
  engineId: 'google',
  displayName: 'Google',
  hostPattern: 'google.com',
  pathPrefix: '/search',
  queryParamKey: 'q',
  enabled: true,
  note: null,
  exampleUrl: null,
  builtIn: true,
}

const builtInRuleWithoutPath: SearchEngineRule = {
  ...builtInRule,
  displayName: 'Site Search',
  engineId: 'site',
  hostPattern: 'search.example.com',
  pathPrefix: null,
  ruleId: 'builtin-site',
}

const customRule: SearchEngineRule = {
  ruleId: 'custom-docs',
  engineId: 'docs',
  displayName: 'Docs Search',
  hostPattern: 'docs.example.com',
  pathPrefix: null,
  queryParamKey: 'query',
  enabled: false,
  note: 'Team docs search',
  exampleUrl: 'https://docs.example.com/search?query=pathkeep',
  builtIn: false,
}

const draft: SearchEngineRuleInput = {
  ruleId: 'custom-docs',
  engineId: 'docs',
  displayName: 'Docs Search',
  hostPattern: 'docs.example.com',
  pathPrefix: null,
  queryParamKey: 'query',
  enabled: false,
  note: '',
  exampleUrl: '',
}

describe('DerivedSearchRulesReview', () => {
  test('edits custom rules and forwards every editor patch', async () => {
    const user = userEvent.setup()
    const onCancelSearchEngineRuleEdit = vi.fn()
    const onDeleteSearchEngineRule = vi.fn().mockResolvedValue(undefined)
    const onEditSearchEngineRule = vi.fn()
    const onSaveSearchEngineRule = vi.fn().mockResolvedValue(undefined)
    const onSearchEngineRuleDraftChange = vi.fn()
    const onStartSearchEngineRule = vi.fn()

    render(
      <I18nProvider>
        <DerivedSearchRulesReview
          action={null}
          searchEngineRuleDraft={draft}
          searchEngineRuleDraftValid
          searchEngineRuleError={null}
          searchEngineRules={[builtInRule, builtInRuleWithoutPath, customRule]}
          searchEngineRulesLoading={false}
          onCancelSearchEngineRuleEdit={onCancelSearchEngineRuleEdit}
          onDeleteSearchEngineRule={onDeleteSearchEngineRule}
          onEditSearchEngineRule={onEditSearchEngineRule}
          onSaveSearchEngineRule={onSaveSearchEngineRule}
          onSearchEngineRuleDraftChange={onSearchEngineRuleDraftChange}
          onStartSearchEngineRule={onStartSearchEngineRule}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('Team docs search')).toBeVisible()
    expect(screen.getByText('Not available')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Add custom rule' }))
    await user.click(screen.getByRole('button', { name: 'Edit rule' }))
    await user.click(screen.getByRole('button', { name: 'Delete rule' }))

    expect(onStartSearchEngineRule).toHaveBeenCalledTimes(1)
    expect(onEditSearchEngineRule).toHaveBeenCalledWith(customRule)
    expect(onDeleteSearchEngineRule).toHaveBeenCalledWith('custom-docs')

    fireEvent.change(screen.getByLabelText('Example URL'), {
      target: { value: 'https://docs.test' },
    })
    await user.click(screen.getByLabelText('Enabled'))
    fireEvent.change(screen.getByLabelText('Notes'), {
      target: { value: 'Use for docs' },
    })
    await user.click(screen.getByRole('button', { name: 'Save rule' }))
    await user.click(screen.getByRole('button', { name: 'common.cancel' }))

    expect(onSearchEngineRuleDraftChange).toHaveBeenCalledWith({
      exampleUrl: expect.stringContaining('https://docs.test'),
    })
    expect(onSearchEngineRuleDraftChange).toHaveBeenCalledWith({
      enabled: true,
    })
    expect(onSearchEngineRuleDraftChange).toHaveBeenCalledWith({
      note: expect.stringContaining('Use for docs'),
    })
    expect(onSaveSearchEngineRule).toHaveBeenCalledTimes(1)
    expect(onCancelSearchEngineRuleEdit).toHaveBeenCalledTimes(1)
  })

  test('renders empty draft optional fields and forwards path/query patches', () => {
    const onSearchEngineRuleDraftChange = vi.fn()
    render(
      <I18nProvider>
        <DerivedSearchRulesReview
          action={null}
          searchEngineRuleDraft={{
            ...draft,
            exampleUrl: null,
            note: null,
            pathPrefix: null,
          }}
          searchEngineRuleDraftValid
          searchEngineRuleError={null}
          searchEngineRules={[builtInRuleWithoutPath]}
          searchEngineRulesLoading={false}
          onCancelSearchEngineRuleEdit={vi.fn()}
          onDeleteSearchEngineRule={vi.fn()}
          onEditSearchEngineRule={vi.fn()}
          onSaveSearchEngineRule={vi.fn()}
          onSearchEngineRuleDraftChange={onSearchEngineRuleDraftChange}
          onStartSearchEngineRule={vi.fn()}
        />
      </I18nProvider>,
    )

    fireEvent.change(screen.getByLabelText('Path prefix'), {
      target: { value: '/find' },
    })
    fireEvent.change(screen.getByLabelText('Query param'), {
      target: { value: 'term' },
    })

    expect(onSearchEngineRuleDraftChange).toHaveBeenCalledWith({
      pathPrefix: expect.stringContaining('/find'),
    })
    expect(onSearchEngineRuleDraftChange).toHaveBeenCalledWith({
      queryParamKey: expect.stringContaining('term'),
    })
  })

  test('shows loading/error/empty states and disables blocked actions', () => {
    const onStartSearchEngineRule = vi.fn()
    render(
      <I18nProvider>
        <DerivedSearchRulesReview
          action="saving-search-rule"
          searchEngineRuleDraft={draft}
          searchEngineRuleDraftValid={false}
          searchEngineRuleError="Rule host is invalid"
          searchEngineRules={[builtInRule]}
          searchEngineRulesLoading
          onCancelSearchEngineRuleEdit={vi.fn()}
          onDeleteSearchEngineRule={vi.fn()}
          onEditSearchEngineRule={vi.fn()}
          onSaveSearchEngineRule={vi.fn()}
          onSearchEngineRuleDraftChange={vi.fn()}
          onStartSearchEngineRule={onStartSearchEngineRule}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('Rule host is invalid')).toBeVisible()
    expect(screen.getByText('No custom search rules yet.')).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'Add custom rule' }),
    ).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Save rule' })).toBeDisabled()
    expect(
      within(screen.getByTestId('settings-search-rule-editor')).getByText(
        'Editing',
      ),
    ).toBeVisible()
  })
})
