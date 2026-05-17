import { invoke } from '@tauri-apps/api/core';

export interface PhotoSummary {
  id: number;
  taken_at: number | null;
  content_hash: string;
  orientation: number | null;
  width: number | null;
  height: number | null;
  is_missing: boolean;
}

export interface TimelinePage {
  photos: PhotoSummary[];
  next_cursor: string | null;
}

export interface PhotoDetail {
  id: number;
  library_id: number;
  taken_at: number | null;
  camera_make: string | null;
  camera_model: string | null;
  lens: string | null;
  focal_len: number | null;
  aperture: number | null;
  shutter: number | null;
  iso: number | null;
  width: number | null;
  height: number | null;
  orientation: number | null;
  gps_lat: number | null;
  gps_lon: number | null;
  rating: number | null;
  label: string | null;
  format: string;
  is_missing: boolean;
  files: PhotoFileInfo[];
}

export interface PhotoFileInfo {
  id: number;
  path: string;
  role: string;
  status: string;
}

export async function queryTimeline(
  libraryId: number,
  cursor: string | null = null,
  limit: number = 200
): Promise<TimelinePage> {
  return invoke('timeline_query', { libraryId, cursor, limit });
}

export async function getPhotoDetail(id: number): Promise<PhotoDetail> {
  return invoke('photo_detail', { id });
}

export async function getThumbnailUrl(
  photoId: number,
  tier: 240 | 480 | 1080
): Promise<string> {
  return invoke('thumbnail_url', { photoId, tier });
}

export async function relocateFile(
  photoFileId: number,
  newPath: string
): Promise<void> {
  return invoke('photo_relocate_file', { photoFileId, newPath });
}
