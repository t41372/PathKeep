import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react'
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
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([])

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

  const profileOptions = useMemo(
    () => [
      {
        id: null as string | null,
        label: t('common.profileAllProfiles'),
        icon: 'all' as const,
      },
      ...((selectedProfiles ?? []).map((profileId) => {
        const matchedProfile = (browserProfiles ?? []).find(
          (profile) => profile.profileId === profileId,
        )

        return {
          id: profileId,
          label: matchedProfile?.profileName ?? profileIdLabel(profileId),
          browserName:
            matchedProfile?.browserName ?? profileIdBrowserKind(profileId),
          icon: 'browser' as const,
        }
      }) ?? []),
    ],
    [browserProfiles, selectedProfiles, t],
  )

  const activeOptionIndex = profileOptions.findIndex(
    (option) => option.id === activeProfileId,
  )
  const focusOption = (nextIndex: number) => {
    optionRefs.current[nextIndex]?.focus()
  }

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

  useEffect(() => {
    if (!open) return
    const focusIndex = activeOptionIndex >= 0 ? activeOptionIndex : 0
    const frame = window.requestAnimationFrame(() => {
      focusOption(focusIndex)
    })

    return () => window.cancelAnimationFrame(frame)
  }, [activeOptionIndex, open])

  function handleOptionNavigation(
    event: ReactKeyboardEvent<HTMLButtonElement>,
    index: number,
  ) {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      focusOption((index + 1) % profileOptions.length)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      focusOption((index - 1 + profileOptions.length) % profileOptions.length)
      return
    }
    if (event.key === 'Home') {
      event.preventDefault()
      focusOption(0)
      return
    }
    if (event.key === 'End') {
      event.preventDefault()
      focusOption(profileOptions.length - 1)
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      setOpen(false)
      triggerRef.current?.focus()
    }
  }

  return (
    <div className="profile-switcher" ref={containerRef}>
      <button
        ref={triggerRef}
        aria-controls={open ? 'profile-scope-listbox' : undefined}
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-label={t('common.profileSwitchCurrent', {
          profile: activeProfileLabel,
        })}
        className="profile-switcher__trigger"
        type="button"
        onClick={() => setOpen((current) => !current)}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault()
            setOpen(true)
          }
        }}
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
          id="profile-scope-listbox"
          className="profile-switcher__dropdown"
          role="listbox"
          aria-label={t('common.profileSwitchLabel')}
        >
          {profileOptions.map((option, index) => (
            <button
              key={option.id ?? 'all'}
              ref={(element) => {
                optionRefs.current[index] = element
              }}
              className={`profile-switcher__option ${
                activeProfileId === option.id
                  ? 'profile-switcher__option--active'
                  : ''
              }`}
              role="option"
              aria-selected={activeProfileId === option.id}
              type="button"
              onClick={() => {
                setActiveProfileId(option.id)
                setOpen(false)
              }}
              onKeyDown={(event) => handleOptionNavigation(event, index)}
            >
              {option.icon === 'all' ? (
                <span className="profile-switcher__option-icon" aria-hidden>
                  ◎
                </span>
              ) : (
                <BrowserIcon
                  browserName={option.browserName}
                  className="profile-switcher__browser-icon"
                  decorative
                />
              )}
              <span>{option.label}</span>
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}
