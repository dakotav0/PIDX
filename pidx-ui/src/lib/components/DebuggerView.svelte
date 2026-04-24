<script lang="ts">
	import type { StatusResult, ProfileDocument } from '$lib/ipc';
	import { getFieldObs } from '$lib/profile';
	import ObservationTable from './ObservationTable.svelte';

	interface Props {
		userId: string;
		status: StatusResult;
		profile: ProfileDocument;
		onUpdate: () => void;
	}

	let { userId, status, profile, onUpdate }: Props = $props();

	let expanded = $state<string | null>(null);

	function toggle(path: string) {
		expanded = expanded === path ? null : path;
	}
</script>

<div>
	{#each status.fields as field}
		{@const isExpanded = expanded === field.path}
		{@const observations = getFieldObs(profile, field.path)}
		<div class="border-b border-border">
			<button
				class="w-full flex items-center gap-3 py-2 text-left hover:bg-surface-1 px-1 -mx-1 rounded"
				onclick={() => toggle(field.path)}
			>
				<span class="font-mono text-sm text-text-primary w-56 shrink-0">{field.path}</span>
				<span class="text-xs text-accent">✓{field.confirmed}</span>
				{#if field.proposed > 0}
					<span class="text-xs text-warn">+{field.proposed}</span>
				{/if}
				{#if field.delta > 0}
					<span class="text-xs text-orange-400">△{field.delta}</span>
				{/if}
				<span class="ml-auto text-text-muted text-xs">{isExpanded ? '▲' : '▼'}</span>
			</button>
			{#if isExpanded}
				<div class="pl-2">
					<ObservationTable {userId} fieldPath={field.path} {observations} {onUpdate} />
				</div>
			{/if}
		</div>
	{/each}
</div>
