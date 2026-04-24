<script lang="ts">
	import type { ProfileDocument } from '$lib/ipc';
	import { resolveReview, resolveDelta, runDecay } from '$lib/ipc';
	import { getFieldObs, formatObsValue, formatDate } from '$lib/profile';

	interface Props {
		userId: string;
		profile: ProfileDocument;
		onUpdate: () => void;
	}

	let { userId, profile, onUpdate }: Props = $props();

	type Queue = 'review' | 'delta';
	let activeQueue = $state<Queue>('review');
	let reviewIdx = $state(0);
	let deltaIdx = $state(0);
	let busy = $state(false);
	let decayResult = $state<{ flagged: number } | null>(null);

	const openReviews = $derived(profile.review_queue.filter((r) => !r.resolved));
	const openDeltas = $derived(profile.delta_queue.filter((d) => !d.resolved));

	const currentReview = $derived(
		openReviews.length > 0 ? openReviews[Math.min(reviewIdx, openReviews.length - 1)] : null
	);
	const currentDelta = $derived(
		openDeltas.length > 0 ? openDeltas[Math.min(deltaIdx, openDeltas.length - 1)] : null
	);

	const reviewObs = $derived(
		currentReview
			? (getFieldObs(profile, currentReview.field)[currentReview.observation_index] ?? null)
			: null
	);

	function navigate(dir: 1 | -1) {
		if (activeQueue === 'review') {
			reviewIdx = Math.max(0, Math.min(openReviews.length - 1, reviewIdx + dir));
		} else {
			deltaIdx = Math.max(0, Math.min(openDeltas.length - 1, deltaIdx + dir));
		}
	}

	async function handleReview(action: 'keep' | 'discard') {
		if (!currentReview || busy) return;
		busy = true;
		try {
			await resolveReview(userId, currentReview.id, action);
			onUpdate();
		} finally {
			busy = false;
		}
	}

	async function handleDelta(keep: 'a' | 'b') {
		if (!currentDelta || busy) return;
		busy = true;
		try {
			await resolveDelta(userId, currentDelta.id, keep);
			onUpdate();
		} finally {
			busy = false;
		}
	}

	async function runDecayPass() {
		if (busy) return;
		busy = true;
		decayResult = null;
		try {
			const res = await runDecay(userId);
			decayResult = { flagged: res.newly_flagged };
			onUpdate();
		} finally {
			busy = false;
		}
	}

	function handleKey(e: KeyboardEvent) {
		if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
		switch (e.key.toLowerCase()) {
			case 'j':
				navigate(1);
				break;
			case 'k':
				navigate(-1);
				break;
			case 'y':
				if (activeQueue === 'review') void handleReview('keep');
				break;
			case 'n':
				if (activeQueue === 'review') void handleReview('discard');
				break;
			case 'a':
				if (activeQueue === 'delta') void handleDelta('a');
				break;
			case 'b':
				if (activeQueue === 'delta') void handleDelta('b');
				break;
		}
		if (e.key === 'Tab') {
			e.preventDefault();
			activeQueue = activeQueue === 'review' ? 'delta' : 'review';
		}
	}
</script>

<svelte:window onkeydown={handleKey} />

