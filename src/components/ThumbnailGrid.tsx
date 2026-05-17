import { useState, useEffect } from 'react';
import { getThumbnailUrl } from '../ipc';
import type { PhotoSummary } from '../ipc';

interface ThumbnailGridProps {
  photos: PhotoSummary[];
  viewMode: 'waterfall' | 'grid';
  onPhotoClick?: (photoId: number) => void;
}

export function ThumbnailGrid({ photos, viewMode, onPhotoClick }: ThumbnailGridProps) {
  return (
    <div className={`thumbnail-grid ${viewMode}`}>
      {photos.map((photo) => (
        <ThumbnailItem
          key={photo.id}
          photo={photo}
          viewMode={viewMode}
          onClick={() => onPhotoClick?.(photo.id)}
        />
      ))}
    </div>
  );
}

interface ThumbnailItemProps {
  photo: PhotoSummary;
  viewMode: 'waterfall' | 'grid';
  onClick?: () => void;
}

function ThumbnailItem({ photo, viewMode, onClick }: ThumbnailItemProps) {
  const [thumbnailUrl, setThumbnailUrl] = useState<string>('');
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadThumbnail = async () => {
      try {
        const tier = viewMode === 'grid' ? 240 : 480;
        const url = await getThumbnailUrl(photo.id, tier);
        setThumbnailUrl(url);
      } catch (err) {
        console.error('Failed to load thumbnail:', err);
      } finally {
        setIsLoading(false);
      }
    };

    loadThumbnail();
  }, [photo.id, viewMode]);

  return (
    <div
      className={`thumbnail-item ${photo.is_missing ? 'missing' : ''}`}
      onClick={onClick}
    >
      {isLoading ? (
        <div className="thumbnail-placeholder" />
      ) : (
        <img
          src={thumbnailUrl}
          alt={`Photo ${photo.id}`}
          loading="lazy"
        />
      )}
      {photo.is_missing && (
        <div className="missing-badge">Missing</div>
      )}
    </div>
  );
}
