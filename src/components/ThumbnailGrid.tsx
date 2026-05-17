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
          onClick={() => onPhotoClick?.(photo.id)}
        />
      ))}
    </div>
  );
}

interface ThumbnailItemProps {
  photo: PhotoSummary;
  onClick?: () => void;
}

function ThumbnailItem({ photo, onClick }: ThumbnailItemProps) {
  // Use the thumbnail_url from PhotoSummary directly — no separate IPC call needed
  const thumbnailUrl = photo.thumbnail_url;

  return (
    <div
      className={`thumbnail-item ${photo.is_missing ? 'missing' : ''}`}
      onClick={onClick}
    >
      <img
        src={thumbnailUrl}
        alt={`Photo ${photo.id}`}
        loading="lazy"
        onError={(e) => {
          // Fallback to placeholder on error
          (e.target as HTMLImageElement).style.display = 'none';
        }}
      />
      {photo.is_missing && (
        <div className="missing-badge">Missing</div>
      )}
    </div>
  );
}
