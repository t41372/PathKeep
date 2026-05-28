import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '@/lib/i18n/catalog/catalog-runtime'
import {
  buildPaperAssistantPrompts,
  buildPaperAssistantViewCopy,
} from './paper-assistant-copy'

function tFor(language: 'en' | 'zh-CN' | 'zh-TW') {
  return createNamespaceTranslator(language, 'assistant')
}

describe('buildPaperAssistantViewCopy', () => {
  test('builds English copy with the live provider label', () => {
    const copy = buildPaperAssistantViewCopy(tFor('en'), {
      providerLabel: 'Ollama / llama3.2',
    })
    expect(copy.greetingTitle).toBe('Ask your archive anything.')
    expect(copy.composer.placeholder).toContain('plain English')
    expect(copy.composer.attribution).toBe('Local · Ollama / llama3.2')
    expect(copy.composer.keyHint).toBe('↵ send · ⇧↵ newline')
    expect(copy.evidenceLabel).toContain('{count}')
  })

  test('falls back to the keyword-only attribution when no provider is set', () => {
    const copy = buildPaperAssistantViewCopy(tFor('en'))
    expect(copy.composer.attribution).toBe('Local · keyword only')
  })

  test('builds Simplified Chinese copy', () => {
    const copy = buildPaperAssistantViewCopy(tFor('zh-CN'), {
      providerLabel: 'Ollama / llama3.2',
    })
    expect(copy.greetingTitle).toBe('问你的存档任何问题。')
    expect(copy.composer.sendLabel).toBe('发送')
    expect(copy.composer.attribution).toBe('本机 · Ollama / llama3.2')
  })

  test('builds Traditional Chinese copy', () => {
    const copy = buildPaperAssistantViewCopy(tFor('zh-TW'))
    expect(copy.greetingTitle).toBe('問你的存檔任何問題。')
    expect(copy.composer.attribution).toBe('本機 · 關鍵字模式')
  })

  test('has no missing-key leakage across locales', () => {
    for (const language of ['en', 'zh-CN', 'zh-TW'] as const) {
      const copy = buildPaperAssistantViewCopy(tFor(language), {
        providerLabel: 'X',
      })
      const all: string[] = [
        copy.greetingTitle,
        typeof copy.greetingSubtitle === 'string' ? copy.greetingSubtitle : '',
        copy.composer.placeholder,
        copy.composer.sendLabel,
        copy.composer.attribution,
        copy.composer.keyHint,
        copy.evidenceLabel,
      ]
      for (const value of all) {
        expect(value).not.toMatch(/assistant\.paper/)
      }
    }
  })
})

describe('buildPaperAssistantPrompts', () => {
  test('builds three prompts in English', () => {
    const prompts = buildPaperAssistantPrompts(tFor('en'))
    expect(prompts).toHaveLength(3)
    expect(prompts[0].id).toBe('prompt-1')
    expect(prompts[0].text).toBe(
      'Where did I read about Rust async runtimes last week?',
    )
  })

  test('builds prompts in Simplified Chinese', () => {
    const prompts = buildPaperAssistantPrompts(tFor('zh-CN'))
    expect(prompts[1].text).toBe('我上次认真研究 SQLite 是什么时候？')
  })

  test('builds prompts in Traditional Chinese', () => {
    const prompts = buildPaperAssistantPrompts(tFor('zh-TW'))
    expect(prompts[2].text).toBe('這個月最占用我注意力的話題是什麼？')
  })
})
