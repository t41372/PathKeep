/**
 * This module renders part of the sidebar navigation surface for the desktop shell.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `Sidebar`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import { sidebarSections } from '../../app/router'
import { useShellData } from '../../app/shell-data-context'
import { isArchiveUnlockRequiredMessage } from '../../lib/archive-access'
import {
  formatBuildVersionLabel,
  formatBuildVersionTitle,
} from '../../lib/build-info'
import { formatBytes } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import { BrandMark } from '../brand-mark'
import { SidebarBackgroundStatus } from './background-status'
import { SidebarNavItem } from './nav-item'

/**
 * Describes the props accepted by `Sidebar`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface SidebarProps {
  collapsed: boolean
  onToggle: () => void
}

/**
 * Explains how sidebar works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const { language, t } = useI18n()
  const {
    activeArchiveTask = null,
    buildInfo,
    dashboard,
    error,
    runtimeStatus = {
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    },
    snapshot,
  } = useShellData()
  const { activeProfileId } = useProfileScope()
  const archiveNeedsUnlock = isArchiveUnlockRequiredMessage(error)
  const buildLabel = formatBuildVersionLabel(buildInfo)
  const buildTitle = formatBuildVersionTitle(buildInfo)

  const archiveLabel = archiveNeedsUnlock
    ? t('navigation.archiveAttentionNeeded')
    : snapshot?.archiveStatus.warning
      ? t('navigation.archiveAttentionNeeded')
      : snapshot?.config.initialized
        ? t('navigation.archiveHealthy')
        : t('navigation.archiveNotInitialized')
  const runtimeLabel = archiveNeedsUnlock
    ? t('common.modeLocked')
    : snapshot?.archiveStatus.encrypted
      ? t('navigation.encryptedArchive')
      : t('navigation.plaintextArchive')
  const archiveSize = formatBytes(
    dashboard?.storage.archiveDatabaseBytes,
    language,
  )
  const activeProfile = snapshot?.browserProfiles.find(
    (profile) => profile.profileId === activeProfileId,
  )
  const profileScopeLabel = activeProfileId
    ? (activeProfile?.profileName ?? profileIdLabel(activeProfileId))
    : t('common.profileAllProfiles')

  /**
   * Explains how toggle theme works.
   *
   * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
   */
  function toggleTheme() {
    const html = document.documentElement
    const current = html.getAttribute('data-theme')
    const next = current === 'light' ? 'dark' : 'light'
    html.setAttribute('data-theme', next)
    try {
      window.localStorage.setItem('pathkeep.theme', next)
    } catch {
      // localStorage may be unavailable
    }
  }

  return (
    <aside
      className="sidebar"
      aria-label={t('navigation.primaryNavigation')}
      data-collapsed={collapsed ? 'true' : 'false'}
    >
      <div className="sidebar-header">
        <div className="logo-lockup">
          <div aria-hidden className="logo-mark">
            <BrandMark alt="" />
          </div>
          <div className="logo-text">
            <span className="logo-name">PATHKEEP</span>
            <span className="logo-version" title={buildTitle ?? undefined}>
              {buildLabel ?? t('navigation.loadingBuild')}
            </span>
          </div>
        </div>
        <button
          aria-label={
            collapsed
              ? t('navigation.expandNavigation')
              : t('navigation.collapseNavigation')
          }
          className="sidebar-toggle"
          type="button"
          onClick={onToggle}
        >
          <span aria-hidden>{collapsed ? '»' : '«'}</span>
        </button>
      </div>

      <nav className="nav-main">
        {sidebarSections.map((section) => (
          <div key={section.id} className="nav-section">
            <div className="nav-section-label">{t(section.labelKey)}</div>
            {section.items.map((item) => (
              <SidebarNavItem
                key={item.id}
                collapsed={collapsed}
                screen={item}
              />
            ))}
          </div>
        ))}
      </nav>

      <footer className="sidebar-footer">
        <div className="sidebar-status">
          <span
            aria-hidden
            className={`status-dot ${
              snapshot?.archiveStatus.warning
                ? 'status-dot--accent'
                : 'status-dot--ok'
            }`}
          />
          <span>{archiveLabel}</span>
        </div>
        <div className="sidebar-meta">
          <span>{runtimeLabel}</span>
          <span className="sep">·</span>
          <span>{archiveSize}</span>
        </div>
        <div className="sidebar-profile-scope">
          <span className="mono-support">
            {t('common.profileScope')}: {profileScopeLabel}
          </span>
        </div>
        <SidebarBackgroundStatus
          activeArchiveTask={activeArchiveTask}
          initialized={Boolean(snapshot?.config.initialized)}
          unlocked={Boolean(snapshot?.archiveStatus.unlocked)}
          runtimeStatus={runtimeStatus}
        />
        <button
          className="theme-toggle"
          type="button"
          aria-label={t('navigation.toggleTheme')}
          onClick={toggleTheme}
        >
          <span>◐</span>
        </button>
      </footer>
    </aside>
  )
}
