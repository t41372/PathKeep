/**
 * `/robots.txt`: every crawler may read every page.
 */
import type { APIRoute } from 'astro'

export const GET: APIRoute = () =>
  new Response('User-agent: *\nAllow: /\n', {
    headers: { 'Content-Type': 'text/plain; charset=utf-8' },
  })
