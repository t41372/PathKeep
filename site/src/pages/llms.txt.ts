/**
 * `/llms.txt`: the page's facts as Markdown for language models, in the
 * shape llmstxt.org proposes. It is built from the English copy, so it
 * never says more than the page does; links become absolute once `site`
 * is set in `astro.config.mjs`.
 */
import type { APIRoute } from 'astro'
import { copy, langs, plain } from '../content/copy'

const repo = 'https://github.com/t41372/PathKeep'

export const GET: APIRoute = ({ site }) => {
  const c = copy.en
  const at = (path: string) => (site ? new URL(path, site).href : path)
  const list = (items: string[]) => items.map((i) => `- ${plain(i)}`)
  const lines = [
    '# PathKeep',
    '',
    `> ${plain(c.description)}`,
    '',
    ...(plain(c.hero.lede) === plain(c.description)
      ? []
      : [plain(c.hero.lede), '']),
    plain(c.hero.note),
    '',
    `## ${plain(c.why.title)}`,
    '',
    plain(c.why.body),
    '',
    ...list(c.why.rows.map((r) => `${r.source}: ${r.keeps}`)),
    '',
    plain(c.why.source),
    '',
    `## ${plain(c.keeps.title)}`,
    '',
    plain(c.keeps.body),
    '',
    ...list(
      c.keeps.spaces.map(
        (s) => `${s.name}: ${s.covers} In the app: ${s.pages}.`,
      ),
    ),
    '',
    plain(c.keeps.more),
    '',
    `## ${plain(c.find.title)}`,
    '',
    plain(c.find.body),
    '',
    ...list(c.find.items.map((i) => `${i.title}: ${i.text}`)),
    '',
    `## ${plain(c.privacy.title)}`,
    '',
    plain(c.privacy.body),
    '',
    ...list(c.privacy.points),
    '',
    plain(c.privacy.network),
    '',
    `## ${plain(c.faq.title)}`,
    '',
    ...c.faq.items.flatMap((i) => [
      `### ${plain(i.title)}`,
      '',
      plain(i.text),
      '',
    ]),
    `## ${plain(c.download.title)}`,
    '',
    plain(c.download.body),
    '',
    ...list(c.download.platforms.map((p) => `${p.name}: ${p.detail}`)),
    '',
    plain(c.download.note),
    '',
    '## Pages',
    '',
    ...langs.map(
      (l) => `- [PathKeep (${copy[l.lang].htmlLang})](${at(l.path)})`,
    ),
    '',
    '## Project',
    '',
    `- [Source on GitHub](${repo})`,
    `- [Releases](${repo}/releases)`,
    `- [README](${repo}#readme)`,
    '',
  ]
  return new Response(lines.join('\n'), {
    headers: { 'Content-Type': 'text/plain; charset=utf-8' },
  })
}
