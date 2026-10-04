/**
 * Type helpers for message catalogs.
 *
 * English is the source of truth for the shape. `defineMessages` makes the
 * compiler reject a Chinese catalog that is missing a key or has an extra one,
 * so translations cannot silently drift.
 */

export type ResolvedLanguage = 'en' | 'zh-CN' | 'zh-TW'

export const supportedLanguages: readonly ResolvedLanguage[] = [
  'en',
  'zh-CN',
  'zh-TW',
]

export interface MessageTree {
  readonly [key: string]: string | MessageTree
}

/** Same keys as `T`, any string values. */
export type MessageShape<T> = {
  readonly [K in keyof T]: T[K] extends string ? string : MessageShape<T[K]>
}

export function defineMessages<const T extends MessageTree>(catalog: {
  en: T
  'zh-CN': MessageShape<T>
  'zh-TW': MessageShape<T>
}) {
  return catalog
}

type StripPlural<K extends string> =
  K extends `${infer Base}_${'one' | 'other'}` ? Base : K

/** Dotted paths to every leaf, with `_one` / `_other` plural forms folded. */
export type MessagePath<T, Prefix extends string = ''> = {
  [K in keyof T & string]: T[K] extends string
    ? `${Prefix}${StripPlural<K>}`
    : MessagePath<T[K], `${Prefix}${K}.`>
}[keyof T & string]
