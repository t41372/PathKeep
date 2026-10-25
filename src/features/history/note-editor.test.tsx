/**
 * `NoteField` against a backend whose saves answer when the test says so.
 *
 * A unit test because the bugs are races the E2E cannot set up on demand
 * (`quality-matrix.md` §什麼時候才寫 unit test): a save still on its way
 * when the page comes back, and two saves answering out of order. Only the
 * backend client is replaced; the panel, its query and the cache are real.
 *
 * Ways it can fail:
 * - a save from the page's last visit lands, the box shows it, and an edit
 *   back to the note it replaced is never written: the box shows one note
 *   and the archive keeps another;
 * - a second save is sent while the first is on its way, the answers come
 *   back in the other order, and the box falls back to the older note while
 *   the archive holds the newer one;
 * - a character is added, saved, and deleted again before the save answers;
 *   the box then looks unedited, takes the save's answer and the deletion
 *   is never written;
 * - the page is left with an edit waiting behind a save still on its way;
 *   back on the page, a newer edit is sent first and the old one lands
 *   after it, so the archive keeps the older note;
 * - the page is left twice: the second visit's correction back to the note it
 *   opened with waits behind the first visit's save, and is then taken for
 *   no change because that visit still thinks the archive holds its opening
 *   note;
 * - a tag is saved while a note save is on its way; the note's late answer
 *   carries the tags from before, the tag field takes them, and the next tag
 *   added writes a list without the first.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { UrlAnnotation } from '@/lib/backend-client/annotations'
import { I18nContext, type I18nValue } from '@/lib/i18n/context'
import { AnnotationSection } from './annotation-section'

const URL_ = 'https://example.com/page'

/**
 * The archive's note, and the saves sent to it. Like the archive's single
 * writer, saves are applied in the order they arrive; a save is applied
 * when it, or one sent after it, is answered, so a page read before then
 * still sees the old note. Only the answers can come back in another order.
 */
const archive = vi.hoisted(() => ({
  notes: '',
  tags: [] as string[],
  saves: [] as { notes: string; applied: boolean; answer: () => void }[],
  sent: [] as string[],
}))

vi.mock('@/lib/backend-client/annotations', () => {
  const annotation = (notes: string, tags: string[]): UrlAnnotation => ({
    url: 'https://example.com/page',
    notes,
    tags,
    updatedAt: '2026-10-29T00:00:00Z',
    createdAt: '2026-10-29T00:00:00Z',
  })
  return {
    annotationsClient: {
      getUrlAnnotation: () =>
        Promise.resolve(annotation(archive.notes, archive.tags)),
      // A note save answers with the page's tags as they were when it was written.
      setUrlNotes: ({ notes }: { notes: string }) =>
        new Promise<UrlAnnotation>((resolve) => {
          const tags = [...archive.tags]
          archive.sent.push(notes)
          archive.saves.push({
            notes,
            applied: false,
            answer: () => resolve(annotation(notes, tags)),
          })
        }),
      replaceUrlTags: ({ tags }: { tags: string[] }) => {
        archive.tags = [...tags]
        return Promise.resolve(annotation(archive.notes, archive.tags))
      },
    },
  }
})

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

const i18n = {
  t: (key: string) => key,
  language: 'en',
  locale: 'en',
} as unknown as I18nValue

let client: QueryClient
const roots: (() => void)[] = []

beforeEach(() => {
  vi.useFakeTimers()
  archive.notes = 'A'
  archive.tags = []
  archive.saves = []
  archive.sent = []
  client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
})

afterEach(() => {
  for (const unmount of roots.splice(0)) unmount()
  vi.useRealTimers()
})

/** Opens the page's panel; the returned function leaves it. */
async function open() {
  const container = document.createElement('div')
  document.body.append(container)
  const root = createRoot(container)
  act(() =>
    root.render(
      <QueryClientProvider client={client}>
        <I18nContext.Provider value={i18n}>
          <AnnotationSection url={URL_} onFilterTag={() => {}} />
        </I18nContext.Provider>
      </QueryClientProvider>,
    ),
  )
  await settle()
  let open = true
  const leave = () => {
    if (!open) return
    open = false
    act(() => root.unmount())
    container.remove()
  }
  roots.push(leave)
  const box = () =>
    container.querySelector<HTMLTextAreaElement>('#history-note')!
  const tagInput = () => container.querySelector<HTMLInputElement>('input')!
  return { box, tagInput, leave }
}

/** Lets promises and React's renders run. */
async function settle() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0)
  })
}

// The setter React's own value tracking cannot see, so the input event counts as an edit.
// eslint-disable-next-line @typescript-eslint/unbound-method -- called with `.call(box, …)`
const setTextareaValue = Object.getOwnPropertyDescriptor(
  HTMLTextAreaElement.prototype,
  'value',
)!.set as (this: HTMLTextAreaElement, value: string) => void

