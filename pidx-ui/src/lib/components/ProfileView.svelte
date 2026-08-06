<script lang="ts">
	import type { ProfileDocument, RegisterMetric, ProfileField } from '$lib/ipc';
	import { getRegister } from '$lib/ipc';
	import {
		REGISTER_METRICS,
		listConfirmed,
		topConfirmed,
		obsToItem,
		formatObsValue,
		type SectionItem
	} from '$lib/profile';
	import RegisterRadar from './RegisterRadar.svelte';
	import ProfileSection from './ProfileSection.svelte';

	interface Props {
		userId: string;
		profile: ProfileDocument;
	}

	let { userId, profile }: Props = $props();

	type Tier = 'nano' | 'micro' | 'standard' | 'rich';
	const TIERS: Tier[] = ['nano', 'micro', 'standard', 'rich'];
	const TIER_CAPTION: Record<Tier, string> = {
		nano: 'identity core only',
		micro: '+ working · register',
		standard: '+ domains · values · reasoning',
		rich: '+ signals · notes · conflicts'
	};

	let tier = $state<Tier>('standard');
	const tierIndex = $derived(TIERS.indexOf(tier));
	const show = $derived({
		nano: tierIndex >= 0,
		micro: tierIndex >= 1,
		standard: tierIndex >= 2,
		rich: tierIndex >= 3
	});

	// ── Register (live scores, computed server-side) ─────────────────────────
	let register = $state<RegisterMetric[]>([]);
	let registerLoading = $state(true);
	let registerError = $state<string | null>(null);

	$effect(() => {
		const id = userId;
		registerLoading = true;
		registerError = null;
		getRegister(id)
			.then((r) => {
				register = r;
				registerLoading = false;
			})
			.catch((e) => {
				registerError = String(e);
				registerLoading = false;
			});
	});

	const radarAxes = $derived(
		REGISTER_METRICS.map((m) => {
			const metric = register.find((r) => r.name === m.key);
			return {
				path: m.key,
				label: m.label,
				short: m.short,
				value: metric ? metric.score / 10 : 0
			};
		})
	);

	const registerRows = $derived(
		REGISTER_METRICS.map((m) => {
			const metric = register.find((r) => r.name === m.key);
			return {
				key: m.label,
				value: metric ? `${metric.score.toFixed(1)} · ${metric.evidence_count} ev` : '—',
				confidence: metric ? metric.score / 10 : undefined
			};
		})
	);

	// ── Sections (confirmed only; output semantics = CLI show) ───────────────
	const identityRows = $derived(listConfirmed(profile.identity.core, 3).map(obsToItem));
	const workingRows = $derived(
		(['mode', 'pace', 'feedback', 'pattern'] as const).flatMap((k) =>
			topConfirmed((profile.working as Record<string, ProfileField>)[k], 1).map((o) => ({
				key: k,
				...obsToItem(o)
			}))
		)
	);
	const reasoningRows = $derived(
		(['style', 'pattern', 'intake', 'stance'] as const).flatMap((k) =>
			topConfirmed((profile.identity.reasoning as Record<string, ProfileField>)[k], 1).map(
				(o) => ({
					key: k,
					...obsToItem(o)
				})
			)
		)
	);
	const domainsRows = $derived(listConfirmed(profile.domains, 6).map(obsToItem));
	const valuesRows = $derived(listConfirmed(profile.values, 6).map(obsToItem));
	const signalsRows = $derived(
		(['phrases', 'avoidances', 'rhythms', 'framings'] as const).flatMap((k) =>
			listConfirmed((profile.signals as Record<string, ProfileField[]>)[k], 3).map((o) => ({
				key: k,
				...obsToItem(o)
			}))
		)
	);
	const annotationRows = $derived(
		[...profile.annotations]
			.sort((a, b) => Number(b.pinned) - Number(a.pinned))
			.map((a): SectionItem => ({
				key: a.field,
				value: a.note,
				source: a.pinned ? `📌 ${a.author}` : a.author
			}))
	);
	const deltaRows = $derived(
		profile.delta_queue
			.filter((d) => !d.resolved)
			.slice(0, 5)
			.map((d): SectionItem => ({
				key: d.field,
				value: `A: ${formatObsValue(d.a.value)}  ⇄  B: ${formatObsValue(d.b.value)}`,
				source: `${d.a.source.orientation} vs ${d.b.source.orientation}`
			}))
	);
</script>

<!-- Tier toggle -->
<div class="flex flex-wrap items-center gap-2 mb-5">
	{#each TIERS as t}
		<button
			class="text-xs px-3 py-1 rounded border transition-colors {tier === t
				? 'border-accent text-accent bg-surface-2'
				: 'border-border text-text-muted hover:text-text-secondary'}"
			onclick={() => (tier = t)}
		>
			{t}
		</button>
	{/each}
	<span class="text-xs text-text-muted">{TIER_CAPTION[tier]}</span>
</div>

<div class="md:flex md:gap-6 items-start">
	<!-- Register panel -->
	<div class="shrink-0 md:w-72 mb-6 md:mb-0">
		{#if registerError}
			<p class="text-error text-xs">register: {registerError}</p>
		{:else if registerLoading}
			<p class="text-text-muted text-xs mb-2">register…</p>
		{:else}
			<RegisterRadar axes={radarAxes} />
			<p class="text-xs text-text-muted text-center mt-1">register · recomputed at read</p>
		{/if}

		{#if show.micro && registerRows.length > 0}
			<ul class="mt-4 space-y-1.5">
				{#each registerRows as row}
					<li class="flex items-center gap-2 text-xs">
						<span class="text-text-muted w-24 shrink-0 font-mono">{row.key}</span>
						<div class="h-1 rounded-full bg-surface-3 flex-1 overflow-hidden">
							{#if row.confidence != null}
								<div
									class="h-full bg-accent/50"
									style="width: {Math.round(row.confidence * 100)}%"
								></div>
							{/if}
						</div>
						<span class="text-text-secondary tabular-nums w-16 text-right">{row.value}</span>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<!-- Tiered sections -->
	<div class="flex-1 min-w-0 grid gap-4 md:grid-cols-2 2xl:grid-cols-3 items-start">
		{#if show.nano}
			<ProfileSection
				title="Identity core"
				items={identityRows}
				empty="no confirmed identity observations"
			/>
		{/if}

		{#if show.micro}
			<ProfileSection title="Working" items={workingRows} empty="no confirmed working style" />
		{/if}

		{#if show.standard}
			<ProfileSection title="Domains" items={domainsRows} empty="no confirmed domains" />
			<ProfileSection title="Values" items={valuesRows} empty="no confirmed values" />
			<ProfileSection title="Reasoning" items={reasoningRows} empty="no confirmed reasoning" />
		{/if}

		{#if show.rich}
			<ProfileSection title="Signals" items={signalsRows} empty="no confirmed signals" />
			<ProfileSection title="Annotations" items={annotationRows} empty="no annotations" />
			{#if deltaRows.length > 0}
				<ProfileSection title="Open conflicts" items={deltaRows} empty="" />
			{/if}
		{/if}
	</div>
</div>
