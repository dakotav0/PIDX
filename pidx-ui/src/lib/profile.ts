import type { ProfileDocument, ProfileField, ObservationRow, ObservationDomainValue } from './ipc';

export interface DiffField {
	path: string;
	label: string;
	group: string;
}

export const DIFF_FIELDS: DiffField[] = [
	{ path: 'working.mode', label: 'mode', group: 'Working' },
	{ path: 'working.pace', label: 'pace', group: 'Working' },
	{ path: 'working.feedback', label: 'feedback', group: 'Working' },
	{ path: 'working.pattern', label: 'pattern', group: 'Working' },
	{ path: 'identity.reasoning.style', label: 'style', group: 'Reasoning' },
	{ path: 'identity.reasoning.pattern', label: 'pattern', group: 'Reasoning' },
	{ path: 'identity.reasoning.intake', label: 'intake', group: 'Reasoning' },
	{ path: 'identity.reasoning.stance', label: 'stance', group: 'Reasoning' },
];

export const RADAR_AXES: { path: string; label: string }[] = [
	{ path: 'working.mode', label: 'mode' },
	{ path: 'working.pace', label: 'pace' },
	{ path: 'working.feedback', label: 'feedback' },
	{ path: 'working.pattern', label: 'pattern' },
	{ path: 'identity.reasoning.style', label: 'style' },
	{ path: 'identity.reasoning.stance', label: 'stance' },
];

export function getFieldObs(profile: ProfileDocument, path: string): ObservationRow[] {
	return resolveField(profile, path)?.observations ?? [];
}

function resolveField(profile: ProfileDocument, path: string): ProfileField | null {
	const parts = path.split('.');
	switch (parts[0]) {
		case 'working':
			return (profile.working as Record<string, ProfileField>)[parts[1]] ?? null;
		case 'identity':
			if (parts[1] === 'core') return profile.identity.core[Number(parts[2])] ?? null;
			if (parts[1] === 'reasoning')
				return (profile.identity.reasoning as Record<string, ProfileField>)[parts[2]] ?? null;
			return null;
		case 'domains':
			return profile.domains[Number(parts[1])] ?? null;
		case 'values':
			return profile.values[Number(parts[1])] ?? null;
		case 'signals': {
			const list = (profile.signals as Record<string, ProfileField[]>)[parts[1]];
			return list?.[Number(parts[2])] ?? null;
		}
		default:
			return null;
	}
}

export function getActiveValue(profile: ProfileDocument, path: string): string {
	const obs = getFieldObs(profile, path);
	const confirmed = obs.filter((o) => o.status === 'confirmed');
	if (confirmed.length === 0) return '—';
	const best = confirmed.reduce((a, b) => (a.confidence >= b.confidence ? a : b));
	return formatObsValue(best.value);
}

export function fieldConfidence(profile: ProfileDocument, path: string): number {
	const obs = getFieldObs(profile, path);
	const confirmed = obs.filter((o) => o.status === 'confirmed');
	if (confirmed.length === 0) return 0;
	return Math.max(...confirmed.map((o) => o.confidence));
}

export function formatObsValue(
	value: string | number | ObservationDomainValue
): string {
	if (typeof value === 'string') return value;
	if (typeof value === 'number') return String(value);
	const parts = [value.label];
	if (value.proficiency) parts.push(value.proficiency);
	parts.push(`${(value.weight * 100).toFixed(0)}%`);
	return parts.join(' · ');
}

export function formatDate(ts: string): string {
	return ts.slice(0, 10);
}