// eslint-disable-next-line @typescript-eslint/unbound-method -- called with `.call(input, …)`
const setInputValue = Object.getOwnPropertyDescriptor(
  HTMLInputElement.prototype,
  'value',
)!.set as (this: HTMLInputElement, value: string) => void

/** Types a tag and presses Enter. */
async function addTag(input: HTMLInputElement, tag: string) {
  act(() => {
    setInputValue.call(input, tag)
    input.dispatchEvent(new Event('input', { bubbles: true }))
  })
  act(() => {
    input.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
    )
  })
  await settle()
}

function type(box: HTMLTextAreaElement, value: string) {
  act(() => {
    setTextareaValue.call(box, value)
    box.dispatchEvent(new Event('input', { bubbles: true }))
  })
}

/** Waits past the pause after which an edit saves. */
async function pause() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1000)
  })
}

/** Answers the `index`th save still waiting, applying it and every save sent before it. */
async function answer(index = 0) {
  const save = waiting()[index]
  for (const earlier of archive.saves.slice(
    0,
    archive.saves.indexOf(save) + 1,
  )) {
    if (!earlier.applied) {
      earlier.applied = true
      archive.notes = earlier.notes
    }
  }
  const reply = save.answer
  save.answer = done
  act(() => reply())
  await settle()
}

const done = () => {}
const waiting = () => archive.saves.filter((save) => save.answer !== done)

describe('NoteField', () => {
  it('writes an edit back to the old note after a save from the last visit lands', async () => {
    const first = await open()
    type(first.box(), 'B')
    // Leaving writes the pending edit; its answer is still on its way.
    first.leave()
    await settle()
    expect(archive.sent).toEqual(['B'])

    const again = await open()
    await answer()
    expect(again.box().value).toBe('B')

    type(again.box(), 'A')
    await pause()
    expect(archive.sent).toEqual(['B', 'A'])
    await answer()
    expect(archive.notes).toBe('A')
    expect(again.box().value).toBe('A')
  })

  it('keeps a correction typed while the save it corrects is on its way', async () => {
    const page = await open()
    type(page.box(), 'Ax')
    await pause()
    expect(archive.sent).toEqual(['Ax'])
    // The save has not answered; the extra character goes again.
    type(page.box(), 'A')
    await answer()
    expect(page.box().value).toBe('A')
    await pause()
    expect(archive.sent).toEqual(['Ax', 'A'])
    await answer()
    expect(archive.notes).toBe('A')
    expect(page.box().value).toBe('A')
  })

  it('keeps an edit made on coming back over one the last visit still had to send', async () => {
    const first = await open()
    type(first.box(), 'B')
    await pause()
    // B is on its way; C waits for it, and leaving does not send it sooner.
    type(first.box(), 'C')
    first.leave()
    await settle()
    expect(archive.sent).toEqual(['B'])

    const again = await open()
    type(again.box(), 'D')
    await pause()
    while (waiting().length > 0) await answer()
    expect(archive.notes).toBe('D')
    expect(archive.sent.at(-1)).toBe('D')
  })

  it('writes a correction queued behind the last visit’s save after leaving again', async () => {
    const first = await open()
    type(first.box(), 'B')
    await pause()
    first.leave()
    await settle()
    const again = await open()
    // Opened while B is on its way: the box shows A. A typo, corrected back to A.
    type(again.box(), 'Ax')
    type(again.box(), 'A')
    await pause()
    again.leave()
    await settle()
    while (waiting().length > 0) await answer()
    expect(archive.notes).toBe('A')
  })

  it('keeps a tag saved while a note save is on its way', async () => {
    const page = await open()
    type(page.box(), 'B')
    await pause()
    await addTag(page.tagInput(), 'first')
    expect(archive.tags).toEqual(['first'])
    // The note's answer still carries the tags from before.
    await answer()
    await addTag(page.tagInput(), 'second')
    expect(archive.tags).toEqual(['first', 'second'])
  })

  it('keeps the newest note when two saves answer out of order', async () => {
    const page = await open()
    type(page.box(), 'X')
    await pause()
    type(page.box(), 'Y')
    await pause()
    // Whatever was sent, answering the later one first must not leave the
    // box on the older note.
    if (waiting().length === 2) await answer(1)
    while (waiting().length > 0) await answer()
    expect(archive.notes).toBe('Y')
    expect(page.box().value).toBe('Y')
    expect(
      client.getQueryData<UrlAnnotation>(['annotations', 'url', URL_]),
    ).toMatchObject({ notes: 'Y' })
  })
})
