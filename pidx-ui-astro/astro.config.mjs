import { defineConfig } from 'astro/config';
import svelte from '@astrojs/svelte';

// https://astro.build/config
export default defineConfig({
  integrations: [svelte()],

  // Tauri dev server expects output on a specific port
  server: {
    port: 1420,
    strictPort: true,
  },

  // Output static files for Tauri to serve
  output: 'static',

  // Vite config for Tauri compatibility
  vite: {
    // Prevent Vite from hiding Rust errors
    clearScreen: false,
    // Tauri expects a fixed port, fail if unavailable
    server: {
      strictPort: true,
    },
    // Env vars Tauri exposes to the frontend
    envPrefix: ['VITE_', 'TAURI_'],
    build: {
      // Tauri supports es2021
      target: ['es2021', 'chrome100', 'safari13'],
      // Don't minify for debug builds
      minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
      // Produce sourcemaps for debug builds
      sourcemap: !!process.env.TAURI_DEBUG,
    },
  },
});
