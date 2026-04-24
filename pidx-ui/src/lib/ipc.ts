/**
 * Typed IPC wrappers over Tauri invoke().
 *
 * Each function maps directly to a command in src-tauri/src/commands.rs.
 * Types are hand-written to match the serde_json::Value shapes those commands
 * return — update here when command signatures change.
 */

import { invoke } from '@tauri-apps/api/core';

// ── Shared types ─────────────────────────────────────────────────────────────

export interface OkResult {
	ok: boolean;
	error?: string;
}

export interface UserEntry {
	user_id: string;
	version: string;
	updated: string;
	overall_confidence: number;
}

export interface FieldSummary {
	path: string;
	confirmed: number;
	proposed: number;
	delta: number;
}

export interface StatusResult {
	user_id: string;
	version: string;
	overall_confidence: number;
	updated: string;
	fields: FieldSummary[];
	delta_queue_open: number;
	review_queue_pending: number;
}

// ── Profile document types ────────────────────────────────────────────────────

export type ObservationStatus = 'proposed' | 'confirmed' | 'rejected' | 'delta' | 'archived';

export interface ObservationDomainValue {
	label: string;
	weight: number;
	proficiency?: string;
}

export interface ObservationRow {
	value: string | number | ObservationDomainValue;
	source: {
		origination: string;
		orientation: string;
		session_ref: string;
		timestamp: string;
	};
	confidence: number;
	weight: number;
	status: ObservationStatus;
	revision: number;
	decay_exempt: boolean;
}

export interface ProfileField {
	observations: ObservationRow[];
	proposal_count?: number;
}

export interface DeltaItem {
	id: string;
	field: string;
	a: ObservationRow;
	b: ObservationRow;
	created_at: string;
	resolved: boolean;
}

export interface ReviewItem {
	id: string;
	field: string;
	observation_index: number;
	effective_confidence: number;
	flagged_at: string;
	resolved: boolean;
}

export interface ProfileDocument {
	meta: {
		user_id: string;
		version: string;
		updated: string;
		overall_confidence: number;
	};
	working: {
		mode: ProfileField;
		pace: ProfileField;
		feedback: ProfileField;
		pattern: ProfileField;
	};
	identity: {
		core: ProfileField[];
		reasoning: {
			style: ProfileField;
			pattern: ProfileField;
			intake: ProfileField;
			stance: ProfileField;
		};
	};
	domains: ProfileField[];
	values: ProfileField[];
	signals: {
		phrases: ProfileField[];
		avoidances: ProfileField[];
		rhythms: ProfileField[];
		framings: ProfileField[];
	};
	delta_queue: DeltaItem[];
	review_queue: ReviewItem[];
	annotations: unknown[];
}

export interface DeltaEntry {
	id: string;
	field: string;
	a: { orientation: string; value: string };
	b: { orientation: string; value: string };
}

export interface ReviewEntry {
	id: string;
	field: string;
	observation_index: number;
	effective_confidence: number;
	flagged_at: string;
}

// ── Read commands ─────────────────────────────────────────────────────────────

export function listUsers(): Promise<{ count: number; users: UserEntry[] }> {
	return invoke('list_users');
}

export function getProfile(userId: string): Promise<ProfileDocument> {
	return invoke('get_profile', { user_id: userId });
}

export function getShow(userId: string, tier: 'nano' | 'micro' | 'standard' | 'rich'): Promise<string> {
	return invoke('get_show', { user_id: userId, tier });
}

export function getStatus(userId: string): Promise<StatusResult> {
	return invoke('get_status', { user_id: userId });
}

// ── Write commands ────────────────────────────────────────────────────────────

export function confirmObservation(
	userId: string,
	field: string,
	index: number
): Promise<OkResult & { field: string; index: number; value: string; new_status: string }> {
	return invoke('confirm_observation', { user_id: userId, field, index });
}

export function rejectObservation(
	userId: string,
	field: string,
	index: number
): Promise<OkResult & { field: string; index: number; new_status: string }> {
	return invoke('reject_observation', { user_id: userId, field, index });
}

export function confirmAll(
	userId: string,
	fieldPrefix: string
): Promise<OkResult & { confirmed_count: number; fields: string[] }> {
	return invoke('confirm_all', { user_id: userId, field_prefix: fieldPrefix });
}

export function rejectAll(
	userId: string,
	fieldPrefix: string
): Promise<OkResult & { rejected_count: number; fields: string[] }> {
	return invoke('reject_all', { user_id: userId, field_prefix: fieldPrefix });
}

export function clearProfile(
	userId: string,
	target: 'deltas' | 'reviews' | 'proposed' | 'all'
): Promise<OkResult & { target: string; cleared_count: number }> {
	return invoke('clear', { user_id: userId, target });
}

// ── Lifecycle commands ────────────────────────────────────────────────────────

export function ingestPacketContent(
	userId: string,
	packetJson: string
): Promise<OkResult & { observations_proposed: number; deltas_flagged: number }> {
	return invoke('ingest_packet_content', { user_id: userId, packet_json: packetJson });
}

export function ingestPacket(
	userId: string,
	packetPath: string
): Promise<OkResult & { observations_proposed: number; deltas_flagged: number }> {
	return invoke('ingest_packet', { user_id: userId, packet_path: packetPath });
}

export function resolveReview(
	userId: string,
	reviewId: string,
	action: 'keep' | 'discard'
): Promise<OkResult & { review_id: string; action: string; field: string }> {
	return invoke('resolve_review', { user_id: userId, review_id: reviewId, action });
}

export function resolveDelta(
	userId: string,
	deltaId: string,
	keep: 'a' | 'b'
): Promise<OkResult & { delta_id: string; kept: string; field: string }> {
	return invoke('resolve_delta', { user_id: userId, delta_id: deltaId, keep });
}

export function annotate(
	userId: string,
	field: string,
	note: string,
	pinned: boolean
): Promise<OkResult & { id: string; field: string; note: string; pinned: boolean }> {
	return invoke('annotate', { user_id: userId, field, note, pinned });
}

export function runDecay(
	userId: string,
	threshold?: number
): Promise<OkResult & { newly_flagged: number; review_queue_pending: number }> {
	return invoke('decay', { user_id: userId, threshold });
}
