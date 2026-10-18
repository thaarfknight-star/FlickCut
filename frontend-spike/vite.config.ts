import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Must match build.devUrl in src-tauri/tauri.conf.json.
  server: {
    port: 1420,
    strictPort: true,
  },
})
