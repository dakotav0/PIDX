<script lang="ts">
	import { listUsers, type UserEntry } from '$lib/ipc';
	import { onMount } from 'svelte';

	let users = $state<UserEntry[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);

	onMount(async () => {
		try {
			const result = await listUsers();
			users = result.users;
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	});
</script>

<main class="p-6">
	<h1 class="text-xl font-bold mb-6 text-[var(--color-accent)]">PIDX</h1>

	{#if loading}
		<p class="text-[var(--color-text-secondary)]">Loading profiles…</p>
	{:else if error}
		<p class="text-[var(--color-error)]">Error: {error}</p>
	{:else if users.length === 0}
		<p class="text-[var(--color-text-muted)]">No profiles found.</p>
	{:else}
		<ul class="space-y-1">
			{#each users as user}
				<li class="flex gap-4 py-2 border-b border-[var(--color-border)]">
					<a href="/profile/{user.user_id}" class="font-bold text-[var(--color-text-primary)] hover:text-[var(--color-accent)]">
						{user.user_id}
					</a>
					<span class="text-[var(--color-text-muted)]">v{user.version}</span>
					<span class="text-[var(--color-accent)]">{(user.overall_confidence * 100).toFixed(0)}%</span>
					<span class="text-[var(--color-text-secondary)]">{user.updated.slice(0, 10)}</span>
				</li>
			{/each}
		</ul>
	{/if}
</main>
