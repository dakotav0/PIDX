import { vitePreprocess } from '@astrojs/svelte';

/** @type {import('@sveltejs/vite-plugin-svelte').SvelteConfig} */
export default {
  // Astro handles Svelte integration via astro.config.mjs;
  // this file only exists to suppress the "no Svelte config found" warning.
  preprocess: vitePreprocess(),
};
