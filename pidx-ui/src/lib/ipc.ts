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
	proposal_count?: number;
	preview: string | null;
}

export interface StatusResult {
	user_id: string;
	version: string;
	overall_confidence: number;
	updated: string;
	fields: FieldSummary[];
	totals: { confirmed: number; proposed: number; delta: number };
	delta_queue_open: number;
	review_queue_pending: number;
	bridge_log_processed: number;
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

export function getProfile(userId: string): Promise<unknown> {
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

export function ingestPacket(
	userId: string,
	packetPath: string
): Promise<OkResult & { observations_proposed: number; deltas_flagged: number }> {
	return invoke('ingest_packet', { user_id: userId, packet_path: packetPath });
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
): Promise<OkResult & { flagged: number; review_queue_size: number }> {
	return invoke('decay', { user_id: userId, threshold });
}
