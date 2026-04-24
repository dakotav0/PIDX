<script lang="ts">
	export interface RadarAxis {
		path: string;
		label: string;
		value: number; // 0.0 – 1.0
	}

	interface Props {
		axes: RadarAxis[];
		selected?: string | null;
		onSelect?: (path: string) => void;
	}

	let { axes, selected = null, onSelect }: Props = $props();

	const CX = 150,
		CY = 150,
		R = 100,
		LABEL_R = 125;

	const N = $derived(axes.length);

	function angle(i: number) {
		return -Math.PI / 2 + ((2 * Math.PI) / N) * i;
	}

	function pt(i: number, r: number) {
		const a = angle(i);
		return { x: CX + r * Math.cos(a), y: CY + r * Math.sin(a) };
	}

	function dataPt(i: number) {
		const v = Math.max(0, Math.min(1, axes[i]?.value ?? 0));
		return pt(i, v * R);
	}

	function ringPoly(scale: number) {
		return Array.from({ length: N }, (_, i) => {
			const p = pt(i, scale * R);
			return `${p.x},${p.y}`;
		}).join(' ');
	}

	const polygon = $derived(
		axes.map((_, i) => {
			const p = dataPt(i);
			return `${p.x},${p.y}`;
		}).join(' ')
	);

	function anchor(i: number): 'start' | 'end' | 'middle' {
		const cos = Math.cos(angle(i));
		if (cos > 0.15) return 'start';
		if (cos < -0.15) return 'end';
		return 'middle';
	}

	const rings = [0.25, 0.5, 0.75, 1.0];
</script>

<svg viewBox="0 0 300 300" class="w-full max-w-[260px]" aria-label="Register radar chart">
	<!-- Ring guides -->
	{#each rings as scale}
		<polygon
			points={ringPoly(scale)}
			fill="none"
			stroke="var(--color-border)"
			stroke-width={scale === 1.0 ? 1 : 0.5}
		/>
	{/each}

	<!-- Axis spokes -->
	{#each axes as _, i}
		{@const tip = pt(i, R)}
		<line
			x1={CX}
			y1={CY}
			x2={tip.x}
			y2={tip.y}
			stroke="var(--color-border)"
			stroke-width="0.75"
		/>
	{/each}

	<!-- Data polygon -->
	{#if N > 0}
		<polygon
			points={polygon}
			fill="var(--color-accent)"
			fill-opacity="0.12"
			stroke="var(--color-accent)"
			stroke-width="1.5"
			stroke-linejoin="round"
		/>
	{/if}

	<!-- Data dots + labels -->
	{#each axes as axis, i}
		{@const dp = dataPt(i)}
		{@const lp = pt(i, LABEL_R)}
		{@const isSel = selected === axis.path}

		<text
			x={lp.x}
			y={lp.y}
			text-anchor={anchor(i)}
			dominant-baseline="middle"
			font-size="10"
			font-family="monospace"
			fill={isSel ? 'var(--color-accent)' : 'var(--color-text-secondary)'}
			class="cursor-pointer select-none"
			role="button"
			tabindex="0"
			onclick={() => onSelect?.(axis.path)}
			onkeydown={(e) => e.key === 'Enter' && onSelect?.(axis.path)}
		>{axis.label}</text>

		<circle
			cx={dp.x}
			cy={dp.y}
			r={isSel ? 5 : 3.5}
			fill="var(--color-accent)"
			fill-opacity={isSel ? 1 : 0.65}
			class="cursor-pointer"
			role="button"
			tabindex="0"
			onclick={() => onSelect?.(axis.path)}
			onkeydown={(e) => e.key === 'Enter' && onSelect?.(axis.path)}
		>
			<title>{axis.label}: {(axis.value * 100).toFixed(0)}%</title>
		</circle>
	{/each}
</svg>
