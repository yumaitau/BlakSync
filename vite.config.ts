import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const guiTarget = process.env.BLAKSYNC_GUI_PROXY || 'http://127.0.0.1:8385'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    host: '127.0.0.1',
    port: 5173,
    proxy: {
      '/api': {
        target: guiTarget,
        changeOrigin: false,
      },
    },
  },
  preview: {
    host: '127.0.0.1',
    port: 4173,
  },
})