{#if openReviews.length === 0 && openDeltas.length === 0}
	<div class="py-12 text-center">
		<p class="text-accent text-lg mb-1">✓ All clear</p>
		<p class="text-text-muted text-sm mb-6">No reviews or delta conflicts pending.</p>
		<button
			disabled={busy}
			class="text-xs px-3 py-1.5 rounded border border-border text-text-muted hover:text-text-secondary hover:border-accent disabled:opacity-40 transition-colors"
			onclick={runDecayPass}
		>Run decay pass</button>
		{#if decayResult}
			<p class="text-xs text-text-muted mt-2">
				{decayResult.flagged > 0
					? `↓ ${decayResult.flagged} observation${decayResult.flagged === 1 ? '' : 's'} flagged for review`
					: '✓ Nothing flagged'}
			</p>
		{/if}
	</div>
{:else}
	<!-- Queue tabs -->
	<div class="flex gap-1 mb-5 border-b border-border">
		<button
			class="px-3 py-1.5 text-sm -mb-px border-b-2 transition-colors {activeQueue === 'review'
				? 'border-accent text-accent'
				: 'border-transparent text-text-muted hover:text-text-secondary'}"
			onclick={() => (activeQueue = 'review')}
		>
			Review
			{#if openReviews.length > 0}
				<span class="ml-1 text-xs px-1.5 py-0.5 rounded-full bg-warn/20 text-warn"
					>{openReviews.length}</span
				>
			{/if}
		</button>
		<button
			class="px-3 py-1.5 text-sm -mb-px border-b-2 transition-colors {activeQueue === 'delta'
				? 'border-accent text-accent'
				: 'border-transparent text-text-muted hover:text-text-secondary'}"
			onclick={() => (activeQueue = 'delta')}
		>
			Deltas
			{#if openDeltas.length > 0}
				<span class="ml-1 text-xs px-1.5 py-0.5 rounded-full bg-orange-400/20 text-orange-400"
					>{openDeltas.length}</span
				>
			{/if}
		</button>
	</div>

	<!-- Review queue -->
	{#if activeQueue === 'review'}
		{#if openReviews.length === 0}
			<div class="py-10 text-center">
				<p class="text-accent">✓ Review queue clear</p>
			</div>
		{:else if currentReview}
			<!-- Progress -->
			<p class="text-xs text-text-muted mb-4">
				{Math.min(reviewIdx, openReviews.length - 1) + 1} of {openReviews.length}
				<span class="ml-2 opacity-60">J/K to navigate</span>
			</p>

			<!-- Card -->
			<div class="border border-border rounded-lg p-5 max-w-lg bg-surface-1">
				<p class="font-mono text-sm text-warn mb-4">{currentReview.field}</p>

				<div class="space-y-2 text-sm mb-5">
					<div class="flex gap-3">
						<span class="text-text-muted w-24 shrink-0">value</span>
						<span class="text-text-primary font-mono">
							{reviewObs ? formatObsValue(reviewObs.value) : '—'}
						</span>
					</div>
					<div class="flex gap-3">
						<span class="text-text-muted w-24 shrink-0">confidence</span>
						<span class="text-warn"
							>{(currentReview.effective_confidence * 100).toFixed(0)}%</span
						>
						<span class="text-text-muted text-xs">decayed below threshold</span>
					</div>
					<div class="flex gap-3">
						<span class="text-text-muted w-24 shrink-0">source</span>
						<span class="text-text-secondary text-xs">
							{reviewObs?.source.orientation ?? '—'}
						</span>
					</div>
					<div class="flex gap-3">
						<span class="text-text-muted w-24 shrink-0">flagged</span>
						<span class="text-text-secondary">{formatDate(currentReview.flagged_at)}</span>
					</div>
				</div>

				<div class="flex gap-3">
					<button
						disabled={busy}
						class="px-4 py-1.5 text-sm rounded border border-accent text-accent hover:bg-accent hover:text-surface-0 disabled:opacity-40 transition-colors"
						onclick={() => handleReview('keep')}
					>
						Keep <kbd class="ml-1 text-xs opacity-60">Y</kbd>
					</button>
					<button
						disabled={busy}
						class="px-4 py-1.5 text-sm rounded border border-error text-error hover:bg-error hover:text-surface-0 disabled:opacity-40 transition-colors"
						onclick={() => handleReview('discard')}
					>
						Archive <kbd class="ml-1 text-xs opacity-60">N</kbd>
					</button>
				</div>
			</div>

			<!-- Nav hint -->
			{#if openReviews.length > 1}
				<div class="flex gap-4 mt-4 text-xs text-text-muted">
					<button
						disabled={reviewIdx === 0}
						class="disabled:opacity-30"
						onclick={() => navigate(-1)}>← prev</button
					>
					<button
						disabled={reviewIdx >= openReviews.length - 1}
						class="disabled:opacity-30"
						onclick={() => navigate(1)}>next →</button
					>
				</div>
			{/if}
		{/if}
	{/if}

	<!-- Delta queue -->
	{#if activeQueue === 'delta'}
		{#if openDeltas.length === 0}
			<div class="py-10 text-center">
				<p class="text-accent">✓ No open deltas</p>
			</div>
		{:else if currentDelta}
			<!-- Progress -->
			<p class="text-xs text-text-muted mb-4">
				{Math.min(deltaIdx, openDeltas.length - 1) + 1} of {openDeltas.length}
				<span class="ml-2 opacity-60">J/K to navigate</span>
			</p>

			<!-- Delta card -->
			<div class="border border-border rounded-lg p-5 max-w-2xl bg-surface-1">
				<p class="font-mono text-sm text-orange-400 mb-1">{currentDelta.field}</p>
				<p class="text-xs text-text-muted mb-5">
					Conflict detected — choose which observation to keep.
				</p>

				<div class="grid grid-cols-2 gap-4 mb-5">
					{#each [{ side: 'a', obs: currentDelta.a }, { side: 'b', obs: currentDelta.b }] as { side, obs }}
						<div
							class="border border-border rounded p-3 {side === 'a'
								? 'border-accent/40'
								: 'border-warn/40'}"
						>
							<p
								class="text-xs font-bold mb-2 {side === 'a' ? 'text-accent' : 'text-warn'}"
							>
								{side.toUpperCase()}
							</p>
							<div class="space-y-1.5 text-xs">
								<div class="flex gap-2">
									<span class="text-text-muted w-12 shrink-0">value</span>
									<span class="text-text-primary font-mono break-all"
										>{formatObsValue(obs.value)}</span
									>
								</div>
								<div class="flex gap-2">
									<span class="text-text-muted w-12 shrink-0">conf</span>
									<span class="text-text-secondary"
										>{(obs.confidence * 100).toFixed(0)}%</span
									>
								</div>
								<div class="flex gap-2">
									<span class="text-text-muted w-12 shrink-0">source</span>
									<span class="text-text-secondary truncate" title={obs.source.orientation}
										>{obs.source.orientation}</span
									>
								</div>
								<div class="flex gap-2">
									<span class="text-text-muted w-12 shrink-0">date</span>
									<span class="text-text-secondary"
										>{formatDate(obs.source.timestamp)}</span
									>
								</div>
							</div>
						</div>
					{/each}
				</div>

				<div class="flex gap-3">
					<button
						disabled={busy}
						class="px-4 py-1.5 text-sm rounded border border-accent text-accent hover:bg-accent hover:text-surface-0 disabled:opacity-40 transition-colors"
						onclick={() => handleDelta('a')}
					>
						Keep A <kbd class="ml-1 text-xs opacity-60">A</kbd>
					</button>
					<button
						disabled={busy}
						class="px-4 py-1.5 text-sm rounded border border-warn text-warn hover:bg-warn hover:text-surface-0 disabled:opacity-40 transition-colors"
						onclick={() => handleDelta('b')}
					>
						Keep B <kbd class="ml-1 text-xs opacity-60">B</kbd>
					</button>
				</div>
			</div>

			{#if openDeltas.length > 1}
				<div class="flex gap-4 mt-4 text-xs text-text-muted">
					<button
						disabled={deltaIdx === 0}
						class="disabled:opacity-30"
						onclick={() => navigate(-1)}>← prev</button
					>
					<button
						disabled={deltaIdx >= openDeltas.length - 1}
						class="disabled:opacity-30"
						onclick={() => navigate(1)}>next →</button
					>
				</div>
			{/if}
		{/if}
	{/if}

	<!-- Decay footer -->
	<div class="mt-8 pt-4 border-t border-border flex items-center gap-3">
		<button
			disabled={busy}
			class="text-xs px-3 py-1.5 rounded border border-border text-text-muted hover:text-text-secondary hover:border-accent disabled:opacity-40 transition-colors"
			onclick={runDecayPass}
		>Run decay pass</button>
		{#if decayResult}
			<span class="text-xs text-text-muted">
				{decayResult.flagged > 0
					? `↓ ${decayResult.flagged} flagged for review`
					: '✓ nothing flagged'}
			</span>
		{/if}
	</div>
{/if}
