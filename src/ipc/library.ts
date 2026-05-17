import { invoke } from '@tauri-apps/api/core';

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

export async function listLibraries(): Promise<Library[]> {
  return invoke('library_list');
}

export async function addLibrary(path: string): Promise<Library> {
  return invoke('library_add', { path });
}

export async function scanLibrary(id: number): Promise<ScanJob> {
  return invoke('library_scan', { id });
}

export async function relocateFolder(
  libraryId: number,
  oldPrefix: string,
  newPrefix: string
): Promise<number> {
  return invoke('library_relocate_folder', { libraryId, oldPrefix, newPrefix });
}
