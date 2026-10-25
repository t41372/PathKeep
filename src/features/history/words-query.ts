/**
 * Writes the Advanced search form's "Words" fields as a query in the syntax
 * the backend parses (`vault-core/src/archive/search_query.rs` for the
 * operators, `vault-core/src/search/query.rs` for how `OR` groups words).
 *
 * Responsible for: turning each field into words and operators and quoting
 * what needs quoting. `OR` joins the word or phrase on each side of it and
 * everything else is AND, as in Google, so "any of these" is one run of
 * words joined by `OR`.
 * Not responsible for: reading a typed query back into the form (the form
 * only writes queries, like Google's Advanced Search page); deciding what a
 * query finds.
 *
 * Ways this can go wrong, and what guards each:
 * 1. Something written between two "any" words (an operator, a phrase)
 *    breaks the chain, since `OR` only joins its two neighbours: the "any"
 *    words are written together, with nothing in between.
 * 2. A value with spaces after an operator (`intitle:release notes`) reads
 *    as an operator and a stray word. Values with spaces are quoted right
 *    after the colon (`intitle:"release notes"`), which the parser accepts.
 * 3. A quote or backslash inside a quoted value ends the quote early. They
 *    are escaped with a backslash, the only escape the tokenizer knows; so
 *    are the curly quotes it also treats as quotes.
 * 4. A plain word that is syntax: a typed `OR` would join its neighbours,
 *    so it is quoted.
 *    (Operator-looking words such as `site:x` stay operators even quoted; a
 *    field that receives one passes it on, and the preview shows it.)
 * 5. Empty or blank fields leaving `site:` or `""` behind: blank values
 *    write nothing.
 * 6. A phrase typed in a word field ("release notes" in quotes) split into
 *    two words: quoted runs stay one term, as the parser reads them.
 * 7. A query that starts and ends with a slash (`/docs/`) read as a regex:
 *    the search box treats `/…/` that way, so the first word is quoted.
 * 8. A minus typed in "all" or "any" (`-foo`) read as an exclusion: in "any"
 *    it would also break the `OR` chain and make the other word required.
 *    Those fields mean the word; the minus is dropped, as "none" drops it.
 */
import { tagQuery } from './tags'

export interface WordsForm {
  /** Every word must appear. */
  all: string
  /** These words together, in this order. */
  phrase: string
  /** At least one of these words. */
  any: string
  /** None of these words. */
  none: string
  site: string
  /** Words in the page title. */
  title: string
  /** Words in the address. */
  url: string
  fileType: string
  /** Tags, separated by commas; a page needs all of them. */
  tags: string
  /** Text the page's note contains. */
  note: string
  starred: boolean
}

export const emptyWordsForm: WordsForm = {
  all: '',
  phrase: '',
  any: '',
  none: '',
  site: '',
  title: '',
  url: '',
  fileType: '',
  tags: '',
  note: '',
  starred: false,
}

interface Term {
  value: string
  quoted: boolean
}

/** Splits a field into terms the way the query tokenizer does: by spaces, keeping quoted runs whole. */
export function fieldTerms(text: string): Term[] {
  const terms: Term[] = []
  for (const match of text.matchAll(/["“”]([^"“”]*)["“”]?|[^\s"“”]+/g)) {
    const quoted = match[1] !== undefined
    const value = (quoted ? match[1] : match[0]).trim()
    if (value) terms.push({ value, quoted })
  }
  return terms
}

/** `"value"` with the characters that would end the quote escaped. */
function quote(value: string) {
  return `"${value.replace(/["“”\\]/g, (char) => `\\${char}`)}"`
}

/** A value the parser reads back as one token. */
function token(value: string) {
  return /[\s"“”\\]/.test(value) ? quote(value) : value
}

function word(term: Term) {
  if (term.quoted) return quote(term.value)
  // A bare OR splits groups; quoted, it is just a word.
  return term.value.toUpperCase() === 'OR'
    ? quote(term.value)
    : token(term.value)
}

function operator(name: string, value: string) {
  return `${name}:${token(value)}`
}

/** The query the form describes, or an empty string when every field is blank. */
export function composeWordsQuery(form: WordsForm): string {
  const phrase = form.phrase.trim()
  // A minus typed in any of the word fields: "none" already means it, and in
  // "all" or "any" it would turn the word into an exclusion.
  const unsigned = (field: string) =>
    fieldTerms(field)
      .map((term) => ({ ...term, value: term.value.replace(/^-+/, '') }))
      .filter((term) => term.value !== '')
  const parts = [
    ...unsigned(form.all).map(word),
    unsigned(form.any).map(word).join(' OR '),
    phrase && quote(phrase),
    ...unsigned(form.none).map((term) => `-${word(term)}`),
    form.site.trim() && operator('site', form.site.trim()),
    ...fieldTerms(form.title).map((term) => operator('intitle', term.value)),
    ...fieldTerms(form.url).map((term) => operator('inurl', term.value)),
    form.fileType.trim() &&
      operator('filetype', form.fileType.trim().replace(/^\.+/, '')),
    ...form.tags
      .split(',')
      .map((tag) => tag.trim())
      .filter(Boolean)
      .map(tagQuery),
    form.note.trim() && operator('note', form.note.trim()),
    form.starred && 'is:starred',
  ]
  const query = parts.filter(Boolean).join(' ')
  if (!/^\/.+\/$/s.test(query)) return query
  const [first, ...rest] = query.split(' ')
  return [quote(first), ...rest].join(' ')
}

/** Whether a typed query is only plain words, so the form can start from it. */
export function isPlainWords(query: string) {
  return (
    query.trim() !== '' &&
    !/["“”:\\]|(^|\s)-|(^|\s)OR(\s|$)/i.test(query) &&
    !/^\/.*\/$/s.test(query.trim())
  )
}
