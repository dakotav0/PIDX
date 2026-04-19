<script lang="ts">
	import { listUsers, getProfile, type UserEntry, type ProfileDocument } from '$lib/ipc';
	import { DIFF_FIELDS, getActiveValue } from '$lib/profile';
	import { onMount } from 'svelte';

	let users = $state<UserEntry[]>([]);
	let userA = $state('');
	let userB = $state('');
	let profileA = $state<ProfileDocument | null>(null);
	let profileB = $state<ProfileDocument | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);

	onMount(async () => {
		try {
			const res = await listUsers();
			users = res.users;
			if (users.length >= 1) userA = users[0].user_id;
			if (users.length >= 2) userB = users[1].user_id;
		} catch {
			// non-fatal
		}
	});

	async function compare() {
		if (!userA || !userB) return;
		loading = true;
		error = null;
		profileA = null;
		profileB = null;
		try {
			[profileA, profileB] = await Promise.all([getProfile(userA), getProfile(userB)]);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	// Group diff fields by their group key
	const groupedFields = $derived(() => {
		const groups: Record<string, typeof DIFF_FIELDS> = {};
		for (const f of DIFF_FIELDS) {
			if (!groups[f.group]) groups[f.group] = [];
			groups[f.group].push(f);
		}
		return Object.entries(groups);
	});

	function matchClass(a: string, b: string): string {
		if (a === '—' && b === '—') return 'text-text-muted';
		if (a === b) return 'text-accent';
		return 'text-warn';
	}
</script>

<main class="p-6 max-w-4xl">
	<h1 class="text-xl font-bold text-accent mb-1">Diff</h1>
	<p class="text-sm text-text-muted mb-6">Compare confirmed field values across two profiles.</p>

	<!-- Selectors -->
	<div class="flex items-end gap-4 mb-6">
		<div>
			<label for="diff-user-a" class="block text-xs text-text-muted mb-1">Profile A</label>
			{#if users.length > 0}
				<select
					id="diff-user-a"
					bind:value={userA}
					class="bg-surface-2 border border-border rounded px-2 py-1.5 text-sm text-text-primary focus:outline-none focus:border-accent"
				>
					{#each users as u}
						<option value={u.user_id}>{u.user_id}</option>
					{/each}
				</select>
			{:else}
				<input
					id="diff-user-a"
					bind:value={userA}
					placeholder="user_id"
					class="bg-surface-2 border border-border rounded px-2 py-1.5 text-sm text-text-primary focus:outline-none focus:border-accent"
				/>
			{/if}
		</div>

		<span class="text-text-muted text-sm mb-1.5">vs</span>

		<div>
			<label for="diff-user-b" class="block text-xs text-text-muted mb-1">Profile B</label>
			{#if users.length > 0}
				<select
					id="diff-user-b"
					bind:value={userB}
					class="bg-surface-2 border border-border rounded px-2 py-1.5 text-sm text-text-primary focus:outline-none focus:border-accent"
				>
					{#each users as u}
						<option value={u.user_id}>{u.user_id}</option>
					{/each}
				</select>
			{:else}
				<input
					id="diff-user-b"
					bind:value={userB}
					placeholder="user_id"
					class="bg-surface-2 border border-border rounded px-2 py-1.5 text-sm text-text-primary focus:outline-none focus:border-accent"
				/>
			{/if}
		</div>

		<button
			disabled={loading || !userA || !userB}
			class="px-4 py-1.5 text-sm rounded border border-accent text-accent hover:bg-accent hover:text-surface-0 disabled:opacity-40 transition-colors mb-0.5"
			onclick={compare}
		>
			{loading ? 'Loading…' : 'Compare'}
		</button>
	</div>

	{#if error}
		<p class="text-error text-sm">{error}</p>
	{:else if profileA && profileB}
		<!-- Legend -->
		<div class="flex gap-4 text-xs mb-4">
			<span class="text-accent">● match</span>
			<span class="text-warn">● differs</span>
			<span class="text-text-muted">— not set</span>
		</div>

		<!-- Comparison table -->
		<table class="w-full text-sm border-collapse">
			<thead>
				<tr class="text-left border-b border-border">
					<th class="pb-2 pr-4 font-normal text-text-muted w-48">field</th>
					<th class="pb-2 pr-4 font-normal text-accent">{userA}</th>
					<th class="pb-2 font-normal text-warn">{userB}</th>
				</tr>
			</thead>
			<tbody>
				{#each groupedFields() as [group, fields]}
					<tr>
						<td colspan="3" class="pt-4 pb-1 text-xs text-text-muted uppercase tracking-wider">
							{group}
						</td>
					</tr>
					{#each fields as field}
						{@const valA = getActiveValue(profileA, field.path)}
						{@const valB = getActiveValue(profileB, field.path)}
						<tr class="border-t border-border">
							<td class="py-1.5 pr-4 font-mono text-xs text-text-muted">{field.path}</td>
							<td class="py-1.5 pr-4 font-mono text-xs {matchClass(valA, valB)}">{valA}</td>
							<td class="py-1.5 font-mono text-xs {matchClass(valA, valB)}">{valB}</td>
						</tr>
					{/each}
				{/each}
			</tbody>
		</table>
	{/if}
</main>
