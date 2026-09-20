import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  // Svelte 5 components must resolve their browser build under test.
  resolve: process.env.VITEST ? { conditions: ['browser'] } : undefined,
})
