<script lang="ts">
	import favicon from '$lib/assets/favicon.svg';
	import { page } from '$app/state';
	import '../app.css';

	let { children } = $props();

	const isActive = (href: string) =>
		href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<nav class="flex items-center gap-5 px-6 py-2.5 border-b border-border bg-surface-1">
	<a href="/" class="font-bold text-accent text-sm">PIDX</a>
	<div class="flex gap-4 ml-4">
		{#each [['/', 'Profiles'], ['/bridge', 'Bridge'], ['/diff', 'Diff']] as [href, label]}
			<a
				{href}
				class="text-sm transition-colors {isActive(href)
					? 'text-text-primary'
					: 'text-text-muted hover:text-text-secondary'}"
			>{label}</a>
		{/each}
	</div>
</nav>

{@render children()}
