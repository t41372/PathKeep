import { useEffect, useMemo, useRef, useState } from 'react'
import { useShellData } from '../app/shell-data-context'
import { BrowserIcon } from '../lib/browser-icons'
import { useI18n } from '../lib/i18n'
import {
  profileIdBrowserKind,
  profileIdLabel,
  useProfileScope,
} from '../lib/profile-scope-context'

export function ProfileSwitcher() {
  const { snapshot } = useShellData()
  const { t } = useI18n()
  const { activeProfileId, setActiveProfileId } = useProfileScope()
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement | null>(null)

  const selectedProfiles = snapshot?.config.selectedProfileIds
  const browserProfiles = snapshot?.browserProfiles
  const activeProfileLabel = useMemo(() => {
    if (!activeProfileId) {
      return t('common.profileAllProfiles')
    }

    const matchedProfile = (browserProfiles ?? []).find(
      (profile) => profile.profileId === activeProfileId,
    )

    return matchedProfile?.profileName ?? profileIdLabel(activeProfileId)
  }, [activeProfileId, browserProfiles, t])

  useEffect(() => {
    if (
      activeProfileId &&
      !selectedProfiles?.some((profileId) => profileId === activeProfileId)
    ) {
      setActiveProfileId(null)
    }
  }, [activeProfileId, selectedProfiles, setActiveProfileId])

  useEffect(() => {
    if (!open) return

    const handlePointerDown = (event: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setOpen(false)
      }
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setOpen(false)
      }
    }

    document.addEventListener('mousedown', handlePointerDown)
    document.addEventListener('keydown', handleKeyDown)

    return () => {
      document.removeEventListener('mousedown', handlePointerDown)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [open])

  return (
    <div className="profile-switcher" ref={containerRef}>
      <button
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-label={t('common.profileSwitchLabel')}
        className="profile-switcher__trigger"
        type="button"
        onClick={() => setOpen((current) => !current)}
      >
        <span className="profile-switcher__icon" aria-hidden>
          ◉
        </span>
        <span className="profile-switcher__label">{activeProfileLabel}</span>
        <span className="profile-switcher__caret" aria-hidden>
          {open ? '▴' : '▾'}
        </span>
      </button>

      {open ? (
        <div
          className="profile-switcher__dropdown"
          role="listbox"
          aria-label={t('common.profileSwitchLabel')}
        >
          <button
            className={`profile-switcher__option ${
              activeProfileId === null ? 'profile-switcher__option--active' : ''
            }`}
            role="option"
            aria-selected={activeProfileId === null}
            type="button"
            onClick={() => {
              setActiveProfileId(null)
              setOpen(false)
            }}
          >
            <span className="profile-switcher__option-icon" aria-hidden>
              ◎
            </span>
            <span>{t('common.profileAllProfiles')}</span>
          </button>

          {(selectedProfiles ?? []).map((profileId) => {
            const matchedProfile = (browserProfiles ?? []).find(
              (profile) => profile.profileId === profileId,
            )

            return (
              <button
                key={profileId}
                className={`profile-switcher__option ${
                  activeProfileId === profileId
                    ? 'profile-switcher__option--active'
                    : ''
                }`}
                role="option"
                aria-selected={activeProfileId === profileId}
                type="button"
                onClick={() => {
                  setActiveProfileId(profileId)
                  setOpen(false)
                }}
              >
                <BrowserIcon
                  browserName={
                    matchedProfile?.browserName ??
                    profileIdBrowserKind(profileId)
                  }
                  className="profile-switcher__browser-icon"
                  decorative
                />
                <span>
                  {matchedProfile?.profileName ?? profileIdLabel(profileId)}
                </span>
              </button>
            )
          })}
        </div>
      ) : null}
    </div>
  )
}
