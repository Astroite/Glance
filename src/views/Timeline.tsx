import { useEffect, useCallback, useState, useMemo } from 'react';
import { GroupedVirtuoso } from 'react-virtuoso';
import { useTimelineStore } from '../state/useTimelineStore';
import { useLibraryStore } from '../state/useLibraryStore';
import { ThumbnailGrid } from '../components/ThumbnailGrid';
import { Lightbox } from './Lightbox';
import type { PhotoSummary } from '../ipc';

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

  // Group photos by date for Virtuoso group API
  const { groups, groupCounts, flatPhotos } = useMemo(() => {
    return buildGroupedData(photos);
  }, [photos]);

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
            groupCounts={groupCounts}
            endReached={handleEndReached}
            overscan={200}
            groupContent={(index: number) => (
              <div className="date-header">{groups[index]}</div>
            )}
            itemContent={(index: number) => {
              const photo = flatPhotos[index];
              return (
                <div className="photo-item-wrapper">
                  <ThumbnailGrid
                    photos={[photo]}
                    viewMode={viewMode}
                    onPhotoClick={handlePhotoClick}
                  />
                </div>
              );
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

interface GroupedData {
  groups: string[];
  groupCounts: number[];
  flatPhotos: PhotoSummary[];
}

function buildGroupedData(photos: PhotoSummary[]): GroupedData {
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
  const groupCounts: number[] = [];
  const flatPhotos: PhotoSummary[] = [];

  for (const [date, datePhotos] of groupMap) {
    groups.push(date);
    groupCounts.push(datePhotos.length);
    flatPhotos.push(...datePhotos);
  }

  return { groups, groupCounts, flatPhotos };
}
