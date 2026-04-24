<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { getProfile, getStatus, type StatusResult, type ProfileDocument } from '$lib/ipc';
	import DebuggerView from '$lib/components/DebuggerView.svelte';
	import InspectorView from '$lib/components/InspectorView.svelte';
	import GardenerView from '$lib/components/GardenerView.svelte';

	const userId = $derived(page.params.user!);

	let status = $state<StatusResult | null>(null);
	let profile = $state<ProfileDocument | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);

	type Tab = 'debug' | 'inspect' | 'garden';
	let activeTab = $state<Tab>('debug');

	const totals = $derived({
		confirmed: status?.fields.reduce((s, f) => s + f.confirmed, 0) ?? 0,
		proposed: status?.fields.reduce((s, f) => s + f.proposed, 0) ?? 0,
		delta: status?.fields.reduce((s, f) => s + f.delta, 0) ?? 0
	});

	async function load() {
		loading = true;
		error = null;
		try {
			[status, profile] = await Promise.all([getStatus(userId), getProfile(userId)]);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	onMount(load);
</script>

<main class="p-6 max-w-4xl">
	<a href="/" class="text-xs text-text-muted hover:text-accent mb-4 inline-block">← back</a>

	{#if loading}
		<p class="text-text-secondary">Loading…</p>
	{:else if error}
		<p class="text-error">Error: {error}</p>
	{:else if status && profile}
		<!-- Header -->
		<div class="flex items-baseline gap-4 mb-3">
			<h1 class="text-xl font-bold text-accent">{status.user_id}</h1>
			<span class="text-sm text-text-muted">v{status.version}</span>
			<span class="text-sm text-text-secondary">{(status.overall_confidence * 100).toFixed(0)}%</span>
			<span class="text-xs text-text-muted">{status.updated.slice(0, 10)}</span>
		</div>

		<!-- Stats -->
		<div class="flex gap-5 mb-5 text-sm">
			<span class="text-accent">✓ {totals.confirmed}</span>
			{#if totals.proposed > 0}
				<span class="text-warn">+{totals.proposed} proposed</span>
			{/if}
			{#if totals.delta > 0}
				<span class="text-orange-400">△{totals.delta} delta</span>
			{/if}
			{#if status.delta_queue_open > 0}
				<span class="text-orange-400">{status.delta_queue_open} open deltas</span>
			{/if}
			{#if status.review_queue_pending > 0}
				<span class="text-text-muted">{status.review_queue_pending} review pending</span>
			{/if}
		</div>

		<!-- Tab bar -->
		<div class="flex gap-1 mb-5 border-b border-border">
			{#each [['debug', 'Debugger'], ['inspect', 'Inspector'], ['garden', 'Gardener']] as [id, label]}
				<button
					class="px-3 py-1.5 text-sm -mb-px border-b-2 transition-colors {activeTab === id
						? 'border-accent text-accent'
						: 'border-transparent text-text-muted hover:text-text-secondary'}"
					onclick={() => (activeTab = id as Tab)}
				>{label}</button>
			{/each}
		</div>

		<!-- Tab content -->
		{#if activeTab === 'debug'}
			<DebuggerView {userId} {status} {profile} onUpdate={load} />
		{:else if activeTab === 'inspect'}
			<InspectorView {userId} {profile} onUpdate={load} />
		{:else if activeTab === 'garden'}
			<GardenerView {userId} {profile} onUpdate={load} />
		{/if}
	{/if}
</main>
