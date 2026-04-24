<script lang="ts">
	import type { ProfileDocument } from '$lib/ipc';
	import { getShow, annotate } from '$lib/ipc';
	import { getFieldObs, fieldConfidence, RADAR_AXES } from '$lib/profile';
	import ObservationTable from './ObservationTable.svelte';
	import RegisterRadar from './RegisterRadar.svelte';

	interface Props {
		userId: string;
		profile: ProfileDocument;
		onUpdate: () => void;
	}

	let { userId, profile, onUpdate }: Props = $props();

	type Tier = 'nano' | 'micro' | 'standard' | 'rich';
	const tiers: Tier[] = ['nano', 'micro', 'standard', 'rich'];

	let tier = $state<Tier>('micro');
	let showText = $state<string | null>(null);
	let showLoading = $state(false);

	let selectedField = $state<string | null>(null);

	let annotating = $state(false);
	let noteText = $state('');
	let notePinned = $state(false);
	let annotating_busy = $state(false);

	const radarAxes = $derived(
		RADAR_AXES.map((a) => ({ ...a, value: fieldConfidence(profile, a.path) }))
	);

	$effect(() => {
		const t = tier;
		const id = userId;
		showLoading = true;
		showText = null;
		getShow(id, t)
			.then((text) => {
				showText = text;
				showLoading = false;
			})
			.catch((e) => {
				showText = `Error: ${String(e)}`;
				showLoading = false;
			});
	});

	async function submitAnnotation(e: SubmitEvent) {
		e.preventDefault();
		if (!selectedField || !noteText.trim()) return;
		annotating_busy = true;
		try {
			await annotate(userId, selectedField, noteText.trim(), notePinned);
			annotating = false;
			noteText = '';
			notePinned = false;
			onUpdate();
		} finally {
			annotating_busy = false;
		}
	}

	function selectField(path: string) {
		selectedField = selectedField === path ? null : path;
		annotating = false;
		noteText = '';
	}
</script>

<!-- Tier toggle -->
<div class="flex gap-2 mb-5">
	{#each tiers as t}
		<button
			class="text-xs px-3 py-1 rounded border {tier === t
				? 'border-accent text-accent bg-surface-2'
				: 'border-border text-text-muted hover:text-text-secondary'}"
			onclick={() => (tier = t)}
		>
			{t}
		</button>
	{/each}
</div>

<!-- Inspector body: radar + content panel -->
<div class="flex gap-6 items-start">
	<!-- Radar -->
	<div class="shrink-0 w-[260px]">
		<RegisterRadar axes={radarAxes} selected={selectedField} onSelect={selectField} />
		<p class="text-xs text-text-muted text-center mt-1">register</p>
		{#if selectedField}
			<p class="text-xs text-text-muted text-center mt-0.5">click again to deselect</p>
		{/if}
	</div>

	<!-- Content panel -->
	<div class="flex-1 min-w-0">
		{#if selectedField}
			<!-- Drill-down -->
			<div class="flex items-center justify-between mb-2">
				<span class="font-mono text-sm text-accent">{selectedField}</span>
				<button
					class="text-xs text-text-muted hover:text-text-secondary"
					onclick={() => selectField(selectedField!)}
				>× close</button>
			</div>

			<ObservationTable
				{userId}
				fieldPath={selectedField}
				observations={getFieldObs(profile, selectedField)}
				{onUpdate}
			/>

			<!-- Annotation -->
			{#if annotating}
				<form onsubmit={submitAnnotation} class="mt-3 space-y-2">
					<textarea
						bind:value={noteText}
						placeholder="Add annotation…"
						class="w-full text-xs bg-surface-2 border border-border rounded p-2 text-text-primary resize-none h-20 focus:outline-none focus:border-accent"
					></textarea>
					<div class="flex items-center gap-3">
						<label class="flex items-center gap-1.5 text-xs text-text-secondary cursor-pointer">
							<input type="checkbox" bind:checked={notePinned} class="accent-[var(--color-accent)]" />
							pin
						</label>
						<button
							type="submit"
							disabled={annotating_busy || !noteText.trim()}
							class="text-xs px-2 py-1 border border-accent text-accent rounded hover:bg-accent hover:text-surface-0 disabled:opacity-40 transition-colors"
						>save</button>
						<button
							type="button"
							class="text-xs text-text-muted hover:text-text-secondary"
							onclick={() => {
								annotating = false;
								noteText = '';
							}}>cancel</button
						>
					</div>
				</form>
			{:else}
				<button
					class="text-xs text-text-muted hover:text-accent mt-2 transition-colors"
					onclick={() => (annotating = true)}
				>+ add annotation</button>
			{/if}
		{:else}
			<!-- Show output -->
			{#if showLoading}
				<p class="text-text-muted text-sm">Loading…</p>
			{:else if showText}
				<pre
					class="text-xs font-mono text-text-secondary whitespace-pre-wrap leading-relaxed"
				>{showText}</pre>
			{/if}
		{/if}
	</div>
</div>
