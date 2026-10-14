/**
 * Adding an assistant service the way a user does, in Settings → AI, for the
 * specs that talk to a fake model (`fake-model.ts`). The service added
 * becomes the one Ask uses.
 */
import { expect, type Page } from '@playwright/test'

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
