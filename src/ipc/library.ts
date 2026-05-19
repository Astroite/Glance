import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface Library {
  id: number;
  name: string;
  root_path: string;
  created_at: number;
}

export interface ScanJob {
  id: number;
  library_id: number;
  status: string;
  cursor: string | null;
  started_at: number;
  finished_at: number | null;
  added: number;
  updated: number;
  missing: number;
}

export interface ScanCompletePayload {
  library_id: number;
  result: { added: number; updated: number; missing: number };
  job: ScanJob | null;
}

export interface ScanErrorPayload {
  library_id: number;
  error: string;
}

export async function listLibraries(): Promise<Library[]> {
  return invoke('library_list');
}

export async function addLibrary(path: string): Promise<Library> {
  return invoke('library_add', { path });
}

export async function scanLibrary(id: number): Promise<void> {
  return invoke('library_scan', { id });
}

export async function relocateFolder(
  libraryId: number,
  oldPrefix: string,
  newPrefix: string
): Promise<number> {
  return invoke('library_relocate_folder', { libraryId, oldPrefix, newPrefix });
}

export async function pauseScan(libraryId: number): Promise<void> {
  return invoke('library_scan_pause', { id: libraryId });
}

export async function resumeScan(libraryId: number): Promise<void> {
  return invoke('library_scan_resume', { id: libraryId });
}

export function onScanComplete(callback: (payload: ScanCompletePayload) => void) {
  return listen<ScanCompletePayload>('scan-complete', (event) => callback(event.payload));
}

export function onScanPaused(callback: (payload: { library_id: number; job: ScanJob | null }) => void) {
  return listen<{ library_id: number; job: ScanJob | null }>('scan-paused', (event) => callback(event.payload));
}

export function onScanError(callback: (payload: ScanErrorPayload) => void) {
  return listen<ScanErrorPayload>('scan-error', (event) => callback(event.payload));
}
