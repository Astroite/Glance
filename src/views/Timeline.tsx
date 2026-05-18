import { useEffect, useCallback, useState, useMemo } from 'react';
import { GroupedVirtuoso } from 'react-virtuoso';
import { useTimelineStore } from '../state/useTimelineStore';
import { useLibraryStore } from '../state/useLibraryStore';
import { Lightbox } from './Lightbox';
import type { PhotoSummary } from '../ipc';

function useColumnCount(): number {
  const [cols, setCols] = useState(() => Math.max(1, Math.min(12, Math.floor(window.innerWidth / 200))));

  useEffect(() => {
    const update = () => setCols(Math.max(1, Math.min(12, Math.floor(window.innerWidth / 200))));
    window.addEventListener('resize', update);
    return () => window.removeEventListener('resize', update);
  }, []);

  return cols;
}

interface GroupedData {
  groups: string[];
  rowCounts: number[];
  rows: PhotoSummary[][];
}

function buildGroupedData(photos: PhotoSummary[], cols: number): GroupedData {
  const groupMap = new Map<string, PhotoSummary[]>();

  for (const photo of photos) {
    const date = photo.taken_at
      ? new Date(photo.taken_at * 1000).toLocaleDateString('zh-CN', {
          year: 'numeric',
          month: 'long',
          day: 'numeric',
        })
      : 'Unknown Date';

    if (!groupMap.has(date)) {
      groupMap.set(date, []);
    }
    groupMap.get(date)!.push(photo);
  }

  const groups: string[] = [];
  const rowCounts: number[] = [];
  const rows: PhotoSummary[][] = [];

  for (const [date, datePhotos] of groupMap) {
    groups.push(date);
    const groupRows: PhotoSummary[][] = [];
    for (let i = 0; i < datePhotos.length; i += cols) {
      groupRows.push(datePhotos.slice(i, i + cols));
    }
    rowCounts.push(groupRows.length);
    rows.push(...groupRows);
  }

  return { groups, rowCounts, rows };
}

interface PhotoRowProps {
  photos: PhotoSummary[];
  onPhotoClick?: (photoId: number) => void;
}

function PhotoRow({ photos, onPhotoClick }: PhotoRowProps) {
  return (
    <div className="photo-row" style={{ '--cols': photos.length } as React.CSSProperties}>
      {photos.map((p) => (
        <div
          key={p.id}
          className={`thumbnail-item ${p.is_missing ? 'missing' : ''}`}
          onClick={() => onPhotoClick?.(p.id)}
        >
          <img
            src={p.thumbnail_url}
            alt={`Photo ${p.id}`}
            loading="lazy"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = 'none';
            }}
          />
          {p.is_missing && <div className="missing-badge">Missing</div>}
        </div>
      ))}
    </div>
  );
}

export function Timeline() {
  const { selectedLibrary } = useLibraryStore();
  const {
    photos,
    isLoading,
    hasMore,
    viewMode,
    loadInitial,
    loadMore,
    setViewMode,
  } = useTimelineStore();

  const [selectedPhotoId, setSelectedPhotoId] = useState<number | null>(null);
  const cols = useColumnCount();

  useEffect(() => {
    if (selectedLibrary) {
      loadInitial(selectedLibrary.id);
    }
  }, [selectedLibrary, loadInitial]);

  const handleEndReached = useCallback(() => {
    if (selectedLibrary && hasMore && !isLoading) {
      loadMore(selectedLibrary.id);
    }
  }, [selectedLibrary, hasMore, isLoading, loadMore]);

  const handlePhotoClick = useCallback((photoId: number) => {
    setSelectedPhotoId(photoId);
  }, []);

  const handleCloseLightbox = useCallback(() => {
    setSelectedPhotoId(null);
  }, []);

  const { groups, rowCounts, rows } = useMemo(() => {
    return buildGroupedData(photos, cols);
  }, [photos, cols]);

  if (!selectedLibrary) {
    return <div className="timeline-empty">Select a library to browse photos</div>;
  }

  return (
    <div className="timeline">
      <div className="timeline-header">
        <h2>{selectedLibrary.name}</h2>
        <div className="view-controls">
          <button
            className={viewMode === 'waterfall' ? 'active' : ''}
            onClick={() => setViewMode('waterfall')}
          >
            Waterfall
          </button>
          <button
            className={viewMode === 'grid' ? 'active' : ''}
            onClick={() => setViewMode('grid')}
          >
            Grid
          </button>
        </div>
      </div>

      <div className="timeline-content">
        {photos.length === 0 && !isLoading ? (
          <div className="empty-state">No photos found in this library</div>
        ) : (
          <GroupedVirtuoso
            useWindowScroll
            groupCounts={rowCounts}
            endReached={handleEndReached}
            overscan={200}
            groupContent={(index: number) => (
              <div className="date-header">{groups[index]}</div>
            )}
            itemContent={(index: number) => {
              const row = rows[index];
              return <PhotoRow photos={row} onPhotoClick={handlePhotoClick} />;
            }}
          />
        )}

        {isLoading && <div className="loading">Loading...</div>}
      </div>

      {selectedPhotoId !== null && (
        <Lightbox photoId={selectedPhotoId} onClose={handleCloseLightbox} />
      )}
    </div>
  );
}
