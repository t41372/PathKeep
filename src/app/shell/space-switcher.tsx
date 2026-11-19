/**
 * The switcher at the top of the rail: the current space's icon and name,
 * a row of dots for which space it is when there is more than one, and a
 * menu of the spaces that are on, each with its status line and shortcut.
 *
 * Responsibilities: the button and the menu. The menu holds spaces only.
 * Its last item offers to keep another kind of data, and only while a
 * registered kind is off (`spacesOff`); it opens Settings › Manage data
 * types. A space's status line is its own component (registry `Status`),
 * mounted only while the menu is open, with a quiet placeholder until it
 * loads.
 * Not responsible for: switching (`current-space.ts`) or the slide
 * (`space-slide.tsx`).
 */
import { Check, Plus } from 'lucide-react'
import { DropdownMenu as Menu } from 'radix-ui'
import { Suspense, type CSSProperties } from 'react'
import { useNavigate } from 'react-router-dom'
import { Skeleton } from '@/components/ui/skeleton'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import {
  spaceKeyLabel,
  useSpacesOff,
  useSpacesOn,
  type DataSpace,
} from '@/lib/spaces'
import { useCurrentSpace, useSwitchSpace } from './current-space'
import { SpaceSlide } from './space-slide'

/** A small square in the space's own hue, whichever space is current. */
export function SpaceTile({
  space,
  size = 'md',
}: {
  space: DataSpace
  size?: 'sm' | 'md'
}) {
  const Icon = space.icon
  return (
    <span
      aria-hidden
      style={{ '--tile-h': space.hue } as CSSProperties}
      className={cn(
        'space-tile flex shrink-0 items-center justify-center rounded-[9px]',
        size === 'md' ? 'size-8' : 'size-7',
      )}
    >
      <Icon
        className={size === 'md' ? 'size-[18px]' : 'size-4'}
        strokeWidth={1.8}
      />
    </span>
  )
}

export function SpaceSwitcher({ dir }: { dir: number }) {
  const { t } = useI18n()
  const space = useCurrentSpace()
  const switchTo = useSwitchSpace()
  const navigate = useNavigate()
  const on = useSpacesOn()
  const off = useSpacesOff()
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        aria-label={t('spaces.switcher.label', { name: t(space.name) })}
        className="flex w-[64px] shrink-0 flex-col items-center gap-1 rounded-xl py-1.5 transition-colors outline-none hover:bg-foreground/5 focus-visible:ring-2 focus-visible:ring-ring data-[state=open]:bg-foreground/5"
      >
        <SpaceSlide id={space.id} dir={dir}>
          <span className="flex flex-col items-center gap-1">
            <SpaceTile space={space} />
            <span className="max-w-[60px] truncate text-[11px] leading-none font-semibold">
              {t(space.shortName)}
            </span>
          </span>
        </SpaceSlide>
        {/* Which space of how many, side by side like the slide. */}
        {on.length > 1 && (
          <span aria-hidden className="mt-0.5 flex items-center gap-[3px]">
            {on.map((item) => (
              <span
                key={item.id}
                className={cn(
                  'h-[4px] rounded-full transition-all duration-200',
                  item.id === space.id
                    ? 'w-2.5 bg-foreground/70'
                    : 'w-[4px] bg-foreground/20',
                )}
              />
            ))}
          </span>
        )}
      </DropdownMenuTrigger>
      <DropdownMenuContent
        side="right"
        align="start"
        sideOffset={10}
        className="w-[320px] p-1.5"
      >
        <Menu.RadioGroup
          value={space.id}
          onValueChange={(id) => {
            const next = on.find((item) => item.id === id)
            if (next) switchTo(next)
          }}
        >
          {on.map((item, index) => (
            <Menu.RadioItem
              key={item.id}
              value={item.id}
              // Choosing the current space from a shared screen opens it.
              onSelect={() => item.id === space.id && switchTo(item)}
              className="flex cursor-default items-center gap-3 rounded-lg px-2.5 py-2 outline-hidden select-none focus:bg-accent"
            >
              <SpaceTile space={item} size="sm" />
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="text-[13.5px] font-medium">
                  {t(item.name)}
                </span>
                <span className="truncate text-[12px] text-muted-foreground">
                  <Suspense
                    fallback={
                      <Skeleton aria-hidden className="my-[3px] h-3 w-32" />
                    }
                  >
                    <item.Status />
                  </Suspense>
                </span>
              </span>
              <span className="flex shrink-0 items-center gap-2 text-[11px] text-muted-foreground">
                {index < 9 && (
                  <kbd className="font-sans">{spaceKeyLabel(index + 1)}</kbd>
                )}
                <Menu.ItemIndicator forceMount asChild>
                  <Check
                    aria-hidden
                    className="size-4 text-foreground data-[state=unchecked]:invisible"
                  />
                </Menu.ItemIndicator>
              </span>
            </Menu.RadioItem>
          ))}
        </Menu.RadioGroup>
        {off.length > 0 && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              onSelect={() => void navigate('/settings/kinds')}
              className="gap-2.5 rounded-lg px-2.5 py-2 text-[13px]"
            >
              <Plus aria-hidden />
              {t('spaces.switcher.keepAnother')}
              <span className="ml-auto text-[12px] text-muted-foreground">
                {t('spaces.switcher.moreKinds', { count: off.length })}
              </span>
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
