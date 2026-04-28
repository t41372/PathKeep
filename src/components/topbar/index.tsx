/**
 * This module renders part of the topbar chrome that sits above route content.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `Topbar`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import { useState } from 'react'
import {
  NavigationType,
  useNavigate,
  useNavigationType,
} from 'react-router-dom'
import type { AppScreen } from '../../app/router'
import { isArchiveUnlockRequiredMessage } from '../../lib/archive-access'
import { useI18n } from '../../lib/i18n'
import { formatRelativeTime } from '../../lib/format'
import { ProfileSwitcher } from '../profile-switcher'
import { useShellData } from '../../app/shell-data-context'
import { Glyph } from '../ui'
import { readRouteHistoryIndex } from './history'

/**
 * Describes the props accepted by `Topbar`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface TopbarProps {
  screen: AppScreen
}

/**
 * Explains how topbar works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function Topbar({ screen }: TopbarProps) {
  const navigate = useNavigate()
  const navigationType = useNavigationType()
  const { language, t } = useI18n()
  const {
    activeArchiveTask = null,
    appLockStatus,
    busyAction,
    error,
    lockAppSession,
    dismissNotification = () => undefined,
    markNotificationsRead = () => undefined,
    notifications = [],
    runBackup,
    snapshot,
    unreadNotificationCount = 0,
  } = useShellData()
  const [notificationsOpen, setNotificationsOpen] = useState(false)
  const [forwardAvailable, setForwardAvailable] = useState(false)

  const archiveNeedsUnlock = isArchiveUnlockRequiredMessage(error)
  const backupDisabled =
    (!snapshot && !archiveNeedsUnlock) ||
    busyAction !== null ||
    activeArchiveTask !== null
  const backupLabel = archiveNeedsUnlock
    ? t('dashboard.reviewSecurity')
    : snapshot?.config.initialized
      ? t('navigation.backupNow')
      : t('navigation.initializeFirst')
  const title = t(screen.titleKey)
  const subtitle = t(screen.subtitleKey)
  const canGoBack = readRouteHistoryIndex() > 0
  const canGoForward = forwardAvailable && navigationType === NavigationType.Pop

  function navigateToRoute(href: string) {
    setForwardAvailable(false)
    void navigate(href)
  }

  return (
    <header className="topbar">
      <div className="topbar-left">
        <div
          aria-label={t('navigation.routeHistory')}
          className="topbar-history"
          role="group"
        >
          <button
            aria-label={t('navigation.goBack')}
            className="topbar-history__button"
            disabled={!canGoBack}
            type="button"
            onClick={() => {
              setForwardAvailable(true)
              void navigate(-1)
            }}
          >
            ←
          </button>
          <button
            aria-label={t('navigation.goForward')}
            className="topbar-history__button"
            disabled={!canGoForward}
            type="button"
            onClick={() => {
              setForwardAvailable(false)
              void navigate(1)
            }}
          >
            →
          </button>
        </div>
        <div>
          <h1 className="page-title">{title}</h1>
          <p className="page-subtitle">{subtitle}</p>
        </div>
      </div>

      <div className="topbar-right">
        {appLockStatus?.enabled ? (
          <button
            className="btn-secondary"
            type="button"
            disabled={busyAction !== null}
            onClick={() => {
              void lockAppSession('manual')
            }}
          >
            {t('navigation.lockNow')}
          </button>
        ) : null}
        <div className="topbar-notifications">
          <button
            aria-expanded={notificationsOpen}
            aria-label={
              unreadNotificationCount > 0
                ? t('navigation.notificationsUnread', {
                    count: unreadNotificationCount,
                  })
                : t('navigation.notifications')
            }
            className={`topbar-notifications__button ${
              unreadNotificationCount > 0
                ? 'topbar-notifications__button--unread'
                : ''
            }`}
            type="button"
            onClick={() => {
              const nextOpen = !notificationsOpen
              setNotificationsOpen(nextOpen)
              if (nextOpen) {
                markNotificationsRead()
              }
            }}
          >
            <Glyph icon="notifications" />
            {unreadNotificationCount > 0 ? (
              <span className="topbar-notifications__badge">
                {unreadNotificationCount}
              </span>
            ) : null}
          </button>
          {notificationsOpen ? (
            <div className="topbar-notifications__panel">
              <div className="topbar-notifications__header">
                <span>{t('navigation.notificationsPanelTitle')}</span>
              </div>
              {notifications.length > 0 ? (
                <div className="topbar-notifications__list">
                  {notifications.map((notification) => (
                    <div
                      key={notification.id}
                      className="topbar-notifications__item"
                      data-tone={notification.tone}
                    >
                      <div className="topbar-notifications__item-main">
                        <span className="topbar-notifications__time">
                          {formatRelativeTime(notification.timestamp, language)}
                        </span>
                        <strong>{notification.title}</strong>
                        <p>{notification.body}</p>
                        {notification.href ? (
                          <button
                            className="btn-tiny"
                            type="button"
                            onClick={() => {
                              setNotificationsOpen(false)
                              navigateToRoute(notification.href!)
                            }}
                          >
                            {t('jobs.archiveTaskOpenJobs')}
                          </button>
                        ) : null}
                      </div>
                      <button
                        aria-label={t('navigation.dismissNotification')}
                        className="topbar-notifications__dismiss"
                        type="button"
                        onClick={() => dismissNotification(notification.id)}
                      >
                        ×
                      </button>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="topbar-notifications__empty">
                  {t('navigation.notificationsEmpty')}
                </p>
              )}
            </div>
          ) : null}
        </div>
        <ProfileSwitcher />
        <button
          className="btn-backup"
          type="button"
          disabled={backupDisabled}
          aria-busy={busyAction !== null}
          onClick={() => {
            if (archiveNeedsUnlock) {
              navigateToRoute('/security')
              return
            }
            if (!snapshot?.config.initialized) {
              navigateToRoute('/onboarding')
              return
            }
            void runBackup().catch(() => undefined)
          }}
        >
          <span aria-hidden className="backup-icon">
            ⟳
          </span>
          {busyAction ?? backupLabel}
        </button>
      </div>
    </header>
  )
}
