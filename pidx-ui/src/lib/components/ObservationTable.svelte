<script lang="ts">
	import type { ObservationRow, ObservationStatus } from '$lib/ipc';
	import { confirmObservation, rejectObservation } from '$lib/ipc';
	import { formatObsValue, formatDate } from '$lib/profile';

	interface Props {
		userId: string;
		fieldPath: string;
		observations: ObservationRow[];
		onUpdate?: () => void;
	}

	let { userId, fieldPath, observations, onUpdate }: Props = $props();

	type Filter = ObservationStatus | 'all';
	let filter = $state<Filter>('all');
	let pendingIndices = $state<number[]>([]);

	const indexed = $derived(observations.map((obs, idx) => ({ obs, idx })));
	const visible = $derived(
		filter === 'all' ? indexed : indexed.filter(({ obs }) => obs.status === filter)
	);

	const tabs: { label: string; value: Filter }[] = [
		{ label: 'All', value: 'all' },
		{ label: 'Proposed', value: 'proposed' },
		{ label: 'Confirmed', value: 'confirmed' },
		{ label: 'Delta', value: 'delta' },
		{ label: 'Rejected', value: 'rejected' }
	];

	const statusColor: Record<string, string> = {
		proposed: 'text-[var(--color-warn)]',
		confirmed: 'text-[var(--color-accent)]',
		delta: 'text-orange-400',
		rejected: 'text-[var(--color-error)]',
		archived: 'text-[var(--color-text-muted)]'
	};

	async function confirm(idx: number) {
		pendingIndices = [...pendingIndices, idx];
		try {
			await confirmObservation(userId, fieldPath, idx);
			onUpdate?.();
		} finally {
			pendingIndices = pendingIndices.filter((i) => i !== idx);
		}
	}

	async function reject(idx: number) {
		pendingIndices = [...pendingIndices, idx];
		try {
			await rejectObservation(userId, fieldPath, idx);
			onUpdate?.();
		} finally {
			pendingIndices = pendingIndices.filter((i) => i !== idx);
		}
	}
</script>

<div class="mt-1 mb-3">
	<div class="flex gap-2 mb-2">
		{#each tabs as tab}
			<button
				class="text-xs px-2 py-0.5 rounded border {filter === tab.value
					? 'border-[var(--color-accent)] text-[var(--color-accent)]'
					: 'border-[var(--color-border)] text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]'}"
				onclick={() => (filter = tab.value)}
			>
				{tab.label}
			</button>
		{/each}
	</div>

	{#if visible.length === 0}
		<p class="text-xs text-[var(--color-text-muted)] pl-1">No observations match filter.</p>
	{:else}
		<table class="w-full text-xs border-collapse">
			<thead>
				<tr class="text-[var(--color-text-muted)] text-left">
					<th class="pb-1 pr-3 font-normal w-20">status</th>
					<th class="pb-1 pr-3 font-normal">value</th>
					<th class="pb-1 pr-3 font-normal w-12">conf</th>
					<th class="pb-1 pr-3 font-normal">source</th>
					<th class="pb-1 pr-3 font-normal w-24">date</th>
					<th class="pb-1 font-normal w-16"></th>
				</tr>
			</thead>
			<tbody>
				{#each visible as { obs, idx }}
					<tr class="border-t border-[var(--color-border)]">
						<td class="py-1 pr-3 {statusColor[obs.status] ?? ''}">{obs.status}</td>
						<td class="py-1 pr-3 text-[var(--color-text-primary)] font-mono break-all">
							{formatObsValue(obs.value)}
						</td>
						<td class="py-1 pr-3 text-[var(--color-text-secondary)]">
							{(obs.confidence * 100).toFixed(0)}%
						</td>
						<td
							class="py-1 pr-3 text-[var(--color-text-muted)] truncate max-w-[10rem]"
							title={obs.source.orientation}
						>
							{obs.source.orientation}
						</td>
						<td class="py-1 pr-3 text-[var(--color-text-muted)]">
							{formatDate(obs.source.timestamp)}
						</td>
						<td class="py-1">
							{#if obs.status === 'proposed'}
								<div class="flex gap-1">
									<button
										disabled={pendingIndices.includes(idx)}
										class="px-1.5 py-0.5 rounded text-[var(--color-accent)] border border-[var(--color-accent)] hover:bg-[var(--color-accent)] hover:text-[var(--color-surface-0)] disabled:opacity-40 transition-colors"
										onclick={() => confirm(idx)}
										title="Confirm"
									>✓</button>
									<button
										disabled={pendingIndices.includes(idx)}
										class="px-1.5 py-0.5 rounded text-[var(--color-error)] border border-[var(--color-error)] hover:bg-[var(--color-error)] hover:text-[var(--color-surface-0)] disabled:opacity-40 transition-colors"
										onclick={() => reject(idx)}
										title="Reject"
									>✗</button>
								</div>
							{/if}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</div>
