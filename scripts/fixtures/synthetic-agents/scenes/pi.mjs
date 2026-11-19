/**
 * Responsibilities: the pi scenes: a branch (one reply with two follow-ups
 * under it) and a session whose recorded usage and cost the facts sum up.
 * Not responsible for: the session format (formats/pi.mjs).
 */

import { PI_MODELS } from '../bank.mjs'
import { dayAt, jsonl, uuid7 } from '../basics.mjs'
import { begin, count, finish, put, word } from '../context.mjs'
import { piFile, piHeader, piTurns, writePi } from '../formats/pi.mjs'

/** A pi session where the person went back and asked a follow-up twice. */
export function branchScene(ctx) {
  const c = begin(
    ctx,
    'pi-branch',
    'pi',
    'notes-site',
    dayAt(ctx.today, 4, 10, 30),
  )
  const w = word(c)
  const base = { id: uuid7(c.random, c.start), model: PI_MODELS[0] }
  const rows = piHeader(c, base)
  const render = (turn) => {
    const out = piTurns(base, [turn], c.state)
    count(c, out)
    rows.push(...out.rows)
    return out.rows[0].id
  }
  render({
    user: `Add an RSS feed to the blog. Posts are in src/content/posts, and the ${w} collection is the only one to include.`,
    steps: [
      {
        text: 'I will use @astrojs/rss and build the feed from the content collection.',
      },
      {
        tool: 'run',
        arg: 'npm install @astrojs/rss',
        out: 'added 3 packages, and audited 412 packages in 2s\nfound 0 vulnerabilities',
      },
      {
        text: 'Added src/pages/rss.xml.js. It lists every post with its title, date and summary.',
      },
    ],
  })
  const parentId = c.state.parent
  const [first, second] = [word(c), word(c)]
  const firstChild = render({
    user: `Leave out drafts, and give each item a ${first} category.`,
    steps: [
      {
        text: `Drafts are filtered out and every item carries the ${first} category now.`,
      },
    ],
  })
  c.state.parent = parentId
  const secondChild = render({
    user: `Actually, no categories. Put the whole post in each item, ${second} style.`,
    steps: [
      {
        text: `Each item now has the full post as content:encoded, rendered to HTML, ${second} style.`,
      },
    ],
  })
  const file = piFile(ctx, c.cwd, c.start, base.id)
  put(ctx, 'pi', 'conversation', file, jsonl(rows), c.state.at)
  c.files.push(file)
  finish(c, base.id)
  return {
    conversation: c.key,
    parentId,
    childIds: [firstChild, secondChild],
    words: [first, second],
  }
}

/** A pi session with token usage and cost on every reply. */
export function piUsageScene(ctx) {
  const c = begin(
    ctx,
    'pi-usage',
    'pi',
    'infra-scripts',
    dayAt(ctx.today, 12, 9, 50),
  )
  const [w1, w2] = [word(c), word(c)]
  writePi(
    c,
    [
      {
        user: `Our ${w1} deploy takes 14 minutes. Find where the time goes.`,
        steps: [
          {
            tool: 'run',
            arg: 'just deploy --timings',
            out: 'build      6m12s\nupload     5m48s\nmigrate      21s\nrestart      38s',
          },
          {
            text: 'Upload is the problem: every deploy uploads the whole 1.9 GB image. Pushing only changed layers should cut it to under a minute.',
          },
        ],
      },
      {
        user: 'Do that.',
        steps: [
          {
            text: `Switched the upload to the registry with layer caching; a deploy that changes only code now uploads 41 MB (${w2}).`,
          },
        ],
      },
      {
        user: 'What did this session cost so far?',
        steps: [
          {
            text: 'pi shows it under /session; each reply records its own usage and cost.',
          },
        ],
      },
    ],
    { model: PI_MODELS[0] },
  )
  const conversation = ctx.conversations.at(-1)
  return {
    conversation: c.key,
    usage: conversation.usage,
    assistantMessages: conversation.assistantMessages,
  }
}
