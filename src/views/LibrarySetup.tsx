import { useState } from 'react';
import { useLibraryStore } from '../state/useLibraryStore';

export function LibrarySetup() {
  const {
    libraries,
    isLoading,
    error,
    scanningLibraries,
    addNewLibrary,
    startScan,
    pauseScan,
    resumeScan,
  } = useLibraryStore();
  const [path, setPath] = useState('');

  const handleAdd = async () => {
    if (!path.trim()) return;
    await addNewLibrary(path.trim());
    setPath('');
  };

  const handleScan = async (libraryId: number) => {
    await startScan(libraryId);
  };

  const handlePause = async (libraryId: number) => {
    await pauseScan(libraryId);
  };

  const handleResume = async (libraryId: number) => {
    await resumeScan(libraryId);
  };

  return (
    <div className="library-setup">
      <h2>Photo Libraries</h2>

      {error && <div className="error">{error}</div>}

      <div className="add-library">
        <input
          type="text"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          placeholder="Enter photo directory path..."
          disabled={isLoading}
        />
        <button onClick={handleAdd} disabled={isLoading || !path.trim()}>
          Add Library
        </button>
      </div>

      {libraries.length > 0 && (
        <div className="library-list">
          {libraries.map((lib) => {
            const isScanning = scanningLibraries.has(lib.id);
            return (
              <div key={lib.id} className="library-item">
                <div className="library-info">
                  <strong>{lib.name}</strong>
                  <span className="path">{lib.root_path}</span>
                </div>
                {isScanning ? (
                  <div className="scan-controls">
                    <span className="scanning-indicator">Scanning...</span>
                    <button onClick={() => handlePause(lib.id)}>Pause</button>
                  </div>
                ) : (
                  <div className="scan-controls">
                    <button onClick={() => handleScan(lib.id)} disabled={isLoading}>
                      Scan
                    </button>
                    <button onClick={() => handleResume(lib.id)} disabled={isLoading}>
                      Resume
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {libraries.length === 0 && !isLoading && (
        <p className="empty-state">
          No libraries added yet. Enter a photo directory path above to get started.
        </p>
      )}
    </div>
  );
}
