import type { Config } from 'tailwindcss'

const config: Config = {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        paper: {
          DEFAULT: '#FAF7F2',
          card: '#F2EDE4',
          hover: '#EDE7DC',
        },
        ink: {
          DEFAULT: '#1A1612',
          secondary: '#6B6157',
          tertiary: '#9A9186',
          faint: '#C4BDB3',
        },
        oxblood: {
          DEFAULT: '#6B1F2A',
          light: '#8B3F4A',
          faint: 'rgba(107, 31, 42, 0.08)',
        },
        inkblue: {
          DEFAULT: '#1E3A5F',
          light: '#3E5A7F',
        },
        border: {
          DEFAULT: 'rgba(26, 22, 18, 0.08)',
          strong: 'rgba(26, 22, 18, 0.15)',
        },
      },
      fontFamily: {
        serif: [
          'Newsreader Variable',
          'Songti SC',
          'Noto Serif CJK SC',
          'Source Han Serif SC',
          'SimSun',
          'PMingLiU',
          'Hiragino Mincho ProN',
          'Yu Mincho',
          'Nanum Myeongjo',
          'serif',
        ],
        sans: [
          'Inter Variable',
          'PingFang SC',
          'Microsoft YaHei',
          'Hiragino Sans',
          'Meiryo',
          'Apple SD Gothic Neo',
          'Malgun Gothic',
          'sans-serif',
        ],
        mono: [
          'JetBrains Mono',
          'SF Mono',
          'Menlo',
          'Monaco',
          'Consolas',
          'monospace',
        ],
      },
      fontSize: {
        '2xs': ['0.625rem', { lineHeight: '0.875rem' }],
      },
      borderRadius: {
        sm: '3px',
        DEFAULT: '4px',
        md: '6px',
        lg: '8px',
      },
      boxShadow: {
        card: '0 1px 3px rgba(26, 22, 18, 0.04), 0 1px 2px rgba(26, 22, 18, 0.02)',
        'card-hover': '0 4px 12px rgba(26, 22, 18, 0.06), 0 2px 4px rgba(26, 22, 18, 0.04)',
      },
      animation: {
        'fade-in': 'fadeIn 0.2s ease-out',
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0', transform: 'translateY(4px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
      },
    },
  },
  plugins: [],
}

export default config
