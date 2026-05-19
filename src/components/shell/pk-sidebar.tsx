/**
 * Paper-redesign sidebar: brand mark, sectioned nav, theme/lock footer.
 *
 * Why this file exists:
 * - The sidebar is the most visible piece of shell chrome; it must encode the
 *   paper aesthetic (cream surface, serif logo, mono section labels, slate-blue
 *   active state) and the CORE / OPERATIONS / SYSTEM grouping from
 *   docs/design/screens-and-nav.md.
 *
 * Responsibilities:
 * - Render the sidebar in expanded (216 px) or collapsed (56 px) state.
 * - Drive route navigation through React Router NavLink, honoring the active
 *   route id passed in via `activeId`.
 * - Surface a brand mark, version, theme toggle, lock button, and collapse toggle.
 *
 * Not responsible for:
 * - Loading shell data (parent shell does that).
 * - Holding sidebar collapsed state in storage (parent controls + persists).
 */

import { NavLink } from 'react-router-dom'
import { useI18n } from '@/lib/i18n/hooks'
import { sidebarSections, type AppRouteId } from '@/app/router'
import { PKBrandMark } from '@/components/shell/pk-brand-mark'
import { PKGlyph, type GlyphIconName } from '@/components/shell/pk-glyph'
import { cn } from '@/lib/cn'

export interface PKSidebarProps {
  activeId: AppRouteId
  collapsed: boolean
  onToggleCollapse: () => void
  theme: 'light' | 'dark'
  onToggleTheme: () => void
  onLockNow: () => void
  buildVersion: string | null
  archiveHealthy: boolean
}

export function PKSidebar({
  activeId,
  collapsed,
  onToggleCollapse,
  theme,
  onToggleTheme,
  onLockNow,
  buildVersion,
  archiveHealthy,
}: PKSidebarProps) {
  const { t } = useI18n()

  return (
    <aside
      className={cn(
        'flex h-full shrink-0 flex-col overflow-hidden border-r border-border-default bg-sidebar-bg select-none',
        'transition-[width] duration-250 ease-out',
        collapsed ? 'w-[56px]' : 'w-[216px]',
      )}
      data-testid="pk-sidebar"
      data-collapsed={collapsed ? 'true' : 'false'}
    >
      <div
        className={cn(
          'flex items-center gap-[10px] transition-[padding] duration-250 ease-out',
          collapsed ? 'flex-col px-0 pt-[14px] pb-2' : 'px-4 pt-[18px] pb-[14px]',
        )}
      >
        <PKBrandMark size={30} />
        {!collapsed && (
          <div className="flex min-w-0 flex-col">
            <span className="font-serif text-[17px] leading-[1.1] font-medium tracking-[-0.01em] text-ink">
              PathKeep
            </span>
            <span className="mt-[2px] font-mono text-[10px] tracking-[0.02em] text-ink-faint">
              {buildVersion ?? t('navigation.loadingBuild')}
            </span>
          </div>
        )}
        <button
          type="button"
          onClick={onToggleCollapse}
          title={
            collapsed
              ? t('navigation.expandNavigation')
              : t('navigation.collapseNavigation')
          }
          aria-label={
            collapsed
              ? t('navigation.expandNavigation')
              : t('navigation.collapseNavigation')
          }
          className={cn(
            'flex h-6 w-6 items-center justify-center text-ink-faint transition-colors hover:bg-hover hover:text-ink',
            collapsed ? '' : 'ml-auto',
          )}
        >
          <PKGlyph icon={collapsed ? 'arrow_right' : 'arrow_left'} size={14} />
        </button>
      </div>

      <nav
        className="pk-scrollbar flex-1 overflow-y-auto py-2"
        aria-label={t('navigation.primaryNavigation')}
      >
        {sidebarSections.map((section) => (
          <div
            key={section.id}
            className={cn(
              'py-2',
              'first:pt-0',
              'border-t border-border-light first:border-t-0',
              collapsed ? 'mt-1 pt-2' : 'mt-1 pt-3',
            )}
          >
            {!collapsed && (
              <div className="px-5 pb-1 font-mono text-[9.5px] font-medium tracking-[0.08em] text-ink-faint uppercase select-none">
                {t(section.labelKey)}
              </div>
            )}
            {section.items.map((item) => (
              <NavLink
                key={item.id}
                to={item.href}
                end={item.href === '/'}
                title={collapsed ? t(item.labelKey) : undefined}
                className={({ isActive }) =>
                  cn(
                    'group flex items-center gap-[10px] py-2 text-left text-[13.5px] font-[450] transition-colors',
                    'border-l-2 border-l-transparent',
                    collapsed ? 'justify-center px-0' : 'px-5',
                    isActive || activeId === item.id
                      ? 'border-l-accent bg-accent-soft font-medium text-accent-text'
                      : 'text-ink-muted hover:bg-hover hover:text-ink-secondary',
                  )
                }
              >
                <span className="grid h-5 w-5 shrink-0 place-items-center">
                  <PKGlyph icon={item.icon as GlyphIconName} size={20} />
                </span>
                {!collapsed && (
                  <>
                    <span className="min-w-0 flex-1 truncate">
                      {t(item.labelKey)}
                    </span>
                    {item.badgeKey ? (
                      <span className="border-border-default text-ink-faint -mr-1 ml-auto border px-[5px] py-[1px] font-mono text-[9px] font-semibold tracking-[0.06em] uppercase">
                        {t(item.badgeKey)}
                      </span>
                    ) : null}
                  </>
                )}
              </NavLink>
            ))}
          </div>
        ))}
      </nav>

      <div
        className={cn(
          'border-t border-border-light',
          collapsed
            ? 'flex flex-col items-center gap-1 px-1.5 py-2'
            : 'flex items-center justify-between gap-2 px-4 py-3',
        )}
      >
        <button
          type="button"
          onClick={onLockNow}
          title={t('navigation.lockNow')}
          aria-label={t('navigation.lockNow')}
          className={cn(
            'border-border-default text-ink-muted hover:border-ink-muted hover:text-ink flex h-7 w-7 items-center justify-center border text-[14px] transition-colors',
            collapsed ? '' : '',
          )}
        >
          <PKGlyph icon="lock" size={14} />
        </button>
        {!collapsed && (
          <span
            className="text-ink-faint flex-1 truncate font-mono text-[10.5px]"
            title={
              archiveHealthy
                ? t('navigation.archiveHealthy')
                : t('navigation.archiveAttentionNeeded')
            }
          >
            <span
              className={cn(
                'mr-2 inline-block h-1.5 w-1.5 rounded-full align-middle',
                archiveHealthy ? 'bg-success' : 'bg-warning',
              )}
            />
            {archiveHealthy
              ? t('navigation.archiveHealthy')
              : t('navigation.archiveAttentionNeeded')}
          </span>
        )}
        <button
          type="button"
          onClick={onToggleTheme}
          title={t('navigation.toggleTheme')}
          aria-label={t('navigation.toggleTheme')}
          className="border-border-default text-ink-muted hover:border-accent hover:text-accent flex h-7 w-7 items-center justify-center border text-[14px] transition-colors"
        >
          <PKGlyph icon={theme === 'dark' ? 'sun' : 'moon'} size={14} />
        </button>
      </div>
    </aside>
  )
}
