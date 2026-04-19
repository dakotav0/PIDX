<script lang="ts">
	import { listUsers, ingestPacketContent, type UserEntry } from '$lib/ipc';
	import { onMount } from 'svelte';

	let users = $state<UserEntry[]>([]);
	let userId = $state('');
	let orientation = $state('user');
	let sessionRef = $state(crypto.randomUUID().slice(0, 8));

	interface ObsRow {
		id: number;
		field: string;
		value: string;
		origination: 'passive' | 'direct';
	}

	let rows = $state<ObsRow[]>([{ id: 0, field: '', value: '', origination: 'passive' }]);
	let nextId = 1;

	let submitting = $state(false);
	let result = $state<{ proposed: number; deltas: number } | null>(null);
	let error = $state<string | null>(null);

	onMount(async () => {
		try {
			const res = await listUsers();
			users = res.users;
			if (users.length > 0) userId = users[0].user_id;
		} catch {
			// non-fatal — user can type the id
		}
	});

	function addRow() {
		rows = [...rows, { id: nextId++, field: '', value: '', origination: 'passive' }];
	}

	function removeRow(id: number) {
		rows = rows.filter((r) => r.id !== id);
		if (rows.length === 0) addRow();
	}

	function parseValue(s: string): string | number {
		const n = Number(s);
		return !isNaN(n) && s.trim() !== '' ? n : s;
	}

	function buildPacket(): string {
		const observations = rows
			.filter((r) => r.field.trim() && r.value.trim())
			.map((r) => ({
				field: r.field.trim(),
				value: parseValue(r.value.trim()),
				origination: r.origination
			}));

		return JSON.stringify({
			bridge_version: '0.1',
			orientation: orientation.trim() || 'user',
			session_ref: sessionRef,
			timestamp: new Date().toISOString(),
			observations
		});
	}

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		const validRows = rows.filter((r) => r.field.trim() && r.value.trim());
		if (!userId.trim() || validRows.length === 0) return;

		submitting = true;
		result = null;
		error = null;
		try {
			const res = await ingestPacketContent(userId.trim(), buildPacket());
			result = { proposed: res.observations_proposed, deltas: res.deltas_flagged };
			sessionRef = crypto.randomUUID().slice(0, 8);
		} catch (e) {
			error = String(e);
		} finally {
			submitting = false;
		}
	}

	const FIELD_SUGGESTIONS = [
		'working.mode',
		'working.pace',
		'working.feedback',
		'working.pattern',
		'identity.reasoning.style',
		'identity.reasoning.pattern',
		'identity.reasoning.intake',
		'identity.reasoning.stance',
	];
</script>

<datalist id="field-paths">
	{#each FIELD_SUGGESTIONS as f}
		<option value={f}></option>
	{/each}
</datalist>

<main class="p-6 max-w-2xl">
	<h1 class="text-xl font-bold text-accent mb-1">Bridge</h1>
	<p class="text-sm text-text-muted mb-6">Author a bridge packet and ingest it directly.</p>

	<form onsubmit={submit} class="space-y-6">
		<!-- User + session -->
		<div class="grid grid-cols-2 gap-4">
			<div>
				<label for="bridge-user" class="block text-xs text-text-muted mb-1">user</label>
				{#if users.length > 0}
					<select
						id="bridge-user"
						bind:value={userId}
						class="w-full bg-surface-2 border border-border rounded px-2 py-1.5 text-sm text-text-primary focus:outline-none focus:border-accent"
					>
						{#each users as u}
							<option value={u.user_id}>{u.user_id}</option>
						{/each}
					</select>
				{:else}
					<input
						id="bridge-user"
						bind:value={userId}
						placeholder="user_id"
						class="w-full bg-surface-2 border border-border rounded px-2 py-1.5 text-sm text-text-primary focus:outline-none focus:border-accent"
					/>
				{/if}
			</div>
			<div>
				<label for="bridge-orientation" class="block text-xs text-text-muted mb-1">orientation</label>
				<input
					id="bridge-orientation"
					bind:value={orientation}
					placeholder="user"
					class="w-full bg-surface-2 border border-border rounded px-2 py-1.5 text-sm text-text-primary font-mono focus:outline-none focus:border-accent"
				/>
			</div>
		</div>

		<div>
			<label for="bridge-session" class="block text-xs text-text-muted mb-1">session ref</label>
			<div class="flex gap-2 items-center">
				<input
					id="bridge-session"
					bind:value={sessionRef}
					class="bg-surface-2 border border-border rounded px-2 py-1.5 text-sm text-text-secondary font-mono focus:outline-none focus:border-accent w-48"
				/>
				<button
					type="button"
					class="text-xs text-text-muted hover:text-text-secondary"
					onclick={() => (sessionRef = crypto.randomUUID().slice(0, 8))}>↺ regenerate</button
				>
			</div>
		</div>

		<!-- Observations table -->
		<div>
			<div class="flex items-center justify-between mb-2">
				<span class="text-xs text-text-muted">observations</span>
				<button
					type="button"
					class="text-xs text-accent hover:text-accent/80"
					onclick={addRow}>+ add row</button
				>
			</div>

			<div class="space-y-2">
				{#each rows as row (row.id)}
					<div class="flex gap-2 items-center">
						<input
							bind:value={row.field}
							list="field-paths"
							placeholder="field.path"
							class="flex-1 bg-surface-2 border border-border rounded px-2 py-1.5 text-xs font-mono text-text-primary focus:outline-none focus:border-accent"
						/>
						<input
							bind:value={row.value}
							placeholder="value"
							class="flex-1 bg-surface-2 border border-border rounded px-2 py-1.5 text-xs text-text-primary focus:outline-none focus:border-accent"
						/>
						<select
							bind:value={row.origination}
							class="bg-surface-2 border border-border rounded px-2 py-1.5 text-xs text-text-secondary focus:outline-none focus:border-accent"
						>
							<option value="passive">passive</option>
							<option value="direct">direct</option>
						</select>
						<button
							type="button"
							class="text-text-muted hover:text-error text-sm w-5 shrink-0"
							onclick={() => removeRow(row.id)}>×</button
						>
					</div>
				{/each}
			</div>
		</div>

		<!-- Submit -->
		<div class="flex items-center gap-4">
			<button
				type="submit"
				disabled={submitting || !userId.trim() || rows.every((r) => !r.field.trim())}
				class="px-4 py-2 text-sm rounded border border-accent text-accent hover:bg-accent hover:text-surface-0 disabled:opacity-40 transition-colors"
			>
				{submitting ? 'Ingesting…' : 'Ingest Packet'}
			</button>

			{#if result}
				<span class="text-sm">
					<span class="text-accent">✓ {result.proposed} proposed</span>
					{#if result.deltas > 0}
						<span class="text-orange-400 ml-2">△ {result.deltas} deltas</span>
					{/if}
				</span>
			{/if}
			{#if error}
				<span class="text-sm text-error">{error}</span>
			{/if}
		</div>
	</form>
</main>
