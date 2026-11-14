/**
 * Language and light/dark inside the onboarding window, as AppearanceControls
 * in src/features/onboarding/onboarding-page.tsx. The prototype only speaks
 * 繁體中文, so the language menu only remembers the choice.
 */
import { Moon, Sun } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { useProto } from '~/proto-state'

const languageNames = { en: 'English', 'zh-CN': '简体中文', 'zh-TW': '繁體中文' } as const

export function AppearanceControls() {
  const { theme, set } = useProto()
  const [preference, setPreference] = useState('system')
  return (
    <div className="flex gap-1.5">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="sm" aria-label="切換語言" className="h-[30px] rounded-lg bg-transparent px-2.5 text-xs font-medium text-muted-foreground shadow-none">
            {languageNames['zh-TW']}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="top" align="start">
          <DropdownMenuRadioGroup value={preference} onValueChange={setPreference}>
            <DropdownMenuRadioItem value="system">跟隨系統</DropdownMenuRadioItem>
            {Object.entries(languageNames).map(([lang, name]) => (
              <DropdownMenuRadioItem key={lang} value={lang}>
                {name}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <Button
        variant="outline"
        size="sm"
        aria-label={theme === 'dark' ? '切換到淺色' : '切換到深色'}
        onClick={() => set({ theme: theme === 'dark' ? 'light' : 'dark' })}
        className="h-[30px] w-8 rounded-lg bg-transparent px-0 text-muted-foreground shadow-none"
      >
        {theme === 'dark' ? <Sun /> : <Moon />}
      </Button>
    </div>
  )
}
