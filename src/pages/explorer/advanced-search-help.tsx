/**
 * @file advanced-search-help.tsx
 * @description Render-only hover/focus help for Explorer's local advanced keyword syntax.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Expose the supported Google-like keyword operators next to the Explorer search input.
 * - Keep examples short enough for the dense Explorer workbench header.
 * - Provide both hover and keyboard-focus affordances through semantic tooltip markup.
 *
 * ## Not responsible for
 * - Parsing or validating advanced search syntax.
 * - Owning regex-mode behavior or backend search execution.
 * - Teaching unsupported Google web-index features.
 *
 * ## Dependencies
 * - Depends only on the Explorer namespace translator and route-level CSS.
 *
 * ## Performance notes
 * - Static render-only content; no state or effects on the search hot path.
 */

import { useId } from 'react'
import type { Translator } from './types'

const ADVANCED_SEARCH_EXAMPLES = [
  {
    code: 'site:github.com -pathkeep',
    labelKey: 'advancedSearchHelpSiteExclude',
  },
  {
    code: '"release notes"',
    labelKey: 'advancedSearchHelpExactPhrase',
  },
  {
    code: 'manual OR youtube',
    labelKey: 'advancedSearchHelpOr',
  },
  {
    code: 'intitle:manual inurl:docs',
    labelKey: 'advancedSearchHelpField',
  },
  {
    code: 'filetype:pdf after:2026-05-01 before:2026-05-07',
    labelKey: 'advancedSearchHelpFileDate',
  },
] as const

interface AdvancedSearchHelpProps {
  explorerT: Translator
}

/**
 * Renders the advanced keyword syntax hover card.
 *
 * The panel is always present in the DOM for `aria-describedby`, while CSS
 * controls hover/focus visibility so the component does not add event work to
 * every Explorer keystroke.
 */
export function AdvancedSearchHelp({ explorerT }: AdvancedSearchHelpProps) {
  const panelId = useId()

  return (
    <span className="advanced-search-help">
      <button
        aria-describedby={panelId}
        aria-label={explorerT('advancedSearchHelpAria')}
        className="advanced-search-help__trigger"
        type="button"
      >
        ?
      </button>
      <span className="advanced-search-help__panel" id={panelId} role="tooltip">
        <span className="advanced-search-help__title">
          {explorerT('advancedSearchHelpTitle')}
        </span>
        <span className="advanced-search-help__body">
          {explorerT('advancedSearchHelpIntro')}
        </span>
        <span className="advanced-search-help__examples">
          {ADVANCED_SEARCH_EXAMPLES.map((example) => (
            <span className="advanced-search-help__example" key={example.code}>
              <code>{example.code}</code>
              <span>{explorerT(example.labelKey)}</span>
            </span>
          ))}
        </span>
        <span className="advanced-search-help__note">
          {explorerT('advancedSearchHelpRegexNote')}
        </span>
      </span>
    </span>
  )
}
