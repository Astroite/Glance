import { useEffect, useCallback } from 'react';
import { VirtuosoGrid } from 'react-virtuoso';
import { useTimelineStore } from '../state/useTimelineStore';
import { useLibraryStore } from '../state/useLibraryStore';
import { ThumbnailGrid } from '../components/ThumbnailGrid';
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

  if (!selectedLibrary) {
    return <div className="timeline-empty">Select a library to browse photos</div>;
  }

  // Group photos by date
  const groupedPhotos = groupPhotosByDate(photos);

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
          <VirtuosoGrid
            useWindowScroll
            totalCount={groupedPhotos.length}
            endReached={handleEndReached}
            overscan={200}
            components={{
              ScrollSeekPlaceholder: () => <div className="photo-placeholder" />,
            }}
            itemContent={(index) => {
              const group = groupedPhotos[index];
              return (
                <div className="date-group">
                  <div className="date-header">{group.date}</div>
                  <ThumbnailGrid
                    photos={group.photos}
                    viewMode={viewMode}
                  />
                </div>
              );
            }}
          />
        )}

        {isLoading && <div className="loading">Loading...</div>}
      </div>
    </div>
  );
}

interface DateGroup {
  date: string;
  photos: PhotoSummary[];
}

function groupPhotosByDate(photos: PhotoSummary[]): DateGroup[] {
  const groups: Map<string, PhotoSummary[]> = new Map();

  for (const photo of photos) {
    const date = photo.taken_at
      ? new Date(photo.taken_at * 1000).toLocaleDateString('zh-CN', {
          year: 'numeric',
          month: 'long',
          day: 'numeric',
        })
      : 'Unknown Date';

    if (!groups.has(date)) {
      groups.set(date, []);
    }
    groups.get(date)!.push(photo);
  }

  return Array.from(groups.entries()).map(([date, photos]) => ({
    date,
    photos,
  }));
}
