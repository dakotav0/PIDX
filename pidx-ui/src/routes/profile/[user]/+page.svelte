<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import {
		getProfile,
		getStatus,
		listObservations,
		confirmObservation,
		rejectObservation,
		type StatusResult,
		type ProfileDocument,
		type ObservationEntry
	} from '$lib/ipc';
	import DebuggerView from '$lib/components/DebuggerView.svelte';
	import InspectorView from '$lib/components/InspectorView.svelte';
	import GardenerView from '$lib/components/GardenerView.svelte';
	import ProfileView from '$lib/components/ProfileView.svelte';

	const userId = $derived(page.params.user!);

	let status = $state<StatusResult | null>(null);
	let profile = $state<ProfileDocument | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);

	type Tab = 'profile' | 'review' | 'garden' | 'inspect' | 'debug';
	let activeTab = $state<Tab>('profile');

	// Review inbox: all proposed observations across fields.
	let proposed = $state<ObservationEntry[]>([]);
	let reviewLoading = $state(false);
	let reviewError = $state<string | null>(null);
	let pending = $state<string[]>([]);

	async function loadProposed() {
		reviewLoading = true;
		reviewError = null;
		try {
			proposed = await listObservations(userId, { status: 'proposed' });
		} catch (e) {
			reviewError = String(e);
		} finally {
			reviewLoading = false;
		}
	}

	async function act(entry: ObservationEntry, fn: 'confirm' | 'reject') {
		const key = `${entry.path}:${entry.obs_index}`;
		pending = [...pending, key];
		try {
			if (fn === 'confirm') {
				await confirmObservation(userId, entry.path, entry.obs_index);
			} else {
				await rejectObservation(userId, entry.path, entry.obs_index);
			}
			await Promise.all([loadProposed(), load()]);
		} finally {
			pending = pending.filter((k) => k !== key);
		}
	}

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

	onMount(() => {
		load();
		loadProposed();
	});
</script>

<main class="p-6 w-full">
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
			{#each [['profile', 'Profile'], ['review', 'Review'], ['garden', 'Gardener'], ['inspect', 'Inspector'], ['debug', 'Debugger']] as [id, label]}
				<button
					class="px-3 py-1.5 text-sm -mb-px border-b-2 transition-colors {activeTab === id
						? 'border-accent text-accent'
						: 'border-transparent text-text-muted hover:text-text-secondary'}"
					onclick={() => (activeTab = id as Tab)}
				>{label}</button>
			{/each}
		</div>

		<!-- Tab content -->
		{#if activeTab === 'profile'}
			<ProfileView {userId} {profile} />
		{:else if activeTab === 'debug'}
			<DebuggerView {userId} {status} {profile} onUpdate={load} />
		{:else if activeTab === 'inspect'}
			<InspectorView {userId} {profile} onUpdate={load} />
		{:else if activeTab === 'garden'}
			<GardenerView {userId} {profile} onUpdate={load} />
		{:else if activeTab === 'review'}
			<!-- Review inbox: every proposed observation, one-click confirm/reject -->
			<div class="mb-4">
				<h2 class="text-sm font-semibold text-text-secondary mb-2">
					Proposed observations ({proposed.length})
				</h2>
				{#if reviewError}
					<p class="text-error text-sm">Error: {reviewError}</p>
				{:else if reviewLoading}
					<p class="text-text-muted text-sm">Loading…</p>
				{:else if proposed.length === 0}
					<p class="text-text-muted text-sm">No proposed observations. Clean inbox.</p>
				{:else}
					<ul class="space-y-2">
						{#each proposed as entry (entry.path + ':' + entry.obs_index)}
							{@const key = `${entry.path}:${entry.obs_index}`}
							<li class="border border-border rounded-md p-3">
								<div class="flex items-start justify-between gap-3">
									<div class="min-w-0">
										<div class="text-xs text-text-muted font-mono">{entry.path}</div>
										<p class="text-sm text-text-secondary mt-1">{entry.value}</p>
										<div class="text-xs text-text-muted mt-1">
											{Math.round(entry.confidence * 100)}% · {entry.source}
										</div>
									</div>
									<div class="flex gap-2 shrink-0">
										<button
											class="px-2 py-1 text-xs rounded border border-accent text-accent hover:bg-accent/10 disabled:opacity-40"
											disabled={pending.includes(key)}
											onclick={() => act(entry, 'confirm')}
										>Confirm</button>
										<button
											class="px-2 py-1 text-xs rounded border border-error text-error hover:bg-error/10 disabled:opacity-40"
											disabled={pending.includes(key)}
											onclick={() => act(entry, 'reject')}
										>Reject</button>
									</div>
								</div>
							</li>
						{/each}
					</ul>
				{/if}
			</div>
		{/if}
	{/if}
</main>
