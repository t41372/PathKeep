import {
  CircleCheckIcon,
  InfoIcon,
  Loader2Icon,
  OctagonXIcon,
  TriangleAlertIcon,
} from 'lucide-react'
import { useI18n } from '@/lib/i18n'
import { useTheme } from '@/lib/theme'
import { Toaster as Sonner, type ToasterProps } from 'sonner'

const Toaster = ({ toastOptions, ...props }: ToasterProps) => {
  const { resolved } = useTheme()
  const { t } = useI18n()

  return (
    <Sonner
      theme={resolved}
      className="toaster group"
      icons={{
        success: <CircleCheckIcon className="size-4" />,
        info: <InfoIcon className="size-4" />,
        warning: <TriangleAlertIcon className="size-4" />,
        error: <OctagonXIcon className="size-4" />,
        loading: <Loader2Icon className="size-4 animate-spin" />,
      }}
      style={
        {
          '--normal-bg': 'var(--popover)',
          '--normal-text': 'var(--popover-foreground)',
          '--normal-border': 'var(--border)',
          '--border-radius': 'var(--radius)',
        } as React.CSSProperties
      }
      // Names, in the user's language, the close button a toast that stays
      // until it is closed shows.
      toastOptions={{
        closeButtonAriaLabel: t('common.close'),
        ...toastOptions,
      }}
      {...props}
    />
  )
}

export { Toaster }
