<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';

	interface UserEntry {
		user_id: string;
		version: string;
		updated: string;
		overall_confidence: number;
	}

	let users = $state<UserEntry[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);

	onMount(async () => {
		try {
			const result = await invoke<{ count: number; users: UserEntry[] }>('list_users');
			users = result.users;
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	});
</script>

<main>
	<h1>PIDX</h1>

	{#if loading}
		<p>Loading profiles…</p>
	{:else if error}
		<p class="error">Error: {error}</p>
	{:else if users.length === 0}
		<p>No profiles found.</p>
	{:else}
		<ul>
			{#each users as user}
				<li>
					<strong>{user.user_id}</strong>
					<span>v{user.version}</span>
					<span>{(user.overall_confidence * 100).toFixed(0)}%</span>
					<span>{user.updated.slice(0, 10)}</span>
				</li>
			{/each}
		</ul>
	{/if}
</main>

<style>
	main {
		padding: 2rem;
		font-family: monospace;
	}
	ul {
		list-style: none;
		padding: 0;
	}
	li {
		display: flex;
		gap: 1rem;
		padding: 0.5rem 0;
		border-bottom: 1px solid #333;
	}
	.error {
		color: red;
	}
</style>
