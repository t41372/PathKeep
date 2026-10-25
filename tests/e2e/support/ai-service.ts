/**
 * Adding an AI service the way a user does, in Settings → AI: an assistant
 * for the specs that talk to a fake model (`fake-model.ts`), which becomes the
 * one Ask uses, or an embeddings service for the specs that build a semantic
 * index (`fake-embeddings.ts`).
 */
import { expect, type Page } from '@playwright/test'
import { unlockIfLocked } from './app'

export async function addService(
  page: Page,
  { name, address, key }: { name: string; address: string; key: string },
) {
  await page.goto('/#/settings/ai')
  await page.getByRole('combobox', { name: 'AI service' }).click()
  await page.getByRole('option', { name: 'Add provider…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Add an AI service' })
  await dialog.getByRole('combobox', { name: 'Service' }).click()
  await page.getByRole('option', { name: 'OpenAI or compatible' }).click()
  await dialog.getByRole('textbox', { name: 'Address' }).fill(address)
  await dialog.getByRole('textbox', { name: 'Model' }).fill('e2e-model')
  await dialog.getByRole('textbox', { name: 'Name' }).fill(name)
  await dialog.getByLabel('API key').fill(key)
  await dialog.getByRole('button', { name: 'Add service' }).click()
  await expect(page.getByText(`${name} is ready.`)).toBeVisible()
  await expect(
    page.getByRole('combobox', { name: 'AI service' }),
  ).toContainText(name)
}

/**
 * Adding an LM Studio service as a user does: the address its window shows,
 * no key, and `model` typed in (the list beside it is the service's).
 */
export async function addLmStudioService(
  page: Page,
  { name, address, model }: { name: string; address: string; model: string },
) {
  await page.goto('/#/settings/ai')
  await page.getByRole('combobox', { name: 'AI service' }).click()
  await page.getByRole('option', { name: 'Add provider…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Add an AI service' })
  await dialog.getByRole('combobox', { name: 'Service' }).click()
  await page.getByRole('option', { name: 'LM Studio (this computer)' }).click()
  await dialog.getByRole('textbox', { name: 'Address' }).fill(address)
  await dialog.getByRole('textbox', { name: 'Model' }).fill(model)
  await dialog.getByRole('textbox', { name: 'Name' }).fill(name)
  await dialog.getByRole('button', { name: 'Add service' }).click()
  await expect(page.getByText(`${name} is ready.`)).toBeVisible()
  await expect(
    page.getByRole('combobox', { name: 'AI service' }),
  ).toContainText(name)
}

/** Adds an OpenAI-compatible embeddings service in Settings → AI and picks it. */
export async function addEmbeddingsService(
  page: Page,
  url: string,
  model: string,
  name: string,
) {
  await page.goto('/#/settings/ai')
  await unlockIfLocked(page)
  await page.getByRole('combobox', { name: 'Model' }).click()
  await page.getByRole('option', { name: 'Add an embeddings service…' }).click()
  const dialog = page.getByRole('dialog', { name: 'Add an embeddings service' })
  await dialog.getByRole('combobox', { name: 'Service' }).click()
  await page.getByRole('option', { name: 'OpenAI or compatible' }).click()
  await dialog.getByRole('textbox', { name: 'Address' }).fill(url)
  await dialog.getByRole('textbox', { name: 'Model' }).fill(model)
  await dialog.getByRole('textbox', { name: 'Name' }).fill(name)
  await dialog.getByRole('button', { name: 'Add service' }).click()
  await expect(page.getByRole('combobox', { name: 'Model' })).toContainText(
    name,
  )
}
