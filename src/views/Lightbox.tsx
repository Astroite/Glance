import { useState, useEffect } from 'react';
import { getPhotoDetail, getThumbnailUrl } from '../ipc';
import type { PhotoDetail } from '../ipc';

interface LightboxProps {
  photoId: number;
  onClose: () => void;
}

export function Lightbox({ photoId, onClose }: LightboxProps) {
  const [photo, setPhoto] = useState<PhotoDetail | null>(null);
  const [imageUrl, setImageUrl] = useState<string>('');
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadPhoto = async () => {
      setIsLoading(true);
      try {
        const detail = await getPhotoDetail(photoId);
        setPhoto(detail);

        const url = await getThumbnailUrl(photoId, 1080);
        setImageUrl(url);
      } catch (err) {
        console.error('Failed to load photo:', err);
      } finally {
        setIsLoading(false);
      }
    };

    loadPhoto();
  }, [photoId]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onClose();
    }
  };

  return (
    <div className="lightbox-overlay" onClick={onClose} onKeyDown={handleKeyDown} tabIndex={0}>
      <div className="lightbox-content" onClick={(e) => e.stopPropagation()}>
        {isLoading ? (
          <div className="loading">Loading...</div>
        ) : photo ? (
          <>
            <div className="lightbox-image">
              {photo.is_missing ? (
                <div className="missing-notice">
                  <p>Original file is missing</p>
                  {imageUrl && <img src={imageUrl} alt="Cached preview" />}
                </div>
              ) : (
                <img src={imageUrl} alt={`Photo ${photo.id}`} />
              )}
            </div>

            <div className="lightbox-info">
              <h3>Photo Details</h3>
              {photo.taken_at && (
                <p>
                  <strong>Date:</strong>{' '}
                  {new Date(photo.taken_at * 1000).toLocaleString()}
                </p>
              )}
              {photo.camera_model && (
                <p>
                  <strong>Camera:</strong> {photo.camera_make} {photo.camera_model}
                </p>
              )}
              {photo.lens && (
                <p>
                  <strong>Lens:</strong> {photo.lens}
                </p>
              )}
              {photo.focal_len && (
                <p>
                  <strong>Focal Length:</strong> {photo.focal_len}mm
                </p>
              )}
              {photo.aperture && (
                <p>
                  <strong>Aperture:</strong> f/{photo.aperture.toFixed(1)}
                </p>
              )}
              {photo.shutter && (
                <p>
                  <strong>Shutter:</strong> {formatShutter(photo.shutter)}
                </p>
              )}
              {photo.iso && (
                <p>
                  <strong>ISO:</strong> {photo.iso}
                </p>
              )}
              {photo.rating !== null && (
                <p>
                  <strong>Rating:</strong> {'★'.repeat(photo.rating)}{' '}
                  {'☆'.repeat(5 - photo.rating)}
                </p>
              )}
              {photo.label && (
                <p>
                  <strong>Label:</strong> {photo.label}
                </p>
              )}

              {photo.files.length > 1 && (
                <div className="related-files">
                  <h4>Related Files</h4>
                  <ul>
                    {photo.files.map((file) => (
                      <li key={file.id}>
                        <span className="role">{file.role}</span>
                        <span className="status">{file.status}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          </>
        ) : (
          <div className="error">Failed to load photo</div>
        )}

        <button className="close-button" onClick={onClose}>
          ×
        </button>
      </div>
    </div>
  );
}

function formatShutter(seconds: number): string {
  if (seconds >= 1) {
    return `${seconds.toFixed(1)}s`;
  }
  return `1/${Math.round(1 / seconds)}s`;
}
