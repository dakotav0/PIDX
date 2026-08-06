<script lang="ts">
	import type { SectionItem } from '$lib/profile';

	interface Props {
		title: string;
		items: SectionItem[];
		empty?: string;
	}

	let { title, items, empty = 'nothing confirmed yet' }: Props = $props();
</script>

<section class="border border-border rounded-lg bg-surface-1 p-4">
	<header class="flex items-baseline justify-between mb-3">
		<h3 class="text-sm font-semibold text-text-primary">{title}</h3>
		{#if items.length > 0}
			<span class="text-xs text-text-muted tabular-nums">{items.length}</span>
		{/if}
	</header>

	{#if items.length === 0}
		<p class="text-xs text-text-muted">{empty}</p>
	{:else}
		<ul class="space-y-3">
			{#each items as item}
				<li>
					<div class="flex items-baseline gap-2">
						{#if item.key}
							<span class="text-xs text-text-muted font-mono shrink-0 w-20 truncate" title={item.key}
								>{item.key}</span
							>
						{/if}
						<span class="text-sm text-text-primary leading-snug">{item.value}</span>
					</div>
					{#if item.confidence != null}
						<div class="flex items-center gap-2 mt-1">
							<div class="h-1 rounded-full bg-surface-3 overflow-hidden w-32">
								<div
									class="h-full bg-accent/70"
									style="width: {Math.round(item.confidence * 100)}%"
								></div>
							</div>
							<span class="text-xs text-text-muted tabular-nums"
								>{Math.round(item.confidence * 100)}%</span
							>
							{#if item.source}
								<span
									class="text-xs text-text-muted truncate max-w-44 ml-auto"
									title={item.source}
									>{item.source}</span
								>
							{/if}
						</div>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</section>
