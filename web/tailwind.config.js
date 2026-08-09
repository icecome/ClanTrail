/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // 族迹设计系统 · 黛青主色 + 纸感底
        primary: {
          DEFAULT: '#35525E', // 黛青
          deep: '#2A414B', // 黛青深
          soft: '#E8EEF0', // 黛青浅
        },
        accent: {
          DEFAULT: '#B94A3C', // 朱砂（仅忌日提醒）
          soft: '#F6E7E3', // 朱砂浅
        },
        paper: '#F4F1EA', // 纸底
        card: '#FFFFFF',
        line: '#E6E1D8', // 卡片描边
        ink: {
          strong: '#23282B',
          DEFAULT: '#51585C',
          muted: '#8A9095',
          onprimary: '#F3F0E9', // 黛青底上的文字
        },
        success: { DEFAULT: '#4E7C5E' },
        danger: { DEFAULT: '#B04134', light: '#F6E7E3' },
      },
      fontFamily: {
        sans: [
          'Noto Sans SC', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI',
          'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', 'sans-serif',
        ],
      },
      boxShadow: {
        card: '0 4px 16px rgba(54, 82, 94, 0.06)',
        fab: '0 6px 16px rgba(54, 82, 94, 0.25)',
        tab: '0 8px 24px rgba(54, 82, 94, 0.16)',
      },
      borderRadius: {
        card: '16px',
        btn: '10px',
      },
    },
  },
  plugins: [],
}
