import { useState } from 'react';
import { useLibraryStore } from '../state/useLibraryStore';

export function LibrarySetup() {
  const { libraries, isLoading, error, addNewLibrary, startScan } = useLibraryStore();
  const [path, setPath] = useState('');

  const handleAdd = async () => {
    if (!path.trim()) return;
    await addNewLibrary(path.trim());
    setPath('');
  };

  const handleScan = async (libraryId: number) => {
    await startScan(libraryId);
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
          {libraries.map((lib) => (
            <div key={lib.id} className="library-item">
              <div className="library-info">
                <strong>{lib.name}</strong>
                <span className="path">{lib.root_path}</span>
              </div>
              <button onClick={() => handleScan(lib.id)} disabled={isLoading}>
                Scan
              </button>
            </div>
          ))}
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
