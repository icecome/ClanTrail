/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        primary: { DEFAULT: '#165dff', hover: '#0e4acf', light: '#e8f3ff' },
        danger: { DEFAULT: '#c41d7f', light: '#fcebf5' },
        text: { primary: '#1f2329', secondary: '#4e5969', tertiary: '#86909c' },
        border: { DEFAULT: '#e5e6eb', strong: '#d9dbe0' },
        surface: { DEFAULT: '#fff', muted: '#f2f3f5', page: '#f5f6f8' },
      },
      fontFamily: {
        sans: [
          '-apple-system', 'BlinkMacSystemFont', 'Segoe UI',
          'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', 'sans-serif',
        ],
      },
    },
  },
  plugins: [],
}

